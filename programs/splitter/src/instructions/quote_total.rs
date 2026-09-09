use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::ProfilesIndex, utils::split_gross};

#[derive(Accounts)]
pub struct QuoteTotal<'info> {
    #[account(seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, crate::state::Config>,

    #[account(seeds = [crate::constants::PROFILES_INDEX_SEED], bump = profiles.bump)]
    pub profiles: Account<'info, ProfilesIndex>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct QuoteTotalResult {
    pub merchant_amount: u64,
    pub treasury_amount: u64,
    pub ip_creator_amount: u64,
    pub total_amount: u64,
}

pub fn handle_quote_total(
    ctx: Context<QuoteTotal>,
    gross_amount: u64,
    route_id: [u8; 32],
    ip_creator: Pubkey,
) -> Result<QuoteTotalResult> {
    // SOL-MED-002: respect protocol pause. Indexers should not receive
    // stale fee structures during incident response.
    require!(!ctx.accounts.config.is_paused, ErrorCode::ProtocolPaused);

    let entry = ctx
        .accounts
        .profiles
        .entries
        .iter()
        .find(|e| e.route_id == route_id)
        .ok_or(ErrorCode::UnknownRoute)?;
    require!(entry.enabled, ErrorCode::RouteDisabled);

    let (merchant_amount, treasury_amount, ip_creator_amount) =
        split_gross(gross_amount, entry, &ip_creator)?;

    Ok(QuoteTotalResult {
        merchant_amount,
        treasury_amount,
        ip_creator_amount,
        total_amount: gross_amount,
    })
}
