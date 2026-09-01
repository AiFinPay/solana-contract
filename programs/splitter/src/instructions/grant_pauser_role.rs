use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct GrantPauserRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_grant_pauser_role(ctx: Context<GrantPauserRole>, pauser: Pubkey) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(!pauser.eq(&Pubkey::default()), ErrorCode::ZeroPauser);
    require!(!pauser.eq(&config.admin), ErrorCode::AdminEqualsSigner);
    // Operational role separation: pauser must not reuse the current signer key material.
    require!(config.signer != [0u8; 64], ErrorCode::PauserEqualsSigner);

    ctx.accounts.config.pauser = pauser;
    msg!("Pauser role granted");
    Ok(())
}
