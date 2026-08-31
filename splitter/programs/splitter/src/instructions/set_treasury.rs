use anchor_lang::prelude::*;

use crate::{error::ErrorCode, state::Config, utils::emit_treasury_updated};

#[derive(Accounts)]
pub struct SetTreasury<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    pub admin: Signer<'info>,
}

pub fn handle_set_treasury(ctx: Context<SetTreasury>, new_treasury: Pubkey) -> Result<()> {
    let config = &mut ctx.accounts.config;
    require!(config.admin.eq(&ctx.accounts.admin.key()), ErrorCode::Unauthorized);
    require!(!new_treasury.eq(&Pubkey::default()), ErrorCode::ZeroTreasury);
    config.treasury = new_treasury;
    emit_treasury_updated(new_treasury);
    Ok(())
}
