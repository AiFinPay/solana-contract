use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

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

    pub token_program: Program<'info, Token>,
}

pub fn handle_set_whitelisted_tokens(
    ctx: Context<SetWhitelistedTokens>,
    tokens: Vec<Pubkey>,
    allowed: Vec<bool>,
) -> Result<()> {
    let config = &ctx.accounts.config;
    require!(
        config.admin.eq(&ctx.accounts.admin.key()),
        ErrorCode::Unauthorized
    );
    require!(
        tokens.len() == allowed.len(),
        ErrorCode::ArrayLengthMismatch
    );
    require!(tokens.len() <= MAX_TOKENS, ErrorCode::TokenListFull);

    // SOL-MED-005: each mint referenced in the update must be supplied as
    // a remaining account and validated against the SPL Token program.
    let remaining = ctx.remaining_accounts;
    require!(
        remaining.len() == tokens.len(),
        ErrorCode::ArrayLengthMismatch
    );

    // Validate every supplied mint: owner == token_program, and the key
    // matches the requested mint pubkey.
    for (i, mint_info) in remaining.iter().enumerate() {
        require!(
            *mint_info.owner == ctx.accounts.token_program.key(),
            ErrorCode::InvalidMint
        );
        let mint = Mint::try_deserialize(&mut &mint_info.data.borrow()[..])
            .map_err(|_| ErrorCode::InvalidMint)?;
        require!(mint.is_initialized, ErrorCode::InvalidMint);
        require!(mint_info.key() == tokens[i], ErrorCode::InvalidMint);
    }

    // Reject duplicate mints in a single update to guarantee deterministic add/remove semantics.
    for (i, mint) in tokens.iter().enumerate() {
        require!(!mint.eq(&Pubkey::default()), ErrorCode::ZeroStablecoin);
        require!(
            !tokens[..i].iter().any(|t| t.eq(mint)),
            ErrorCode::DuplicateToken
        );
    }

    let token_list = &mut ctx.accounts.token_list;
    for (i, mint) in tokens.iter().enumerate() {
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
