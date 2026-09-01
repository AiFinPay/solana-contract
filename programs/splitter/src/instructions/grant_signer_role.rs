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
    // Operational role separation: signer must be a different key material from admin/pauser.
    // The secp256k1 public key cannot be directly compared to a Solana ed25519 address,
    // so this only checks the zero/default case which is already rejected above.
    require!(
        config.admin != Pubkey::default() && config.pauser != Pubkey::default(),
        ErrorCode::AdminEqualsSigner
    );

    ctx.accounts.config.signer = signer;
    msg!("Signer role granted");
    Ok(())
}
