use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct Pause<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub authority: Signer<'info>,
}

pub fn handle_pause(ctx: Context<Pause>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    let authority = ctx.accounts.authority.key();
    require!(
        config.admin.eq(&authority) || config.pauser.eq(&authority),
        ErrorCode::Unauthorized
    );
    config.is_paused = true;
    msg!("Splitter paused by {}", authority);
    Ok(())
}
