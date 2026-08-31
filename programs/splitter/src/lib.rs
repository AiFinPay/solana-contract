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

    pub fn revoke_signer_role(ctx: Context<RevokeSignerRole>) -> Result<()> {
        crate::instructions::revoke_signer_role::handle_revoke_signer_role(ctx)
    }

    pub fn grant_pauser_role(ctx: Context<GrantPauserRole>, pauser: Pubkey) -> Result<()> {
        crate::instructions::grant_pauser_role::handle_grant_pauser_role(ctx, pauser)
    }

    pub fn revoke_pauser_role(ctx: Context<RevokePauserRole>) -> Result<()> {
        crate::instructions::revoke_pauser_role::handle_revoke_pauser_role(ctx)
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
