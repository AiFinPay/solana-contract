use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config};

#[derive(Accounts)]
pub struct GrantSignerRole<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

    pub admin: Signer<'info>,
}

pub fn handle_grant_signer_role(ctx: Context<GrantSignerRole>, signer: [u8; 64]) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(signer != [0u8; 64], ErrorCode::ZeroSigner);
    // SOL-MED-006: the previous "role separation" check was dead code; the
    // secp256k1 X coordinate cannot be compared to a Solana ed25519
    // address. Operational separation is enforced out-of-band.

    ctx.accounts.config.signer = signer;
    msg!("Signer role granted");
    Ok(())
}
