use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct RotateSignerRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_rotate_signer_role(
    ctx: Context<RotateSignerRole>,
    new_signer: [u8; 64],
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(new_signer != [0u8; 64], ErrorCode::ZeroSigner);

    ctx.accounts.config.signer = new_signer;
    msg!("Signer role rotated");
    Ok(())
}
