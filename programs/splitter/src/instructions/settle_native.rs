use anchor_lang::{prelude::*, system_program};

use crate::{
    error::ErrorCode,
    state::{Config, ConsumedNonce, PayerNonce, ProfilesIndex, Quote},
    utils::{emit_payment, split_gross, verify_quote_core},
};

#[derive(Accounts)]
#[instruction(nonce: u64)]
pub struct SettleNative<'info> {
    #[account(mut, seeds = [crate::constants::CONFIG_SEED], bump = config.bump)]
    pub config: Box<Account<'info, Config>>,

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
    // SOL-MED-001: the seed set contains a key-controlled pubkey
    // (payer.key()) so the derived address may not be a true PDA.
    // Anchor's `init_if_needed` derives the address via
    // `create_program_address` and fails if it lands on the curve, which
    // is the correct runtime safety net. No additional check required.
    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: native SOL merchant wallet — validated in handler.
    #[account(mut)]
    pub merchant: UncheckedAccount<'info>,

    /// CHECK: effective treasury wallet (global treasury or route treasury).
    #[account(mut)]
    pub treasury: UncheckedAccount<'info>,

    /// CHECK: IP creator wallet — validated when route has creator fee.
    #[account(mut)]
    pub ip_creator: UncheckedAccount<'info>,

    #[account(mut, seeds = [crate::constants::PROFILES_INDEX_SEED], bump = profiles.bump)]
    pub profiles: Box<Account<'info, ProfilesIndex>>,

    pub system_program: Program<'info, System>,
}

pub fn handle_settle_native(
    ctx: Context<SettleNative>,
    nonce: u64,
    quote: Quote,
    signature: [u8; 65],
) -> Result<()> {
    require!(!ctx.accounts.config.is_paused, ErrorCode::ProtocolPaused);
    require!(
        quote.token.eq(&Pubkey::default()),
        ErrorCode::InvalidTokenForNative
    );
    require!(quote.nonce == nonce, ErrorCode::InvalidNonce);
    require!(
        ctx.accounts.merchant.key().eq(&quote.merchant),
        ErrorCode::MerchantMismatch
    );

    let payer_key = ctx.accounts.payer.key();
    let merchant_key = ctx.accounts.merchant.key();
    let treasury_key = ctx.accounts.treasury.key();
    let ip_creator_key = ctx.accounts.ip_creator.key();
    require!(
        merchant_key != payer_key,
        ErrorCode::DuplicateSettlementAccount
    );
    require!(
        treasury_key != payer_key,
        ErrorCode::DuplicateSettlementAccount
    );
    require!(
        ip_creator_key != payer_key,
        ErrorCode::DuplicateSettlementAccount
    );
    require!(
        merchant_key != treasury_key
            && merchant_key != ip_creator_key
            && treasury_key != ip_creator_key,
        ErrorCode::DuplicateSettlementAccount
    );

    // Reject protocol PDAs as settlement recipients. Sending lamports to a
    // program-owned PDA either locks funds forever (the program has no
    // withdrawal path for config/token_list/profiles) or pollutes event
    // accounting. SOL-HIGH-003.
    let protocol_pdas = [
        ctx.accounts.config.key(),
        ctx.accounts.profiles.key(),
        ctx.accounts.payer_nonce.key(),
        ctx.accounts.consumed_nonce.key(),
    ];
    for pda in &protocol_pdas {
        require!(merchant_key != *pda, ErrorCode::ProtocolAccountMisuse);
        require!(treasury_key != *pda, ErrorCode::ProtocolAccountMisuse);
        require!(ip_creator_key != *pda, ErrorCode::ProtocolAccountMisuse);
    }

    let profile = verify_quote_core(
        &quote,
        &signature,
        &ctx.accounts.payer,
        &mut ctx.accounts.payer_nonce,
        &mut ctx.accounts.consumed_nonce,
        &ctx.accounts.profiles,
        &ctx.accounts.config,
    )?;

    // Payer must have enough lamports for the gross amount.
    require!(
        ctx.accounts.payer.lamports() >= quote.gross_amount,
        ErrorCode::IncorrectNativeValue
    );

    let (merchant_amt, treasury_amt, ip_amt) =
        split_gross(quote.gross_amount, &profile, &quote.ip_creator)?;

    let effective_treasury = if profile.route_treasury.eq(&Pubkey::default()) {
        ctx.accounts.config.treasury
    } else {
        profile.route_treasury
    };
    require!(
        ctx.accounts.treasury.key().eq(&effective_treasury),
        ErrorCode::ZeroTreasury
    );

    // Debit payer and credit recipients atomically through the System
    // program so rent-exemption invariants are preserved. Direct lamport
    // mutation bypasses the System program and can drop accounts below
    // the rent-exempt minimum.
    let system_program_id = ctx.accounts.system_program.key();
    let payer_info = ctx.accounts.payer.to_account_info();
    let merchant_info = ctx.accounts.merchant.to_account_info();

    system_program::transfer(
        CpiContext::new(
            system_program_id,
            system_program::Transfer {
                from: payer_info.clone(),
                to: merchant_info,
            },
        ),
        merchant_amt,
    )
    .map_err(|_| ErrorCode::MerchantTransferFailed)?;

    if treasury_amt > 0 {
        let treasury_info = ctx.accounts.treasury.to_account_info();
        system_program::transfer(
            CpiContext::new(
                system_program_id,
                system_program::Transfer {
                    from: payer_info.clone(),
                    to: treasury_info,
                },
            ),
            treasury_amt,
        )
        .map_err(|_| ErrorCode::TreasuryTransferFailed)?;
    }

    if ip_amt > 0 {
        require!(
            !quote.ip_creator.eq(&Pubkey::default()),
            ErrorCode::MissingIPCreator
        );
        require!(
            ctx.accounts.ip_creator.key().eq(&quote.ip_creator),
            ErrorCode::IPCreatorMismatch
        );
        let ip_info = ctx.accounts.ip_creator.to_account_info();
        system_program::transfer(
            CpiContext::new(
                system_program_id,
                system_program::Transfer {
                    from: payer_info,
                    to: ip_info,
                },
            ),
            ip_amt,
        )
        .map_err(|_| ErrorCode::IPCreatorTransferFailed)?;
    }

    emit_payment(&quote, merchant_amt, treasury_amt, ip_amt)?;
    Ok(())
}
