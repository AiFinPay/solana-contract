use anchor_lang::prelude::*;

use crate::{
    constants::{BPS_DENOMINATOR, MAX_QUOTE_LIFETIME_SECONDS, MESSAGE_DOMAIN_TAG},
    error::ErrorCode,
    state::{Config, ConsumedNonce, PayerNonce, ProfilesIndex, Quote, RouteProfileEntry},
};

/// Event emitted on every successful settlement.
#[event]
pub struct Payment {
    pub payment_id: [u8; 32],
    pub payer: Pubkey,
    pub merchant: Pubkey,
    pub token: Pubkey,
    pub gross_amount: u64,
    pub merchant_amount: u64,
    pub treasury_amount: u64,
    pub ip_creator_amount: u64,
    pub valid_until: i64,
    pub route_id: [u8; 32],
    pub order_id_hash: [u8; 32],
}

#[event]
pub struct TreasuryUpdated {
    pub new_treasury: Pubkey,
}

/// Compute the Solana-native signed message hash for a Quote.
///
/// This is NOT EIP-712. It uses:
/// - a versioned domain tag (not keccak),
/// - the Solana program_id as the domain,
/// - an explicit, hand-written byte encoding of `Quote` fields in a fixed
///   order (NOT Borsh), so the contract with off-chain signers is part of
///   source and does not silently change if `Quote` is reordered or
///   extended,
/// - SHA-256 (Solana's native hash function).
///
/// Field order MUST match the EVM v1.4 abi.encode(quote) layout.
/// DO NOT REORDER. This is a cross-chain sacred constant.
pub fn quote_message_hash(program_id: &Pubkey, quote: &Quote) -> [u8; 32] {
    let mut buf = [0u8; QUOTE_ENCODED_LEN];
    encode_quote(quote, &mut buf);
    solana_program::hash::hashv(&[MESSAGE_DOMAIN_TAG, program_id.as_ref(), &buf]).to_bytes()
}

/// Canonical digest exposed for tests and off-chain signing compatibility.
/// Alias for [`quote_message_hash`].
pub fn digest(program_id: &Pubkey, quote: &Quote) -> [u8; 32] {
    quote_message_hash(program_id, quote)
}

/// Length in bytes of the explicit Quote encoding:
/// payer(32) + merchant(32) + token(32) + gross_amount(8) + ip_creator(32)
/// + valid_until(8) + order_id_hash(32) + nonce(8) + route_id(32) = 216.
pub const QUOTE_ENCODED_LEN: usize = 32 + 32 + 32 + 8 + 32 + 8 + 32 + 8 + 32;

/// Hand-written little-endian encoding of every Quote field, in a fixed
/// order. Reordering or extending `Quote` does NOT change this layout,
/// but the on-chain verifier and the off-chain signer MUST keep this
/// identical. Cross-chain sacred constant.
pub fn encode_quote(quote: &Quote, out: &mut [u8; QUOTE_ENCODED_LEN]) {
    let mut o = 0usize;
    out[o..o + 32].copy_from_slice(quote.payer.as_ref());
    o += 32;
    out[o..o + 32].copy_from_slice(quote.merchant.as_ref());
    o += 32;
    out[o..o + 32].copy_from_slice(quote.token.as_ref());
    o += 32;
    out[o..o + 8].copy_from_slice(&quote.gross_amount.to_le_bytes());
    o += 8;
    out[o..o + 32].copy_from_slice(quote.ip_creator.as_ref());
    o += 32;
    out[o..o + 8].copy_from_slice(&quote.valid_until.to_le_bytes());
    o += 8;
    out[o..o + 32].copy_from_slice(&quote.order_id_hash);
    o += 32;
    out[o..o + 8].copy_from_slice(&quote.nonce.to_le_bytes());
    o += 8;
    out[o..o + 32].copy_from_slice(&quote.route_id);
    debug_assert_eq!(o + 32, QUOTE_ENCODED_LEN);
}

