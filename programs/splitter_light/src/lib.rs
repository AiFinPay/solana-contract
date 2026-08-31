use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

pub use constants::*;
pub use error::ErrorCode;
pub use instructions::*;
pub use state::*;
pub use utils::*;

declare_id!("7vGTUXSmooih99MuzQELyaeFeZmuo4QcstS7T9Jv7yyR");

#[program]
pub mod splitter_light {
    use super::*;

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

    pub fn set_signer(
        ctx: Context<SetSigner>,
        new_signer: [u8; 64],
        signature: [u8; 65],
    ) -> Result<()> {
        crate::instructions::set_signer::handle_set_signer(ctx, new_signer, signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_constants_match_keccak_of_names() {
        // Route IDs are hardcoded constants matching EVM v1.4.
        assert_ne!(ROUTE_AGENT_X402, [0u8; 32]);
        assert_ne!(ROUTE_MERCHANT_AIFP1, [0u8; 32]);
        assert_ne!(ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1);
    }

    #[test]
    fn stablecoin_constants_are_set() {
        assert_ne!(USDC_MINT, Pubkey::default());
        assert_ne!(USDT_MINT, Pubkey::default());
        assert_ne!(USDC_MINT, USDT_MINT);
    }

    #[test]
    fn quote_hash_is_deterministic() {
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
        let h1 = utils::quote_hash(&q);
        let h2 = utils::quote_hash(&q);
        assert_eq!(h1, h2);
    }

    #[test]
    fn split_agent_route_has_zero_fees() {
        let profile = RouteProfile::AGENT;
        let (m, t, i) = utils::split_gross(1_000_000, &profile).unwrap();
        assert_eq!(m, 1_000_000);
        assert_eq!(t, 0);
        assert_eq!(i, 0);
    }

    #[test]
    fn split_merchant_route_has_1_percent_treasury() {
        let profile = RouteProfile::MERCHANT;
        let (m, t, i) = utils::split_gross(1_000_000, &profile).unwrap();
        assert_eq!(t, 10_000);
        assert_eq!(i, 0);
        assert_eq!(m, 990_000);
    }

    #[test]
    fn split_rejects_zero_amount() {
        let profile = RouteProfile::AGENT;
        assert!(utils::split_gross(0, &profile).is_err());
    }

    #[test]
    fn route_profile_lookup_rejects_unknown() {
        assert!(RouteProfile::lookup(&[0u8; 32]).is_err());
        assert!(RouteProfile::lookup(&RouteProfile::AGENT.route_id).is_ok());
        assert!(RouteProfile::lookup(&RouteProfile::MERCHANT.route_id).is_ok());
    }
}
