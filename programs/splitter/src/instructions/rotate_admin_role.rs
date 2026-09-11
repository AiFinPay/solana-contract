use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct RotateAdminRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    pub admin: Signer<'info>,
}

pub fn handle_rotate_admin_role(ctx: Context<RotateAdminRole>, new_admin: Pubkey) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(!new_admin.eq(&Pubkey::default()), ErrorCode::ZeroAdmin);
    // No role-separation check: admin, pauser and treasury may share one
    // address (e.g. a single Squads multisig vault). Separation, if wanted,
    // is an operational policy, not an on-chain invariant.

    ctx.accounts.config.admin = new_admin;
    msg!("Admin role rotated");
    Ok(())
}
