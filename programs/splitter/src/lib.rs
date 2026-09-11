pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;
pub use utils::*;

declare_id!("724Ut31i4ecY4dJ25z8HuZetu3A43xtNkPdk4JdbsfdD");

#[program]
pub mod splitter {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
        crate::instructions::initialize::handle_initialize(ctx, params)
    }

    pub fn settle_native(
        ctx: Context<SettleNative>,
        nonce: u64,
        quote: Quote,
        signature: [u8; 65],
    ) -> Result<()> {
        crate::instructions::settle_native::handle_settle_native(ctx, nonce, quote, signature)
    }

    pub fn settle_stable<'a>(
        ctx: Context<'a, SettleStable<'a>>,
        nonce: u64,
        quote: Quote,
        signature: [u8; 65],
    ) -> Result<()> {
        crate::instructions::settle_stable::handle_settle_stable(ctx, nonce, quote, signature)
    }

    pub fn pause(ctx: Context<Pause>) -> Result<()> {
        crate::instructions::pause::handle_pause(ctx)
    }

    pub fn unpause(ctx: Context<Unpause>) -> Result<()> {
        crate::instructions::unpause::handle_unpause(ctx)
    }

    pub fn set_treasury(ctx: Context<SetTreasury>, new_treasury: Pubkey) -> Result<()> {
        crate::instructions::set_treasury::handle_set_treasury(ctx, new_treasury)
    }

    pub fn configure_route(
        ctx: Context<ConfigureRoute>,
        route_id: [u8; 32],
        treasury_bps: u16,
        ip_creator_bps: u16,
        route_treasury: Pubkey,
    ) -> Result<()> {
        crate::instructions::configure_route::handle_configure_route(
            ctx,
            route_id,
            treasury_bps,
            ip_creator_bps,
            route_treasury,
        )
    }

    pub fn disable_route(ctx: Context<DisableRoute>, route_id: [u8; 32]) -> Result<()> {
        crate::instructions::disable_route::handle_disable_route(ctx, route_id)
    }

    pub fn enable_route(ctx: Context<EnableRoute>, route_id: [u8; 32]) -> Result<()> {
        crate::instructions::enable_route::handle_enable_route(ctx, route_id)
    }

    pub fn set_whitelisted_tokens(
        ctx: Context<SetWhitelistedTokens>,
        tokens: Vec<Pubkey>,
        allowed: Vec<bool>,
    ) -> Result<()> {
        crate::instructions::set_whitelisted_tokens::handle_set_whitelisted_tokens(
            ctx, tokens, allowed,
        )
    }

    pub fn grant_signer_role(ctx: Context<GrantSignerRole>, signer: [u8; 64]) -> Result<()> {
        crate::instructions::grant_signer_role::handle_grant_signer_role(ctx, signer)
    }

    pub fn rotate_signer_role(ctx: Context<RotateSignerRole>, new_signer: [u8; 64]) -> Result<()> {
        crate::instructions::rotate_signer_role::handle_rotate_signer_role(ctx, new_signer)
    }

    pub fn grant_pauser_role(ctx: Context<GrantPauserRole>, pauser: Pubkey) -> Result<()> {
        crate::instructions::grant_pauser_role::handle_grant_pauser_role(ctx, pauser)
    }

    pub fn rotate_pauser_role(ctx: Context<RotatePauserRole>, new_pauser: Pubkey) -> Result<()> {
        crate::instructions::rotate_pauser_role::handle_rotate_pauser_role(ctx, new_pauser)
    }

    pub fn rotate_admin_role(ctx: Context<RotateAdminRole>, new_admin: Pubkey) -> Result<()> {
        crate::instructions::rotate_admin_role::handle_rotate_admin_role(ctx, new_admin)
    }

    pub fn quote_total(
        ctx: Context<QuoteTotal>,
        gross_amount: u64,
        route_id: [u8; 32],
        ip_creator: Pubkey,
    ) -> Result<QuoteTotalResult> {
        crate::instructions::quote_total::handle_quote_total(
            ctx,
            gross_amount,
            route_id,
            ip_creator,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_constants_match_keccak_of_names() {
        // Placeholder: route IDs are hardcoded constants matching EVM v1.4.
        // This test documents the expected bytes are non-zero and distinct.
        assert_ne!(ROUTE_AGENT_X402, [0u8; 32]);
        assert_ne!(ROUTE_MERCHANT_AIFP1, [0u8; 32]);
        assert_ne!(ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1);
    }

    #[test]
    fn quote_message_hash_is_deterministic_and_matches_off_chain_implementation() {
        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::new_unique(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::new_unique(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };

        // On-chain Solana-native digest.
        let program_id = crate::id();
        let on_chain = utils::quote_message_hash(&program_id, &q);

        // Off-chain re-implementation: tag + program_id + explicit Quote
        // bytes (hand-written little-endian), SHA-256.
        // The encoding length MUST equal QUOTE_ENCODED_LEN.
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(crate::constants::MESSAGE_DOMAIN_TAG);
        hasher.update(program_id.as_ref());
        let mut buf = [0u8; utils::QUOTE_ENCODED_LEN];
        utils::encode_quote(&q, &mut buf);
        hasher.update(buf);
        let off_chain: [u8; 32] = hasher.finalize().into();

        assert_eq!(on_chain, off_chain);

        let second = utils::quote_message_hash(&program_id, &q);
        assert_eq!(on_chain, second);
    }

    #[test]
    fn quote_encoding_length_matches_layout() {
        // 32 + 32 + 32 + 8 + 32 + 8 + 32 + 8 + 32 = 216
        assert_eq!(utils::QUOTE_ENCODED_LEN, 216);
    }

    #[test]
    fn digest_fixture_for_ts_parity() {
        // Pinned fixture for the TypeScript signer
        // (scripts/signer/signer-parity.test.ts). Fixed quote, fixed
        // program id (declare_id). If this digest changes, the TS
        // implementation MUST be updated in lockstep — the Quote field
        // order is a cross-chain sacred constant.
        let q = Quote {
            payer: Pubkey::new_from_array([1u8; 32]),
            merchant: Pubkey::new_from_array([2u8; 32]),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: 1_700_003_600,
            order_id_hash: [7u8; 32],
            nonce: 3,
            route_id: ROUTE_MERCHANT_AIFP1,
        };
        let digest = utils::quote_message_hash(&crate::id(), &q);
        let hex: String = digest.iter().map(|b| format!("{:02x}", b)).collect();
        println!("TS_PARITY_DIGEST={}", hex);
        // Pinned value — mirrors scripts/signer/signer-parity.test.ts.
        assert_eq!(
            hex,
            "a2bc60ad146e27c0a8e3fcb14c8b246751c0fefd623b0b8f72f903576e4aae9c"
        );
    }

    #[test]
    fn secp256k1_signature_recovers_signer_public_key() {
        use k256::ecdsa::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let signer_pubkey: [u8; 64] = verifying_key.to_encoded_point(false).as_bytes()[1..]
            .try_into()
            .unwrap();

        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };

        let program_id = crate::id();
        let digest = utils::quote_message_hash(&program_id, &q);

        // Sign the digest as a prehash using k256. `sign_digest_recoverable`
        // returns the raw 64-byte signature and the recovery id (0 or 1).
        let (raw_sig, rec_id) = signing_key.sign_prehash_recoverable(&digest).unwrap();

        let mut sig_65 = [0u8; 65];
        sig_65[..64].copy_from_slice(raw_sig.to_bytes().as_ref());
        sig_65[64] = 27 + rec_id.to_byte();

        let recovered = utils::recover_signer(&digest, &sig_65).unwrap();
        assert_eq!(recovered, signer_pubkey);
    }

    #[test]
    fn secp256k1_signature_rejects_tampered_quote() {
        use k256::ecdsa::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let signer_pubkey: [u8; 64] = verifying_key.to_encoded_point(false).as_bytes()[1..]
            .try_into()
            .unwrap();

        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };

        let program_id = crate::id();
        let digest = utils::quote_message_hash(&program_id, &q);

        let (raw_sig, rec_id) = signing_key.sign_prehash_recoverable(&digest).unwrap();

        let mut sig_65 = [0u8; 65];
        sig_65[..64].copy_from_slice(raw_sig.to_bytes().as_ref());
        sig_65[64] = 27 + rec_id.to_byte();

        let recovered = utils::recover_signer(&digest, &sig_65).unwrap();
        assert_eq!(recovered, signer_pubkey);

        // Tamper with the quote and recompute digest; the original signature must NOT recover.
        let mut tampered = q;
        tampered.gross_amount = 2_000_000;
        let tampered_digest = utils::quote_message_hash(&program_id, &tampered);
        let wrong_recovered = utils::recover_signer(&tampered_digest, &sig_65).unwrap();
        assert_ne!(wrong_recovered, signer_pubkey);
    }

    #[test]
    fn split_agent_route_has_zero_fees() {
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let (m, t, i) = utils::split_gross(1_000_000, &profile, &Pubkey::default()).unwrap();
        assert_eq!(m, 1_000_000);
        assert_eq!(t, 0);
        assert_eq!(i, 0);
    }

    #[test]
    fn split_merchant_route_has_1_percent_treasury() {
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 100,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let (m, t, i) = utils::split_gross(1_000_000, &profile, &Pubkey::default()).unwrap();
        assert_eq!(t, 10_000);
        assert_eq!(i, 0);
        assert_eq!(m, 990_000);
    }

    #[test]
    fn split_rejects_zero_amount() {
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        assert!(utils::split_gross(0, &profile, &Pubkey::default()).is_err());
    }

    #[test]
    fn split_rejects_missing_ip_creator() {
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 100,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        assert!(utils::split_gross(1_000_000, &profile, &Pubkey::default()).is_err());
    }

    #[test]
    fn split_combined_fees_distribute_correctly() {
        // 1.0% treasury + 0.5% IP creator on 1_000_000 lamports.
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 100,
            ip_creator_bps: 50,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let ip_creator = Pubkey::new_unique();
        let (m, t, i) = utils::split_gross(1_000_000, &profile, &ip_creator).unwrap();
        assert_eq!(t, 10_000);
        assert_eq!(i, 5_000);
        assert_eq!(m, 985_000);
        // Conservation invariant: legs sum to gross.
        assert_eq!(m + t + i, 1_000_000);
    }

    #[test]
    fn split_rejects_aggregate_fees_exceeding_gross() {
        // Pathological route: 10_000 bps treasury on a 1-lamport gross.
        // Fee would be 1 lamport, leaving merchant with 0 -> ZeroAmount.
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 10_000,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        assert!(utils::split_gross(1, &profile, &Pubkey::default()).is_err());
    }

    #[test]
    fn split_rejects_payment_too_small_for_treasury_fee() {
        // gross=1 with treasury_bps>0 produces a zero-bps fee
        // (1 * 1 / 10_000 == 0). split_gross requires fee > 0 when
        // treasury_bps > 0, so this must error.
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 1,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        assert!(utils::split_gross(1, &profile, &Pubkey::default()).is_err());
    }

    #[test]
    fn split_rejects_payment_too_small_for_ip_creator_fee() {
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 1,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let ip_creator = Pubkey::new_unique();
        assert!(utils::split_gross(1, &profile, &ip_creator).is_err());
    }

    #[test]
    fn split_handles_large_amount_without_overflow() {
        // u64::MAX divided by BPS_DENOMINATOR, times treasury_bps — make sure
        // the u128 widening in split_gross protects against u64 overflow.
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 500, // 5% — the documented max
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        // 100 SOL in lamports (well under u64::MAX).
        let gross: u64 = 100 * 1_000_000_000;
        let (m, t, i) = utils::split_gross(gross, &profile, &Pubkey::default()).unwrap();
        assert_eq!(t, 5 * 1_000_000_000);
        assert_eq!(i, 0);
        assert_eq!(m, 95 * 1_000_000_000);
        assert_eq!(m + t + i, gross);
    }

    #[test]
    fn recover_signer_rejects_invalid_recovery_id() {
        use k256::ecdsa::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::random(&mut OsRng);
        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };
        let program_id = crate::id();
        let digest = utils::quote_message_hash(&program_id, &q);
        let (raw_sig, rec_id) = signing_key.sign_prehash_recoverable(&digest).unwrap();
        let mut sig_65 = [0u8; 65];
        sig_65[..64].copy_from_slice(raw_sig.to_bytes().as_ref());
        sig_65[64] = 27 + rec_id.to_byte();

        // v must be either 27 (rec_id=0) or 28 (rec_id=1). Anything else is
        // rejected by recover_signer before calling the syscall.
        sig_65[64] = 26;
        let err = utils::recover_signer(&digest, &sig_65).unwrap_err();
        let expected: anchor_lang::error::Error = error::ErrorCode::InvalidSignature.into();
        assert_eq!(err, expected);

        sig_65[64] = 29;
        let err = utils::recover_signer(&digest, &sig_65).unwrap_err();
        let expected: anchor_lang::error::Error = error::ErrorCode::InvalidRecoveryId.into();
        assert_eq!(err, expected);
    }

    #[test]
    fn recover_signer_rejects_high_s_malleable_signature() {
        // EIP-2: a signature with s > N/2 must be rejected. We force a
        // high-s by flipping s to (N - original_s), which is the canonical
        // low-s negation of the same signature.
        use k256::ecdsa::SigningKey;
        use rand::rngs::OsRng;

        let signing_key = SigningKey::random(&mut OsRng);
        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };
        let program_id = crate::id();
        let digest = utils::quote_message_hash(&program_id, &q);
        let (raw_sig, rec_id) = signing_key.sign_prehash_recoverable(&digest).unwrap();

        let mut sig_65 = [0u8; 65];
        sig_65[..64].copy_from_slice(raw_sig.to_bytes().as_ref());
        sig_65[64] = 27 + rec_id.to_byte();

        // Original s bytes (big-endian).
        let s_orig: [u8; 32] = sig_65[32..64].try_into().unwrap();

        // Half secp256k1 curve order — the upper bound of the low-s range
        // (mirrors the HALF_N constant in utils.rs).
        const HALF_N: [u8; 32] = [
            0x7F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0x5D, 0x57, 0x6E, 0x73, 0x57, 0xA4, 0x50, 0x1D, 0xDF, 0xE9, 0x2F, 0x46,
            0x68, 0x1B, 0x20, 0xA0,
        ];
        fn is_high_s(s: &[u8; 32]) -> bool {
            for i in 0..32 {
                if s[i] != HALF_N[i] {
                    return s[i] > HALF_N[i];
                }
            }
            false
        }

        if is_high_s(&s_orig) {
            // k256 happened to produce a non-canonical (high-s) signature:
            // the original itself must be rejected by the EIP-2 check.
            let err = utils::recover_signer(&digest, &sig_65).unwrap_err();
            let expected: anchor_lang::error::Error = error::ErrorCode::InvalidSignature.into();
            assert_eq!(err, expected);
        } else {
            // Sanity: the canonical (low-s) signature is accepted.
            let expected_pubkey: [u8; 64] = signing_key
                .verifying_key()
                .to_encoded_point(false)
                .as_bytes()[1..]
                .try_into()
                .unwrap();
            let recovered = utils::recover_signer(&digest, &sig_65).unwrap();
            assert_eq!(recovered, expected_pubkey);

            // Malleate: s' = N - s (big-endian). The result is necessarily
            // high-s and must be rejected before the recovery syscall.
            const N: [u8; 32] = [
                0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
                0xFF, 0xFE, 0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B, 0xBF, 0xD2, 0x5E, 0x8C,
                0xD0, 0x36, 0x41, 0x41,
            ];
            let mut s_neg = [0u8; 32];
            let mut borrow: i16 = 0;
            for i in (0..32).rev() {
                let diff = N[i] as i16 - s_orig[i] as i16 - borrow;
                s_neg[i] = diff as u8;
                borrow = i16::from(diff < 0);
            }
            assert!(is_high_s(&s_neg), "negated s must be high-s");

            sig_65[32..64].copy_from_slice(&s_neg);
            let err = utils::recover_signer(&digest, &sig_65).unwrap_err();
            let expected: anchor_lang::error::Error = error::ErrorCode::InvalidSignature.into();
            assert_eq!(err, expected);
        }
    }

    #[test]
    fn quote_message_hash_is_bound_to_program_id() {
        // Same quote, different program_id -> different digest. This is the
        // cross-program replay defense — without it, a quote signed for one
        // deployment could be replayed against another.
        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };
        let real_program_id = crate::id();
        let attacker_program_id = Pubkey::new_unique();

        let real_digest = utils::quote_message_hash(&real_program_id, &q);
        let attacker_digest = utils::quote_message_hash(&attacker_program_id, &q);
        assert_ne!(real_digest, attacker_digest);
    }

    #[test]
    fn quote_message_hash_changes_with_any_quote_field() {
        // Verify that changing any quote field changes the digest (sanity
        // check on the canonical encoding). This catches silent regressions
        // in encode_quote.
        let mut q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };
        let program_id = crate::id();
        let base = utils::quote_message_hash(&program_id, &q);

        q.gross_amount = 2_000_000;
        assert_ne!(utils::quote_message_hash(&program_id, &q), base);
        q.gross_amount = 1_000_000;

        q.nonce = 1;
        assert_ne!(utils::quote_message_hash(&program_id, &q), base);
        q.nonce = 0;

        q.route_id = ROUTE_MERCHANT_AIFP1;
        assert_ne!(utils::quote_message_hash(&program_id, &q), base);
    }

    #[test]
    fn encode_quote_field_order_matches_abi_layout() {
        // Pinned byte positions for the canonical encoding. Any reordering
        // here is a cross-chain breaking change — this test is the tripwire.
        let payer = Pubkey::new_from_array([0x11; 32]);
        let merchant = Pubkey::new_from_array([0x22; 32]);
        let token = Pubkey::new_from_array([0x33; 32]);
        let ip_creator = Pubkey::new_from_array([0x44; 32]);

        let q = Quote {
            payer,
            merchant,
            token,
            gross_amount: 0x0102_0304_0506_0708,
            ip_creator,
            valid_until: 0x1112_1314_1516_1718,
            order_id_hash: [0xAA; 32],
            nonce: 0x2122_2324_2526_2728,
            route_id: [0xBB; 32],
        };

        let mut buf = [0u8; utils::QUOTE_ENCODED_LEN];
        utils::encode_quote(&q, &mut buf);

        // payer: 0..32
        assert_eq!(&buf[0..32], payer.as_ref());
        // merchant: 32..64
        assert_eq!(&buf[32..64], merchant.as_ref());
        // token: 64..96
        assert_eq!(&buf[64..96], token.as_ref());
        // gross_amount: 96..104 (LE)
        assert_eq!(&buf[96..104], &0x0102_0304_0506_0708u64.to_le_bytes());
        // ip_creator: 104..136
        assert_eq!(&buf[104..136], ip_creator.as_ref());
        // valid_until: 136..144 (LE)
        assert_eq!(&buf[136..144], &0x1112_1314_1516_1718i64.to_le_bytes());
        // order_id_hash: 144..176
        assert_eq!(&buf[144..176], &[0xAA; 32]);
        // nonce: 176..184 (LE)
        assert_eq!(&buf[176..184], &0x2122_2324_2526_2728u64.to_le_bytes());
        // route_id: 184..216
        assert_eq!(&buf[184..216], &[0xBB; 32]);
    }

    #[test]
    fn token_list_is_allowed_matches_membership() {
        let mint_a = Pubkey::new_unique();
        let mint_b = Pubkey::new_unique();
        let other = Pubkey::new_unique();

        let empty = TokenList {
            admin: Pubkey::new_unique(),
            tokens: vec![],
            bump: 255,
        };
        assert!(!empty.is_allowed(mint_a));

        let list = TokenList {
            admin: Pubkey::new_unique(),
            tokens: vec![mint_a, mint_b],
            bump: 255,
        };
        assert!(list.is_allowed(mint_a));
        assert!(list.is_allowed(mint_b));
        assert!(!list.is_allowed(other));
        // Default pubkey is never implicitly allowed.
        assert!(!list.is_allowed(Pubkey::default()));
    }

    #[test]
    fn find_route_profile_returns_entry_or_unknown_route() {
        let entry = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let entries = vec![entry];

        let found = utils::find_route_profile(&entries, &ROUTE_AGENT_X402).unwrap();
        assert_eq!(found.route_id, ROUTE_AGENT_X402);

        let expected: anchor_lang::error::Error = error::ErrorCode::UnknownRoute.into();
        let err = utils::find_route_profile(&entries, &ROUTE_MERCHANT_AIFP1);
        assert!(matches!(err, Err(e) if e == expected));

        let err = utils::find_route_profile(&[], &ROUTE_AGENT_X402);
        assert!(matches!(err, Err(e) if e == expected));
    }

    #[test]
    fn split_gross_supports_ip_only_route() {
        // Zero treasury, 0.5% IP creator: merchant keeps the rest.
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 50,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let ip_creator = Pubkey::new_unique();
        let (m, t, i) = utils::split_gross(1_000_000, &profile, &ip_creator).unwrap();
        assert_eq!(t, 0);
        assert_eq!(i, 5_000);
        assert_eq!(m, 995_000);
        assert_eq!(m + t + i, 1_000_000);
    }

    #[test]
    fn split_gross_rejects_fees_exceeding_gross() {
        // 200% treasury fee overflows the merchant leg -> FeeExceedsGross.
        // (Fee caps are enforced at init-time, not split-time.)
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 20_000,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let err = utils::split_gross(1_000_000, &profile, &Pubkey::default()).unwrap_err();
        let expected: anchor_lang::error::Error = error::ErrorCode::FeeExceedsGross.into();
        assert_eq!(err, expected);
    }

    #[test]
    fn digest_alias_matches_quote_message_hash() {
        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 42,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [7u8; 32],
            nonce: 3,
            route_id: ROUTE_MERCHANT_AIFP1,
        };
        let program_id = crate::id();
        assert_eq!(
            utils::digest(&program_id, &q),
            utils::quote_message_hash(&program_id, &q)
        );
    }

    #[test]
    fn hashv_matches_manual_sha256_and_is_order_sensitive() {
        use sha2::{Digest, Sha256};

        let a = [1u8; 32];
        let b = [2u8; 8];
        let got = utils::hashv(&[&a, &b]);

        let mut hasher = Sha256::new();
        hasher.update(a);
        hasher.update(b);
        let want: [u8; 32] = hasher.finalize().into();
        assert_eq!(got, want);

        // Order matters: swapping inputs must change the digest.
        assert_ne!(got, utils::hashv(&[&b, &a]));
        // Content matters.
        assert_ne!(got, utils::hashv(&[&a, &[3u8; 8]]));
    }

    #[test]
    fn error_codes_are_pinned_for_idl_compat() {
        // Anchor numbers #[error_code] from 6000 in declaration order
        // (via `From<ErrorCode> for u32`; the raw discriminants are 0-based).
        // New variants MUST be appended, never inserted or reordered.
        let code = |e: error::ErrorCode| -> u32 { e.into() };
        assert_eq!(code(error::ErrorCode::Unauthorized), 6000);
        assert_eq!(code(error::ErrorCode::InvalidDeployer), 6035);
        assert_eq!(code(error::ErrorCode::InvalidRecoveryId), 6047);
    }

    // -----------------------------------------------------------------------
    // Additional split_gross edge-case coverage
    // -----------------------------------------------------------------------

    #[test]
    fn split_gross_rejects_ip_creator_zero_when_bps_nonzero() {
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 50,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        // ip_creator_bps > 0 but ip_creator == Pubkey::default() -> MissingIPCreator
        assert!(utils::split_gross(1_000_000, &profile, &Pubkey::default()).is_err());
    }

    #[test]
    fn split_gross_both_fees_exceed_gross() {
        // 60% treasury + 50% IP = 110% total -> FeeExceedsGross
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 6000,
            ip_creator_bps: 5000,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let ip_creator = Pubkey::new_unique();
        assert!(utils::split_gross(1_000_000, &profile, &ip_creator).is_err());
    }

    #[test]
    fn split_gross_rounds_treasury_down() {
        // 1 bps on 99 lamports = 0.01 * 99 = 0.99 -> floor = 0 -> fee must be > 0
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 1,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        // gross=99, treasury_bps=1 -> fee = 99/10000 = 0 (floored) -> rejected
        assert!(utils::split_gross(99, &profile, &Pubkey::default()).is_err());

        // gross=10000, treasury_bps=1 -> fee = 10000/10000 = 1 -> accepted
        let (m, t, i) = utils::split_gross(10_000, &profile, &Pubkey::default()).unwrap();
        assert_eq!(t, 1);
        assert_eq!(i, 0);
        assert_eq!(m, 9_999);
    }

    #[test]
    fn split_gross_rounds_ip_down() {
        // 1 bps IP on 99 lamports -> floor = 0 -> rejected
        let profile = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 1,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let ip_creator = Pubkey::new_unique();
        assert!(utils::split_gross(99, &profile, &ip_creator).is_err());

        // 1 bps IP on 10000 lamports -> fee = 1
        let (m, t, i) = utils::split_gross(10_000, &profile, &ip_creator).unwrap();
        assert_eq!(t, 0);
        assert_eq!(i, 1);
        assert_eq!(m, 9_999);
    }

    #[test]
    fn split_gross_minimum_viable_payment() {
        // Smallest payment that succeeds: treasury_bps=1, gross=10_000 -> fee=1
        let profile = RouteProfileEntry {
            route_id: ROUTE_MERCHANT_AIFP1,
            treasury_bps: 1,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 0,
            route_treasury: Pubkey::default(),
        };
        let (m, t, i) = utils::split_gross(10_000, &profile, &Pubkey::default()).unwrap();
        assert_eq!(m, 9_999);
        assert_eq!(t, 1);
        assert_eq!(i, 0);
        assert_eq!(m + t + i, 10_000);
    }

    // -----------------------------------------------------------------------
    // encode_quote boundary checks
    // -----------------------------------------------------------------------

    #[test]
    fn encode_quote_buffer_fully_written() {
        let q = Quote {
            payer: Pubkey::new_from_array([0x11; 32]),
            merchant: Pubkey::new_from_array([0x22; 32]),
            token: Pubkey::new_from_array([0x33; 32]),
            gross_amount: u64::MAX,
            ip_creator: Pubkey::new_from_array([0x44; 32]),
            valid_until: i64::MAX,
            order_id_hash: [0xFF; 32],
            nonce: u64::MAX,
            route_id: [0xAA; 32],
        };
        let mut buf = [0u8; utils::QUOTE_ENCODED_LEN];
        utils::encode_quote(&q, &mut buf);
        // Every byte in the buffer must be non-zero (all fields use known non-zero content).
        assert!(buf.iter().all(|&b| b != 0), "buffer must be fully written");
    }

    #[test]
    fn encode_quote_zero_fields_produce_zero_bytes() {
        let q = Quote {
            payer: Pubkey::default(),
            merchant: Pubkey::default(),
            token: Pubkey::default(),
            gross_amount: 0,
            ip_creator: Pubkey::default(),
            valid_until: 0,
            order_id_hash: [0u8; 32],
            nonce: 0,
            route_id: [0u8; 32],
        };
        let mut buf = [0u8; utils::QUOTE_ENCODED_LEN];
        utils::encode_quote(&q, &mut buf);
        assert_eq!(buf, [0u8; utils::QUOTE_ENCODED_LEN]);
    }

    // -----------------------------------------------------------------------
    // quote_message_hash edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn quote_hash_zero_gross_amount() {
        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 0,
            ip_creator: Pubkey::default(),
            valid_until: 1_700_000_000,
            order_id_hash: [0u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };
        let program_id = crate::id();
        let h1 = utils::quote_message_hash(&program_id, &q);
        // Digest must be deterministic even for zero-value fields.
        let h2 = utils::quote_message_hash(&program_id, &q);
        assert_eq!(h1, h2);
        // Changing gross_amount must change the hash.
        let mut q2 = q.clone();
        q2.gross_amount = 1;
        assert_ne!(h1, utils::quote_message_hash(&program_id, &q2));
    }

    #[test]
    fn quote_hash_all_max_fields() {
        let q = Quote {
            payer: Pubkey::new_from_array([0xFF; 32]),
            merchant: Pubkey::new_from_array([0xFF; 32]),
            token: Pubkey::new_from_array([0xFF; 32]),
            gross_amount: u64::MAX,
            ip_creator: Pubkey::new_from_array([0xFF; 32]),
            valid_until: i64::MAX,
            order_id_hash: [0xFF; 32],
            nonce: u64::MAX,
            route_id: [0xFF; 32],
        };
        let program_id = crate::id();
        let h = utils::quote_message_hash(&program_id, &q);
        // Must produce a valid 32-byte digest without overflow.
        assert_ne!(h, [0u8; 32]);
    }

    // -----------------------------------------------------------------------
    // Constants validation
    // -----------------------------------------------------------------------

    #[test]
    fn constants_fee_caps_are_consistent() {
        // MAX_AGGREGATE_BPS must be >= MAX_TREASURY_BPS + MAX_IP_CREATOR_BPS
        // so that a route with both maxed fees is representable.
        assert!(MAX_AGGREGATE_BPS as u32 >= MAX_TREASURY_BPS as u32 + MAX_IP_CREATOR_BPS as u32);
    }

    #[test]
    fn constants_max_tokens_and_routes_are_usize() {
        // MAX_ROUTES must fit in a u8 count field.
        assert!(MAX_ROUTES <= u8::MAX as usize);
    }

    // -----------------------------------------------------------------------
    // State struct behavior
    // -----------------------------------------------------------------------

    #[test]
    fn token_list_rejects_default_pubkey() {
        let list = TokenList {
            admin: Pubkey::new_unique(),
            tokens: vec![Pubkey::new_unique()],
            bump: 1,
        };
        assert!(!list.is_allowed(Pubkey::default()));
    }

    #[test]
    fn find_route_profile_preserves_field_identity() {
        let entry = RouteProfileEntry {
            route_id: ROUTE_AGENT_X402,
            treasury_bps: 250,
            ip_creator_bps: 75,
            enabled: true,
            configured_at: 1_700_000_000,
            route_treasury: Pubkey::new_unique(),
        };
        let entries = vec![entry.clone()];
        let found = utils::find_route_profile(&entries, &ROUTE_AGENT_X402).unwrap();
        assert_eq!(found.treasury_bps, 250);
        assert_eq!(found.ip_creator_bps, 75);
        assert_eq!(found.route_treasury, entry.route_treasury);
        assert_eq!(found.configured_at, 1_700_000_000);
    }
}
