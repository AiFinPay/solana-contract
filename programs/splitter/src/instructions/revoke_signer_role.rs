use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct RevokeSignerRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_revoke_signer_role(ctx: Context<RevokeSignerRole>) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    ctx.accounts.config.signer = [0u8; 64];
    msg!("Signer role revoked");
    Ok(())
}
