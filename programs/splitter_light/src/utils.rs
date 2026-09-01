use anchor_lang::prelude::*;
use solana_keccak_hasher::hashv as keccak256_hashv;

use crate::{
    constants::{
        BPS_DENOMINATOR, MAX_IP_CREATOR_BPS, MAX_TREASURY_BPS, MESSAGE_DOMAIN_TAG,
        ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1,
    },
    error::ErrorCode,
    state::{Config, ConsumedNonce, PayerNonce, Quote},
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

/// Route profile. Hardcoded in code — no on-chain registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteProfile {
    pub route_id: [u8; 32],
    pub treasury_bps: u16,
    pub ip_creator_bps: u16,
}

impl RouteProfile {
    pub const AGENT: Self = Self {
        route_id: ROUTE_AGENT_X402,
        treasury_bps: 0,
        ip_creator_bps: 0,
    };

    pub const MERCHANT: Self = Self {
        route_id: ROUTE_MERCHANT_AIFP1,
        treasury_bps: 100,
        ip_creator_bps: 0,
    };

    /// Sentinel used when no route matches.
    pub const UNKNOWN: Self = Self {
        route_id: [0u8; 32],
        treasury_bps: 0,
        ip_creator_bps: 0,
    };

    pub fn lookup(route_id: &[u8; 32]) -> Result<Self> {
        if *route_id == Self::AGENT.route_id {
            Ok(Self::AGENT)
        } else if *route_id == Self::MERCHANT.route_id {
            Ok(Self::MERCHANT)
        } else {
            err!(ErrorCode::UnknownRoute)
        }
    }
}

/// Compute the Solana-native signed message hash for a Quote.
///
/// This is NOT EIP-712. It uses:
/// - a versioned domain tag (not keccak),
/// - the Solana program_id as the domain,
/// - Borsh serialization of `Quote`,
/// - SHA-256 (Solana's native hash function).
///
/// Off-chain signers must implement the same construction.
pub fn quote_message_hash(program_id: &Pubkey, quote: &Quote) -> [u8; 32] {
    let mut quote_bytes = Vec::with_capacity(256);
    quote
        .serialize(&mut quote_bytes)
        .expect("Quote serialization failed");
    solana_program::hash::hashv(&[MESSAGE_DOMAIN_TAG, program_id.as_ref(), &quote_bytes]).to_bytes()
}

/// Canonical digest exposed for tests and off-chain signing compatibility.
/// Alias for [`quote_message_hash`].
pub fn digest(program_id: &Pubkey, quote: &Quote) -> [u8; 32] {
    quote_message_hash(program_id, quote)
}

/// Keccak256 hash of a Quote (kept for tests / backwards documentation only).
/// Prefer [`quote_message_hash`] for new signatures.
pub fn quote_hash(quote: &Quote) -> [u8; 32] {
    let mut quote_bytes = Vec::with_capacity(256);
    quote
        .serialize(&mut quote_bytes)
        .expect("Quote serialization failed");
    keccak256_hashv(&[&quote_bytes]).to_bytes()
}

/// Recover the secp256k1 public key from a 65-byte Ethereum-style signature.
pub fn recover_signer(digest: &[u8; 32], signature: &[u8; 65]) -> Result<[u8; 64]> {
    use solana_secp256k1_recover::secp256k1_recover;

    let v = signature[64];
    let recovery_id = v.checked_sub(27).ok_or(ErrorCode::InvalidSignature)?;
    let pubkey = secp256k1_recover(digest, recovery_id, signature)
        .map_err(|_| ErrorCode::InvalidSignature)?;
    Ok(pubkey.to_bytes())
}

/// Validate the signed quote and mark the nonce consumed. Returns the route profile.
pub fn verify_quote_core(
    quote: &Quote,
    signature: &[u8; 65],
    payer: &Signer,
    payer_nonce: &mut Account<PayerNonce>,
    consumed_nonce: &mut Account<ConsumedNonce>,
    config: &Account<Config>,
) -> Result<RouteProfile> {
    let clock = Clock::get()?;

    let digest = digest(&crate::ID, quote);
    let recovered = recover_signer(&digest, signature)?;
    require!(config.signer != [0u8; 64], ErrorCode::InvalidSigner);
    require!(recovered == config.signer, ErrorCode::InvalidSigner);

    require!(
        clock.unix_timestamp <= quote.valid_until,
        ErrorCode::SignatureExpired
    );
    require!(!quote.payer.eq(&Pubkey::default()), ErrorCode::InvalidPayer);
    require!(quote.payer.eq(&payer.key()), ErrorCode::InvalidPayer);
    require!(
        !quote.merchant.eq(&Pubkey::default()),
        ErrorCode::ZeroMerchant
    );

    let profile = RouteProfile::lookup(&quote.route_id)?;

    if payer_nonce.payer.eq(&Pubkey::default()) {
        payer_nonce.payer = quote.payer;
    }
    require!(quote.nonce == payer_nonce.nonce, ErrorCode::InvalidNonce);
    require!(!consumed_nonce.consumed, ErrorCode::NonceAlreadyConsumed);

    let next_nonce = quote.nonce.checked_add(1).ok_or(ErrorCode::NonceOverflow)?;
    payer_nonce.nonce = next_nonce;

    consumed_nonce.payer = quote.payer;
    consumed_nonce.nonce = quote.nonce;
    consumed_nonce.consumed = true;

    Ok(profile)
}

/// Split a gross amount into merchant / treasury / IP creator legs.
pub fn split_gross(gross_amount: u64, profile: &RouteProfile) -> Result<(u64, u64, u64)> {
    require!(gross_amount > 0, ErrorCode::ZeroAmount);
    require!(
        profile.treasury_bps <= MAX_TREASURY_BPS,
        ErrorCode::TreasuryFeeTooHigh
    );
    require!(
        profile.ip_creator_bps <= MAX_IP_CREATOR_BPS,
        ErrorCode::IPCreatorFeeTooHigh
    );

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

    let ip_amt = 0u64;

    let merchant_amt = gross_amount
        .checked_sub(treasury_amt)
        .and_then(|v| v.checked_sub(ip_amt))
        .ok_or(ErrorCode::ZeroAmount)?;
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
    let payment_id = keccak256_hashv(&[
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

/// Digest over the `set_signer` payload (the current signer and the new
/// signer pubkey). Caller signs this digest with secp256k1. Including
/// `current_signer` binds the signature to a specific rotation step,
/// preventing stale-signer replay.
///
/// This is NOT EIP-712. It uses a versioned domain tag + program_id +
/// SHA-256 over the concatenated signer bytes.
pub fn set_signer_digest(
    program_id: &Pubkey,
    current_signer: &[u8; 64],
    new_signer: &[u8; 64],
) -> [u8; 32] {
    solana_program::hash::hashv(&[
        MESSAGE_DOMAIN_TAG,
        b"-set-signer",
        program_id.as_ref(),
        current_signer,
        new_signer,
    ])
    .to_bytes()
}
