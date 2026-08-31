use anchor_lang::prelude::*;

use crate::{
    error::ErrorCode,
    state::{Config, ConsumedNonce, PayerNonce, Quote},
    utils::{emit_payment, split_gross, verify_quote_core},
};

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct SettleNative<'info> {
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

    /// CHECK: native SOL merchant wallet — validated against quote.merchant.
    #[account(mut)]
    pub merchant: UncheckedAccount<'info>,

    /// CHECK: hardcoded protocol treasury (PROTOCOL_TREASURY).
    #[account(mut, address = crate::constants::PROTOCOL_TREASURY @ ErrorCode::ZeroMerchant)]
    pub treasury: UncheckedAccount<'info>,

    /// CHECK: IP creator wallet — required when route has ip_creator_bps > 0.
    #[account(mut)]
    pub ip_creator: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handle_settle_native(
    ctx: Context<SettleNative>,
    nonce: u64,
    quote: Quote,
    signature: [u8; 65],
) -> Result<()> {
    require!(
        quote.token.eq(&Pubkey::default()),
        ErrorCode::InvalidTokenForNative
    );
    require!(quote.nonce == nonce, ErrorCode::InvalidNonce);
    require!(
        ctx.accounts.merchant.key().eq(&quote.merchant),
        ErrorCode::InvalidPayer
    );
    require!(ctx.accounts.config.initialized, ErrorCode::InvalidSigner);

    let profile = verify_quote_core(
        &quote,
        &signature,
        &ctx.accounts.payer,
        &mut ctx.accounts.payer_nonce,
        &mut ctx.accounts.consumed_nonce,
        &ctx.accounts.config,
    )?;

    require!(
        ctx.accounts.payer.lamports() >= quote.gross_amount,
        ErrorCode::IncorrectNativeValue
    );

    let (merchant_amt, treasury_amt, ip_amt) = split_gross(quote.gross_amount, &profile)?;

    let payer_info = ctx.accounts.payer.to_account_info();
    **payer_info.try_borrow_mut_lamports()? = payer_info
        .lamports()
        .checked_sub(quote.gross_amount)
        .ok_or(ErrorCode::IncorrectNativeValue)?;

    let merchant_info = ctx.accounts.merchant.to_account_info();
    **merchant_info.try_borrow_mut_lamports()? = merchant_info
        .lamports()
        .checked_add(merchant_amt)
        .ok_or(ErrorCode::MerchantTransferFailed)?;

    if treasury_amt > 0 {
        let treasury_info = ctx.accounts.treasury.to_account_info();
        **treasury_info.try_borrow_mut_lamports()? = treasury_info
            .lamports()
            .checked_add(treasury_amt)
            .ok_or(ErrorCode::TreasuryTransferFailed)?;
    }

    if ip_amt > 0 {
        require!(
            !quote.ip_creator.eq(&Pubkey::default()),
            ErrorCode::MissingIPCreator
        );
        let ip_info = ctx.accounts.ip_creator.to_account_info();
        **ip_info.try_borrow_mut_lamports()? = ip_info
            .lamports()
            .checked_add(ip_amt)
            .ok_or(ErrorCode::IPCreatorTransferFailed)?;
    }

    emit_payment(&quote, merchant_amt, treasury_amt, ip_amt)?;
    Ok(())
}
