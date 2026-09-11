use anchor_lang::prelude::*;

use crate::{
    constants::{MAX_AGGREGATE_BPS, MAX_IP_CREATOR_BPS, MAX_ROUTES, MAX_TREASURY_BPS},
    error::ErrorCode,
    state::{Config, ProfilesIndex, RouteProfileEntry},
};

#[derive(Accounts)]
pub struct ConfigureRoute<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    #[account(mut, seeds = [crate::constants::PROFILES_INDEX_SEED], bump = profiles.bump)]
    pub profiles: Box<Account<'info, ProfilesIndex>>,

    pub admin: Signer<'info>,
}

pub fn handle_configure_route(
    ctx: Context<ConfigureRoute>,
    route_id: [u8; 32],
    treasury_bps: u16,
    ip_creator_bps: u16,
    route_treasury: Pubkey,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(
        treasury_bps <= MAX_TREASURY_BPS,
        ErrorCode::TreasuryFeeTooHigh
    );
    require!(
        ip_creator_bps <= MAX_IP_CREATOR_BPS,
        ErrorCode::IPCreatorFeeTooHigh
    );
    let aggregate_bps = treasury_bps as u32 + ip_creator_bps as u32;
    require!(
        aggregate_bps <= MAX_AGGREGATE_BPS as u32,
        ErrorCode::AggregateFeeTooHigh
    );

    let clock = Clock::get()?;
    let profiles = &mut ctx.accounts.profiles;

    if let Some(entry) = profiles.entries.iter_mut().find(|e| e.route_id == route_id) {
        entry.treasury_bps = treasury_bps;
        entry.ip_creator_bps = ip_creator_bps;
        entry.route_treasury = route_treasury;
        entry.configured_at = clock.unix_timestamp;
    } else {
        require!(profiles.entries.len() < MAX_ROUTES, ErrorCode::UnknownRoute);
        profiles.entries.push(RouteProfileEntry {
            route_id,
            treasury_bps,
            ip_creator_bps,
            enabled: true,
            configured_at: clock.unix_timestamp,
            route_treasury,
        });
        profiles.count = profiles.entries.len() as u8;
    }

    msg!("Route configured: route_id={:?}", route_id);
    Ok(())
}
