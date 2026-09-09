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
    let caller = ctx.accounts.admin.key();
    // SOL-MED-004: symmetric pause/unpause to remove the DoS lever a
    // compromised pauser would otherwise hold. Both admin and pauser
    // can pause; both can unpause.
    require!(
        config.admin.eq(&caller) || config.pauser.eq(&caller),
        ErrorCode::Unauthorized
    );
    config.is_paused = false;
    msg!("Splitter unpaused by {}", caller);
    Ok(())
}
