use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct Unpause<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_unpause(ctx: Context<Unpause>) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    config.is_paused = false;
    msg!("Splitter unpaused by {}", ctx.accounts.admin.key());
    Ok(())
}
