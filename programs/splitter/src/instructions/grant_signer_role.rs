use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct GrantSignerRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_grant_signer_role(ctx: Context<GrantSignerRole>, signer: [u8; 64]) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(signer != [0u8; 64], ErrorCode::ZeroSigner);
    require!(
        !ctx.accounts
            .admin
            .key()
            .eq(&Pubkey::new_from_array(signer[..32].try_into().unwrap())),
        ErrorCode::AdminEqualsSigner
    );
    require!(
        !config
            .pauser
            .eq(&Pubkey::new_from_array(signer[..32].try_into().unwrap())),
        ErrorCode::PauserEqualsSigner
    );

    ctx.accounts.config.signer = signer;
    msg!("Signer role granted");
    Ok(())
}
