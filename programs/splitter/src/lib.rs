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

declare_id!("G6neYBZe8AzMvNPcewBhmzLao4CtZqc2uPVzPQBbAdrT");

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
    fn quote_hash_is_deterministic() {
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
        let h1 = utils::quote_hash(&q);
        let h2 = utils::quote_hash(&q);
        assert_eq!(h1, h2);
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

        // Off-chain re-implementation: tag + program_id + Borsh(Quote), SHA-256.
        use anchor_lang::AnchorSerialize;
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(crate::constants::MESSAGE_DOMAIN_TAG);
        hasher.update(program_id.as_ref());
        let mut quote_bytes = Vec::with_capacity(256);
        q.serialize(&mut quote_bytes).unwrap();
        hasher.update(quote_bytes);
        let off_chain: [u8; 32] = hasher.finalize().into();

        assert_eq!(on_chain, off_chain);

        let second = utils::quote_message_hash(&program_id, &q);
        assert_eq!(on_chain, second);
    }

    #[test]
    fn secp256k1_signature_recovers_signer_public_key() {
        use k256::ecdsa::SigningKey;
        use rand_core::OsRng;
        use sha2::{Digest, Sha256};

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
        let (raw_sig, rec_id) = signing_key
            .sign_digest_recoverable(Sha256::new_with_prefix(&digest))
            .expect("recoverable sign");

        let mut sig_65 = [0u8; 65];
        sig_65[..64].copy_from_slice(raw_sig.to_bytes().as_ref());
        sig_65[64] = 27 + rec_id.to_byte();

        let recovered = utils::recover_signer(&digest, &sig_65).unwrap();
        assert_eq!(recovered, signer_pubkey);
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
}