/// Recover the secp256k1 public key from a 65-byte Ethereum-style signature.
/// signature layout: r (32) || s (32) || v (1). recovery_id = v - 27.
/// Rejects high-s values to prevent ECDSA malleability (EIP-2 canonical sigs).
pub fn recover_signer(digest: &[u8; 32], signature: &[u8; 65]) -> Result<[u8; 64]> {
    use solana_secp256k1_recover::secp256k1_recover;

    let v = signature[64];
    let recovery_id = v.checked_sub(27).ok_or(ErrorCode::InvalidSignature)?;
    // SOL-LOW-002: a valid secp256k1 recovery_id is 0 or 1. v=27 -> 0,
    // v=28 -> 1. Anything else is malformed; reject fast.
    require!(recovery_id <= 1, ErrorCode::InvalidRecoveryId);

    let s_high = is_high_s(signature);
    if s_high {
        return Err(ErrorCode::InvalidSignature.into());
    }

    let pubkey = secp256k1_recover(digest, recovery_id, &signature[..64])
        .map_err(|_| ErrorCode::InvalidSignature)?;
    Ok(pubkey.to_bytes())
}

/// Return true if the s-component of the signature is greater than half the
/// secp256k1 curve order (EIP-2 non-canonical high-s).
fn is_high_s(signature: &[u8; 65]) -> bool {
    // Half secp256k1 curve order N (N = HALF_N * 2 - 1; HALF_N = (N+1)/2), big-endian.
    const HALF_N: [u8; 32] = [
        0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0xFF, 0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46, 0x68, 0x1B,
        0x20, 0xA0,
    ];

    let s = &signature[32..64];
    // Lexicographic compare against HALF_N (both big-endian). If s > HALF_N, it's high-s.
    for i in 0..32 {
        if s[i] != HALF_N[i] {
            return s[i] > HALF_N[i];
        }
    }
    // s == HALF_N is low-s (the boundary itself is allowed by EIP-2).
    false
}

/// Validate the signed quote and mark the nonce consumed. Returns the route profile.
pub fn verify_quote_core(
    quote: &Quote,
    signature: &[u8; 65],
    payer: &Signer,
    payer_nonce: &mut Account<PayerNonce>,
    consumed_nonce: &mut Account<ConsumedNonce>,
    profiles: &Account<ProfilesIndex>,
    config: &Account<Config>,
) -> Result<RouteProfileEntry> {
    let clock = Clock::get()?;

    let digest = digest(&crate::ID, quote);
    let recovered = recover_signer(&digest, signature)?;
    require!(recovered == config.signer, ErrorCode::InvalidSigner);

    require!(
        clock.unix_timestamp <= quote.valid_until,
        ErrorCode::SignatureExpired
    );
    // SOL-LOW-004: bound quote lifetime so a misconfigured off-chain
    // signer cannot lock quotes effectively forever.
    require!(
        quote.valid_until <= clock.unix_timestamp + MAX_QUOTE_LIFETIME_SECONDS,
        ErrorCode::SignatureExpired
    );
    require!(!quote.payer.eq(&Pubkey::default()), ErrorCode::InvalidPayer);
    require!(quote.payer.eq(&payer.key()), ErrorCode::InvalidPayer);
    require!(
        !quote.merchant.eq(&Pubkey::default()),
        ErrorCode::ZeroMerchant
    );

    let profile = find_route_profile(&profiles.entries, &quote.route_id)?;
    require!(profile.enabled, ErrorCode::RouteDisabled);

    // SOL-LOW-005: enforce payer-nonce consistency BEFORE writing
    // payer_nonce.payer. If the existing payer_nonce.payer is non-default
    // it must match quote.payer; otherwise the first call must be the
    // first call ever for this payer (and the seed ensures that).
    if !payer_nonce.payer.eq(&Pubkey::default()) {
        require!(payer_nonce.payer == quote.payer, ErrorCode::InvalidPayer);
    }
    require!(quote.nonce == payer_nonce.nonce, ErrorCode::InvalidNonce);
    require!(!consumed_nonce.consumed, ErrorCode::NonceAlreadyConsumed);

    // Now it is safe to write payer_nonce state.
    if payer_nonce.payer.eq(&Pubkey::default()) {
        payer_nonce.payer = quote.payer;
    }
    let next_nonce = quote.nonce.checked_add(1).ok_or(ErrorCode::NonceOverflow)?;
    payer_nonce.nonce = next_nonce;

    consumed_nonce.payer = quote.payer;
    consumed_nonce.nonce = quote.nonce;
    consumed_nonce.consumed = true;

    Ok(profile.clone())
}

