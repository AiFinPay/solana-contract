use anchor_lang::prelude::*;

use crate::{
    constants::MAX_TOKENS,
    error::ErrorCode,
    state::{Config, TokenList},
};

#[derive(Accounts)]
pub struct SetWhitelistedTokens<'info> {
    #[account(seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,

    #[account(mut, seeds = [crate::constants::TOKEN_LIST_SEED], bump = token_list.bump)]
    pub token_list: Account<'info, TokenList>,

    pub admin: Signer<'info>,
}

pub fn handle_set_whitelisted_tokens(
    ctx: Context<SetWhitelistedTokens>,
    tokens: Vec<Pubkey>,
    allowed: Vec<bool>,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(config.admin.eq(&ctx.accounts.admin.key()), ErrorCode::Unauthorized);
    require!(tokens.len() == allowed.len(), ErrorCode::ArrayLengthMismatch);

    let token_list = &mut ctx.accounts.token_list;
    for (i, mint) in tokens.iter().enumerate() {
        require!(!mint.eq(&Pubkey::default()), ErrorCode::ZeroStablecoin);
        let pos = token_list.tokens.iter().position(|t| t.eq(mint));
        if allowed[i] {
            if pos.is_none() {
                require!(
                    token_list.tokens.len() < MAX_TOKENS,
                    ErrorCode::TokenListFull
                );
                token_list.tokens.push(*mint);
            }
        } else if let Some(idx) = pos {
            token_list.tokens.remove(idx);
        }
    }

    msg!("Token list updated");
    Ok(())
}
