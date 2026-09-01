use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct RotatePauserRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_rotate_pauser_role(ctx: Context<RotatePauserRole>, new_pauser: Pubkey) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(!new_pauser.eq(&Pubkey::default()), ErrorCode::ZeroPauser);
    require!(!new_pauser.eq(&config.admin), ErrorCode::AdminEqualsSigner);

    ctx.accounts.config.pauser = new_pauser;
    msg!("Pauser role rotated");
    Ok(())
}
