use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer as SplTransfer};

use crate::constants::{USDC_MINT, USDT_MINT};
use crate::{
    error::ErrorCode,
    state::{Config, ConsumedNonce, PayerNonce, Quote},
    utils::{emit_payment, split_gross, verify_quote_core},
};

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct SettleStable<'info> {
    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + Config::INIT_SPACE,
        seeds = [crate::constants::CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, Config>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + PayerNonce::INIT_SPACE,
        seeds = [crate::constants::PAYER_NONCE_SEED, payer.key().as_ref()],
        bump
    )]
    pub payer_nonce: Account<'info, PayerNonce>,

    #[account(
        init_if_needed,
        payer = payer,
        space = 8 + ConsumedNonce::INIT_SPACE,
        seeds = [crate::constants::CONSUMED_NONCE_SEED, payer.key().as_ref(), nonce.to_le_bytes().as_ref()],
        bump
    )]
    pub consumed_nonce: Account<'info, ConsumedNonce>,

    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: the mint — must equal USDC_MINT or USDT_MINT (validated in handler).
    pub mint: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_settle_stable<'a>(
    ctx: Context<'a, SettleStable<'a>>,
    nonce: u64,
    quote: Quote,
    signature: [u8; 65],
) -> Result<()> {
    require!(
        !quote.token.eq(&Pubkey::default()),
        ErrorCode::UnsupportedToken
    );
    require!(
        quote.token.eq(&ctx.accounts.mint.key()),
        ErrorCode::UnsupportedToken
    );
    require!(
        ctx.accounts.mint.key() == USDC_MINT || ctx.accounts.mint.key() == USDT_MINT,
        ErrorCode::UnsupportedToken
    );
    require!(quote.nonce == nonce, ErrorCode::InvalidNonce);
    require!(ctx.accounts.config.initialized, ErrorCode::InvalidSigner);

    let remaining = ctx.remaining_accounts;
    require!(remaining.len() >= 3, ErrorCode::UnsupportedToken);

    // Validate that remaining accounts are distinct from each other and from the payer.
    let payer_key = ctx.accounts.payer.key();
    for (i, account_i) in remaining.iter().enumerate() {
        let key_i = account_i.key();
        require!(key_i != payer_key, ErrorCode::DuplicateSettlementAccount);
        for account_j in remaining.iter().skip(i + 1) {
            require!(
                key_i != account_j.key(),
                ErrorCode::DuplicateSettlementAccount
            );
        }
    }

    let mint = ctx.accounts.mint.key();
    let token_program = ctx.accounts.token_program.key();

    let _payer_ata = deserialize_token_account(
        &remaining[0],
        token_program,
        &mint,
        Some(&ctx.accounts.payer.key()),
    )?;
    let _merchant_ata =
        deserialize_token_account(&remaining[1], token_program, &mint, Some(&quote.merchant))?;
    let treasury_ata = deserialize_token_account(&remaining[2], token_program, &mint, None)?;

    let profile = verify_quote_core(
        &quote,
        &signature,
        &ctx.accounts.payer,
        &mut ctx.accounts.payer_nonce,
        &mut ctx.accounts.consumed_nonce,
        &ctx.accounts.config,
    )?;

    require!(
        treasury_ata.owner == crate::constants::PROTOCOL_TREASURY,
        ErrorCode::ZeroMerchant
    );

    let (merchant_amt, treasury_amt, ip_amt) = split_gross(quote.gross_amount, &profile)?;

    let token_program_id = ctx.accounts.token_program.key();
    let payer_account = ctx.accounts.payer.to_account_info();

    token::transfer(
        CpiContext::new(
            token_program_id,
            SplTransfer {
                from: remaining[0].clone(),
                to: remaining[1].clone(),
                authority: payer_account.clone(),
            },
        ),
        merchant_amt,
    )?;

    if treasury_amt > 0 {
        token::transfer(
            CpiContext::new(
                token_program_id,
                SplTransfer {
                    from: remaining[0].clone(),
                    to: remaining[2].clone(),
                    authority: payer_account.clone(),
                },
            ),
            treasury_amt,
        )?;
    }

    if ip_amt > 0 {
        require!(
            !quote.ip_creator.eq(&Pubkey::default()),
            ErrorCode::MissingIPCreator
        );
        require!(remaining.len() >= 4, ErrorCode::MissingIPCreator);
        let _ip_ata = deserialize_token_account(
            &remaining[3],
            token_program,
            &mint,
            Some(&quote.ip_creator),
        )?;
        token::transfer(
            CpiContext::new(
                token_program_id,
                SplTransfer {
                    from: remaining[0].clone(),
                    to: remaining[3].clone(),
                    authority: payer_account.clone(),
                },
            ),
            ip_amt,
        )?;
    }

    emit_payment(&quote, merchant_amt, treasury_amt, ip_amt)?;
    Ok(())
}

fn deserialize_token_account(
    info: &AccountInfo,
    token_program: Pubkey,
    expected_mint: &Pubkey,
    expected_owner: Option<&Pubkey>,
) -> Result<TokenAccount> {
    require!(*info.owner == token_program, ErrorCode::UnsupportedToken);
    let data = info.try_borrow_data()?;
    let account =
        TokenAccount::try_deserialize(&mut &data[..]).map_err(|_| ErrorCode::UnsupportedToken)?;
    require!(account.mint == *expected_mint, ErrorCode::UnsupportedToken);
    if let Some(owner) = expected_owner {
        require!(account.owner == *owner, ErrorCode::InvalidPayer);
    }
    Ok(account)
}