pub fn find_route_profile<'a>(
    entries: &'a [RouteProfileEntry],
    route_id: &[u8; 32],
) -> Result<&'a RouteProfileEntry> {
    entries
        .iter()
        .find(|e| e.route_id == *route_id)
        .ok_or(ErrorCode::UnknownRoute.into())
}

/// Split a gross amount into merchant / treasury / IP creator legs.
pub fn split_gross(
    gross_amount: u64,
    profile: &RouteProfileEntry,
    ip_creator: &Pubkey,
) -> Result<(u64, u64, u64)> {
    require!(gross_amount > 0, ErrorCode::ZeroAmount);

    let treasury_amt = if profile.treasury_bps > 0 {
        let fee = (gross_amount as u128)
            .checked_mul(profile.treasury_bps as u128)
            .ok_or(ErrorCode::PaymentTooSmallForTreasury)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::PaymentTooSmallForTreasury)? as u64;
        require!(fee > 0, ErrorCode::PaymentTooSmallForTreasury);
        fee
    } else {
        0
    };

    let ip_amt = if profile.ip_creator_bps > 0 {
        require!(
            !ip_creator.eq(&Pubkey::default()),
            ErrorCode::MissingIPCreator
        );
        let fee = (gross_amount as u128)
            .checked_mul(profile.ip_creator_bps as u128)
            .ok_or(ErrorCode::PaymentTooSmallForRoyalty)?
            .checked_div(BPS_DENOMINATOR as u128)
            .ok_or(ErrorCode::PaymentTooSmallForRoyalty)? as u64;
        require!(fee > 0, ErrorCode::PaymentTooSmallForRoyalty);
        fee
    } else {
        0
    };

    let merchant_amt = gross_amount
        .checked_sub(treasury_amt)
        .and_then(|v| v.checked_sub(ip_amt))
        .ok_or(ErrorCode::FeeExceedsGross)?;
    require!(merchant_amt > 0, ErrorCode::ZeroAmount);

    Ok((merchant_amt, treasury_amt, ip_amt))
}

/// Emit the canonical Payment event and compute its payment_id.
pub fn emit_payment(
    quote: &Quote,
    merchant_amt: u64,
    treasury_amt: u64,
    ip_amt: u64,
) -> Result<()> {
    let payment_id = solana_program::hash::hashv(&[
        quote.payer.as_ref(),
        quote.merchant.as_ref(),
        quote.token.as_ref(),
        &quote.gross_amount.to_le_bytes(),
        quote.ip_creator.as_ref(),
        &quote.valid_until.to_le_bytes(),
        &quote.order_id_hash,
        &quote.nonce.to_le_bytes(),
        &quote.route_id,
    ])
    .to_bytes();

    emit!(Payment {
        payment_id,
        payer: quote.payer,
        merchant: quote.merchant,
        token: quote.token,
        gross_amount: quote.gross_amount,
        merchant_amount: merchant_amt,
        treasury_amount: treasury_amt,
        ip_creator_amount: ip_amt,
        valid_until: quote.valid_until,
        route_id: quote.route_id,
        order_id_hash: quote.order_id_hash,
    });
    Ok(())
}

pub fn emit_treasury_updated(new_treasury: Pubkey) {
    emit!(TreasuryUpdated { new_treasury });
}
