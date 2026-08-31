use anchor_lang::prelude::*;

use crate::constants::INITIAL_SIGNER;
use crate::{
    error::ErrorCode,
    state::Config,
    utils::{recover_signer, set_signer_digest},
};

#[derive(Accounts)]
pub struct SetSigner<'info> {
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Config::INIT_SPACE,
        seeds = [crate::constants::CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, Config>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Rotate the trusted secp256k1 signer. The caller must provide a valid
/// secp256k1 signature from the *current* signer over the EIP-712 digest of
/// the new signer pubkey.
///
/// On first ever call (when `config.initialized == false`), the digest is
/// verified against `INITIAL_SIGNER` (the hardcoded bootstrap pubkey)
/// instead of `config.signer`.
pub fn handle_set_signer(
    ctx: Context<SetSigner>,
    new_signer: [u8; 64],
    signature: [u8; 65],
) -> Result<()> {
    require!(new_signer != [0u8; 64], ErrorCode::ZeroSigner);

    let current_signer = if ctx.accounts.config.initialized {
        ctx.accounts.config.signer
    } else {
        INITIAL_SIGNER
    };
    require!(current_signer != [0u8; 64], ErrorCode::InvalidSigner);

    let digest = set_signer_digest(&crate::ID, &current_signer, &new_signer);
    let recovered = recover_signer(&digest, &signature)?;
    require!(recovered == current_signer, ErrorCode::InvalidSigner);

    ctx.accounts.config.signer = new_signer;
    ctx.accounts.config.initialized = true;
    ctx.accounts.config.bump = ctx.bumps.config;

    msg!("Signer rotated");
    Ok(())
}
