use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct RevokePauserRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_revoke_pauser_role(ctx: Context<RevokePauserRole>) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    ctx.accounts.config.pauser = Pubkey::default();
    msg!("Pauser role revoked");
    Ok(())
}
