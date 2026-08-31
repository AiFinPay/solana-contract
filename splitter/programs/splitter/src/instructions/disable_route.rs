use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::{Config, ProfilesIndex}};

#[derive(Accounts)]
pub struct DisableRoute<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [crate::constants::PROFILES_INDEX_SEED], bump = profiles.bump)]
    pub profiles: Account<'info, ProfilesIndex>,

    pub admin: Signer<'info>,
}

pub fn handle_disable_route(ctx: Context<DisableRoute>, route_id: [u8; 32]) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(config.admin.eq(&ctx.accounts.admin.key()), ErrorCode::Unauthorized);

    let profiles = &mut ctx.accounts.profiles;
    let entry = profiles
        .entries
        .iter_mut()
        .find(|e| e.route_id == route_id)
        .ok_or(ErrorCode::UnknownRoute)?;
    entry.enabled = false;
    msg!("Route disabled: route_id={:?}", route_id);
    Ok(())
}
