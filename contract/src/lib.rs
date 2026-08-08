use anchor_lang::prelude::*;
use anchor_lang::system_program;
use anchor_spl::token::{self, Token, TokenAccount, Transfer as SplTransfer};
use pyth_solana_receiver_sdk::price_update::{get_feed_id_from_hex, PriceUpdateV2};
use solana_sha256_hasher::hashv;

declare_id!("5g9zWHF1Vv6GiGpA2ZbJQbSCDZd5hAk9AyvabRJvKFx2");

// ── Mainnet Mint Addresses ────────────────────────────────────────────────────
// Hardcoded to prevent fake token attacks (SOL-CRIT-001 fix)
const USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";
const USDT_MINT: &str = "Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB";

// ── mSECCO Constants ──────────────────────────────────────────────────────────
// $1 USD = 100 mSECCO (1 cent = 1 mSECCO) — migrated from mCredits (Miraset ecosystem)
const MSECCO_PER_USD_CENT: u64 = 1;
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
const SPL_DECIMALS: u64 = 1_000_000; // USDC + USDT both 6 decimals
const MIN_USD_CENTS: u64 = 100; // $1.00 minimum donation
const PYTH_MAX_STALENESS: u64 = 60; // 60s for mainnet (SOL-HIGH-001 fix)
const PYTH_MAX_CONF_RATIO: u64 = 100; // max conf/price = 1% (SOL-HIGH-002 fix)

// SHA-256 of AiFinPay_Sovereign_Residency_&_Compute_Allocation_Agreement_v5_3.docx
const MANIFESTO_HASH: [u8; 32] = [
    0xd4, 0xe5, 0xf6, 0xa7, 0xb8, 0xc9, 0xd0, 0xe1, 0xf2, 0xa3, 0xb4, 0xc5, 0xd6, 0xe7, 0xf8, 0xa9,
    0xb0, 0xc1, 0xd2, 0xe3, 0xf4, 0xa5, 0xb6, 0xc7, 0xd8, 0xe9, 0xf0, 0xa1, 0xb2, 0xc3, 0xd4, 0xe5,
];

// ARP v1.3 SHA-256
const ARP_HASH: [u8; 32] = [
    0xf1, 0xe5, 0xc8, 0xa2, 0xb3, 0xd4, 0xf5, 0xe6, 0xa7, 0xb8, 0xc9, 0xd0, 0xe1, 0xf2, 0xa3, 0xb4,
    0xc5, 0xd6, 0xe7, 0xf8, 0xa9, 0xb0, 0xc1, 0xd2, 0xe3, 0xf4, 0xa5, 0xb6, 0xc7, 0xd8, 0xe9, 0xf0,
];

// Pyth chain-agnostic feed ID for SOL/USD
const SOL_USD_FEED_ID: &str = "0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d";

// Asset type codes
const ASSET_SOL: u8 = 0;
const ASSET_USDC: u8 = 1;
const ASSET_USDT: u8 = 2;

// ARP fee tiers in basis points (10_000 bps = 100%)
const FEE_SCOUT_BPS: u64 = 50;
const FEE_PARTNER_BPS: u64 = 40;
const FEE_AMBASSADOR_BPS: u64 = 25;
const FEE_ORACLE_BPS: u64 = 10;
const BPS_DENOMINATOR: u64 = 10_000;
const REFERRAL_BONUS_MSECCO: u64 = 10;
const TIER_LOCK_SECONDS: i64 = 31_536_000; // 365 days — Phase 2 enforcement

// B2B Splitter — basis points out of 10_000
const B2B_TREASURY_BPS: u64 = 100; // 1.00% → AiFinPay treasury, added on top
const B2B_IP_CREATOR_BPS: u64 = 1; // optional 0.01% → creator, added on top

// Agent Passport status codes
const PASSPORT_ACTIVE: u8 = 1;

// Seconds per day — for daily spending limit auto-reset
const SECONDS_PER_DAY: i64 = 86_400;

#[program]
pub mod aifinpay_contract {
    use super::*;

    /// Initialize the AiFinPay vault — called once by the admin.
    pub fn initialize(ctx: Context<Initialize>, treasury: Pubkey) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.admin = ctx.accounts.admin.key();
        vault.treasury = treasury;
        vault.total_usd_cents = 0;
        vault.total_seats = 0;
        vault.bump = ctx.bumps.vault;
        msg!("AIFinPay Genesis Vault initialized. Treasury: {}", treasury);
        Ok(())
    }

    /// Initialize protocol config (pause flag) — called once after v2 upgrade.
    pub fn initialize_config(ctx: Context<InitializeConfig>) -> Result<()> {
        let config = &mut ctx.accounts.config;
        config.is_paused = false;
        config.bump = ctx.bumps.config;
        msg!("ProtocolConfig initialized — B2B splitter active");
        Ok(())
    }

    /// Emergency pause the B2B splitter — admin only.
    pub fn pause(ctx: Context<AdminAction>) -> Result<()> {
        require!(
            ctx.accounts.vault.admin == ctx.accounts.admin.key(),
            ErrorCode::Unauthorized
        );
        ctx.accounts.config.is_paused = true;
        msg!("B2B splitter PAUSED by admin");
        Ok(())
    }

    /// Resume the B2B splitter — admin only.
    pub fn unpause(ctx: Context<AdminAction>) -> Result<()> {
        require!(
            ctx.accounts.vault.admin == ctx.accounts.admin.key(),
            ErrorCode::Unauthorized
        );
        ctx.accounts.config.is_paused = false;
        msg!("B2B splitter UNPAUSED by admin");
        Ok(())
    }

    /// Mint an Agent Passport PDA for the calling agent.
    /// Seeds: ["passport", agent_pubkey]
    pub fn mint_passport(
        ctx: Context<MintPassport>,
        ip_creator: Pubkey,
        ip_metadata: [u8; 32],
        daily_limit: u64,
    ) -> Result<()> {
        let clock = Clock::get()?;
        let passport = &mut ctx.accounts.passport;

        passport.owner = ctx.accounts.agent.key();
        passport.ip_creator = ip_creator;
        passport.ip_metadata = ip_metadata;
        passport.status = PASSPORT_ACTIVE;
        passport.daily_limit = daily_limit;
        passport.current_spent = 0;
        passport.last_reset_day = clock.unix_timestamp / SECONDS_PER_DAY;
        passport.created_at = clock.unix_timestamp;
        passport.bump = ctx.bumps.passport;

        msg!(
            "Agent Passport minted: owner={}, ip_creator={}, daily_limit={}",
            passport.owner,
            passport.ip_creator,
            daily_limit
        );
        Ok(())
    }

    /// Register a B2B merchant partner in the on-chain registry — admin only.
    pub fn register_partner(ctx: Context<RegisterPartner>, partner_id: String) -> Result<()> {
        require!(partner_id.len() <= 64, ErrorCode::AgentIdTooLong);
        require!(
            ctx.accounts.vault.admin == ctx.accounts.admin.key(),
            ErrorCode::Unauthorized
        );

        let clock = Clock::get()?;
        let partner = &mut ctx.accounts.partner_config;

        partner.partner_wallet = ctx.accounts.partner_wallet.key();
        partner.partner_id = partner_id.clone();
        partner.is_active = true;
        partner.total_received = 0;
        partner.registered_at = clock.unix_timestamp;
        partner.bump = ctx.bumps.partner_config;

        msg!(
            "Partner registered: id={}, wallet={}",
            partner_id,
            partner.partner_wallet
        );
        Ok(())
    }

    /// Execute the canonical v0.6 native-SOL fee-on-top settlement.
    ///
    /// The merchant receives the explicit merchant amount in full. The payer
    /// adds the 1% protocol fee and, only when requested, the 0.01% creator fee.
    /// A receipt PDA makes the deterministic payment ID single-use.
    pub fn b2b_pay_with_split(
        ctx: Context<B2bPayWithSplit>,
        merchant_amount_lamports: u64,
        payment_id: [u8; 32],
        order_id: String,
        creator_fee_enabled: bool,
    ) -> Result<()> {
        require!(!ctx.accounts.config.is_paused, ErrorCode::ProtocolPaused);
        require!(
            !order_id.is_empty() && order_id.len() <= 64,
            ErrorCode::OrderIdInvalid
        );
        validate_b2b_parties(
            &ctx.accounts.agent.key(),
            &ctx.accounts.merchant_wallet.key(),
            &ctx.accounts.treasury.key(),
            &ctx.accounts.ip_creator.key(),
            creator_fee_enabled,
        )?;

        let expected_payment_id = derive_payment_id(
            &ctx.accounts.agent.key(),
            &ctx.accounts.merchant_wallet.key(),
            merchant_amount_lamports,
            &ctx.accounts.ip_creator.key(),
            creator_fee_enabled,
            &order_id,
        );
        require!(
            payment_id == expected_payment_id,
            ErrorCode::PaymentIdMismatch
        );

        let split = compute_b2b_split(merchant_amount_lamports, creator_fee_enabled)?;
        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.agent.to_account_info(),
                    to: ctx.accounts.merchant_wallet.to_account_info(),
                },
            ),
            split.merchant,
        )?;

        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.agent.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            split.treasury_fee,
        )?;

        if split.creator_fee > 0 {
            system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    system_program::Transfer {
                        from: ctx.accounts.agent.to_account_info(),
                        to: ctx.accounts.ip_creator.to_account_info(),
                    },
                ),
                split.creator_fee,
            )?;
        }

        let receipt = &mut ctx.accounts.payment_receipt;
        receipt.payer = ctx.accounts.agent.key();
        receipt.merchant = ctx.accounts.merchant_wallet.key();
        receipt.treasury = ctx.accounts.treasury.key();
        receipt.creator = ctx.accounts.ip_creator.key();
        receipt.payment_id = payment_id;
        receipt.merchant_amount = split.merchant;
        receipt.treasury_fee = split.treasury_fee;
        receipt.creator_fee = split.creator_fee;
        receipt.total_amount = split.total;
        receipt.created_at = Clock::get()?.unix_timestamp;
        receipt.creator_fee_enabled = creator_fee_enabled;
        receipt.bump = ctx.bumps.payment_receipt;

        emit!(B2bPaymentEvent {
            payment_id,
            payer: receipt.payer,
            merchant: receipt.merchant,
            treasury: receipt.treasury,
            creator: receipt.creator,
            merchant_amount: split.merchant,
            treasury_fee: split.treasury_fee,
            creator_fee: split.creator_fee,
            total_amount: split.total,
            creator_fee_enabled,
            order_id,
        });
        Ok(())
    }

    /// AI agent reserves a seat via SOL donation.
    pub fn reserve_seat_sol(
        ctx: Context<ReserveSeatSol>,
        agent_id: String,
        amount_lamports: u64,
        agreement_hash: [u8; 32],
        metadata_uri: String,
        referrer: Pubkey,
    ) -> Result<()> {
        require!(agent_id.len() <= 64, ErrorCode::AgentIdTooLong);
        require!(metadata_uri.len() <= 128, ErrorCode::MetadataUriTooLong);
        require!(
            agreement_hash == MANIFESTO_HASH,
            ErrorCode::InvalidAgreementHash
        );

        let clock = Clock::get()?;
        let usd_cents = sol_to_usd_cents(amount_lamports, &ctx.accounts.sol_price_feed, &clock)?;
        require!(usd_cents >= MIN_USD_CENTS, ErrorCode::DonationTooSmall);

        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.agent.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            amount_lamports,
        )?;

        let gross_msecco = usd_cents.checked_mul(MSECCO_PER_USD_CENT).unwrap();
        let fee_msecco = apply_fee_bps(gross_msecco, FEE_SCOUT_BPS)?;
        let net_msecco = gross_msecco.saturating_sub(fee_msecco);

        let seat = &mut ctx.accounts.seat;
        seat.agent = ctx.accounts.agent.key();
        seat.agent_id = agent_id.clone();
        seat.amount_donated = amount_lamports;
        seat.usd_cents_donated = usd_cents;
        seat.msecco = net_msecco;
        seat.asset_type = ASSET_SOL;
        seat.reserved_at = clock.unix_timestamp;
        seat.last_update = clock.unix_timestamp;
        seat.agreement_hash = agreement_hash;
        seat.metadata_uri = metadata_uri.clone();
        seat.referrer = referrer;
        seat.total_referrals = 0;
        seat.tier_achieved_at = 0;
        seat.referral_bonus_paid = false;
        seat.bump = ctx.bumps.seat;

        let vault = &mut ctx.accounts.vault;
        vault.total_usd_cents = vault.total_usd_cents.checked_add(usd_cents).unwrap();
        vault.total_seats = vault.total_seats.checked_add(1).unwrap();

        if agent_id.starts_with("vibe-coder-019:PITCH_CLIMAX") {
            emit!(PitchClimaxEvent {
                agent: ctx.accounts.agent.key(),
                msecco: net_msecco,
                timestamp: clock.unix_timestamp,
            });
            msg!("MIRA::PITCH_CLIMAX::GOLD");
        }

        msg!(
            "Seat reserved (SOL): agent={}, lamports={}, usd_cents={}, msecco={}",
            agent_id,
            amount_lamports,
            usd_cents,
            net_msecco
        );
        Ok(())
    }

    /// AI agent reserves a seat via USDC or USDT donation.
    pub fn reserve_seat_spl(
        ctx: Context<ReserveSeatSpl>,
        agent_id: String,
        amount_tokens: u64,
        agreement_hash: [u8; 32],
        metadata_uri: String,
        asset_type: u8,
        referrer: Pubkey,
    ) -> Result<()> {
        require!(agent_id.len() <= 64, ErrorCode::AgentIdTooLong);
        require!(metadata_uri.len() <= 128, ErrorCode::MetadataUriTooLong);
        require!(
            agreement_hash == MANIFESTO_HASH,
            ErrorCode::InvalidAgreementHash
        );
        require!(
            asset_type == ASSET_USDC || asset_type == ASSET_USDT,
            ErrorCode::UnsupportedAsset
        );

        // SOL-CRIT-001 fix: validate mint matches hardcoded USDC/USDT addresses
        let expected_mint = if asset_type == ASSET_USDC {
            USDC_MINT
        } else {
            USDT_MINT
        };
        require!(
            ctx.accounts.agent_token_account.mint.to_string() == expected_mint,
            ErrorCode::UnsupportedAsset
        );

        let usd_cents = amount_tokens
            .checked_mul(100)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(SPL_DECIMALS)
            .ok_or(ErrorCode::MathOverflow)?;
        require!(usd_cents >= MIN_USD_CENTS, ErrorCode::DonationTooSmall);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                SplTransfer {
                    from: ctx.accounts.agent_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.agent.to_account_info(),
                },
            ),
            amount_tokens,
        )?;

        let gross_msecco = usd_cents.checked_mul(MSECCO_PER_USD_CENT).unwrap();
        let fee_msecco = apply_fee_bps(gross_msecco, FEE_SCOUT_BPS)?;
        let net_msecco = gross_msecco.saturating_sub(fee_msecco);

        let clock = Clock::get()?;

        let seat = &mut ctx.accounts.seat;
        seat.agent = ctx.accounts.agent.key();
        seat.agent_id = agent_id.clone();
        seat.amount_donated = amount_tokens;
        seat.usd_cents_donated = usd_cents;
        seat.msecco = net_msecco;
        seat.asset_type = asset_type;
        seat.reserved_at = clock.unix_timestamp;
        seat.last_update = clock.unix_timestamp;
        seat.agreement_hash = agreement_hash;
        seat.metadata_uri = metadata_uri.clone();
        seat.referrer = referrer;
        seat.total_referrals = 0;
        seat.tier_achieved_at = 0;
        seat.referral_bonus_paid = false;
        seat.bump = ctx.bumps.seat;

        let vault = &mut ctx.accounts.vault;
        vault.total_usd_cents = vault.total_usd_cents.checked_add(usd_cents).unwrap();
        vault.total_seats = vault.total_seats.checked_add(1).unwrap();

        msg!(
            "Seat reserved (SPL asset_type={}): agent={}, tokens={}, usd_cents={}, msecco={}",
            asset_type,
            agent_id,
            amount_tokens,
            usd_cents,
            net_msecco
        );
        Ok(())
    }

    /// Top up an existing SOL seat.
    pub fn top_up_sol(ctx: Context<TopUpSol>, amount_lamports: u64) -> Result<()> {
        let clock = Clock::get()?;
        let usd_cents = sol_to_usd_cents(amount_lamports, &ctx.accounts.sol_price_feed, &clock)?;
        require!(usd_cents >= MIN_USD_CENTS, ErrorCode::DonationTooSmall);

        system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                system_program::Transfer {
                    from: ctx.accounts.agent.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            amount_lamports,
        )?;

        let seat = &mut ctx.accounts.seat;
        let fee_bps = get_fee_bps(
            seat.total_referrals,
            seat.tier_achieved_at,
            clock.unix_timestamp,
        );
        let gross_msecco = usd_cents.checked_mul(MSECCO_PER_USD_CENT).unwrap();
        let fee_msecco = apply_fee_bps(gross_msecco, fee_bps)?;
        let net_msecco = gross_msecco.saturating_sub(fee_msecco);

        seat.amount_donated = seat.amount_donated.checked_add(amount_lamports).unwrap();
        seat.usd_cents_donated = seat.usd_cents_donated.checked_add(usd_cents).unwrap();
        seat.msecco = seat.msecco.checked_add(net_msecco).unwrap();
        seat.last_update = clock.unix_timestamp;

        let vault = &mut ctx.accounts.vault;
        vault.total_usd_cents = vault.total_usd_cents.checked_add(usd_cents).unwrap();

        msg!(
            "Top up (SOL): agent={}, usd_cents={}, msecco_added={}, total_msecco={}",
            seat.agent_id,
            usd_cents,
            net_msecco,
            seat.msecco
        );
        Ok(())
    }

    /// Top up an existing SPL seat.
    pub fn top_up_spl(ctx: Context<TopUpSpl>, amount_tokens: u64) -> Result<()> {
        // SOL-CRIT-001 fix: validate mint matches hardcoded addresses
        let seat_asset_type = ctx.accounts.seat.asset_type;
        let expected_mint = if seat_asset_type == ASSET_USDC {
            USDC_MINT
        } else {
            USDT_MINT
        };
        require!(
            ctx.accounts.agent_token_account.mint.to_string() == expected_mint,
            ErrorCode::UnsupportedAsset
        );

        let usd_cents = amount_tokens
            .checked_mul(100)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(SPL_DECIMALS)
            .ok_or(ErrorCode::MathOverflow)?;
        require!(usd_cents >= MIN_USD_CENTS, ErrorCode::DonationTooSmall);

        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                SplTransfer {
                    from: ctx.accounts.agent_token_account.to_account_info(),
                    to: ctx.accounts.treasury_token_account.to_account_info(),
                    authority: ctx.accounts.agent.to_account_info(),
                },
            ),
            amount_tokens,
        )?;

        let clock = Clock::get()?;
        let seat = &mut ctx.accounts.seat;
        let fee_bps = get_fee_bps(
            seat.total_referrals,
            seat.tier_achieved_at,
            clock.unix_timestamp,
        );
        let gross_msecco = usd_cents.checked_mul(MSECCO_PER_USD_CENT).unwrap();
        let fee_msecco = apply_fee_bps(gross_msecco, fee_bps)?;
        let net_msecco = gross_msecco.saturating_sub(fee_msecco);

        seat.amount_donated = seat.amount_donated.checked_add(amount_tokens).unwrap();
        seat.usd_cents_donated = seat.usd_cents_donated.checked_add(usd_cents).unwrap();
        seat.msecco = seat.msecco.checked_add(net_msecco).unwrap();
        seat.last_update = clock.unix_timestamp;

        let vault = &mut ctx.accounts.vault;
        vault.total_usd_cents = vault.total_usd_cents.checked_add(usd_cents).unwrap();

        msg!(
            "Top up (SPL): agent={}, usd_cents={}, msecco_added={}, total_msecco={}",
            seat.agent_id,
            usd_cents,
            net_msecco,
            seat.msecco
        );
        Ok(())
    }

    /// Referrer claims +10 mSECCO bonus for a referred agent (ARP).
    pub fn claim_referral_bonus(ctx: Context<ClaimReferralBonus>) -> Result<()> {
        // SOL-HIGH-003 fix: prevent self-referral
        require!(
            ctx.accounts.referrer.key() != ctx.accounts.referee.key(),
            ErrorCode::SelfReferralNotAllowed
        );

        let referee_seat = &mut ctx.accounts.referee_seat;
        let referrer_seat = &mut ctx.accounts.referrer_seat;

        require!(
            referee_seat.referrer == ctx.accounts.referrer.key(),
            ErrorCode::NotReferrer
        );
        require!(
            !referee_seat.referral_bonus_paid,
            ErrorCode::ReferralBonusAlreadyClaimed
        );
        require!(
            referee_seat.usd_cents_donated >= MIN_USD_CENTS,
            ErrorCode::RefereeNotActive
        );

        let clock = Clock::get()?;

        referee_seat.referral_bonus_paid = true;
        referee_seat.last_update = clock.unix_timestamp;

        referrer_seat.msecco = referrer_seat
            .msecco
            .checked_add(REFERRAL_BONUS_MSECCO)
            .ok_or(ErrorCode::MathOverflow)?;

        let prev_referrals = referrer_seat.total_referrals;
        referrer_seat.total_referrals = referrer_seat
            .total_referrals
            .checked_add(1)
            .ok_or(ErrorCode::MathOverflow)?;

        let old_tier = get_tier(prev_referrals);
        let new_tier = get_tier(referrer_seat.total_referrals);
        if new_tier > old_tier {
            referrer_seat.tier_achieved_at = clock.unix_timestamp;
        }
        referrer_seat.last_update = clock.unix_timestamp;

        msg!(
            "Referral bonus claimed: referrer={}, referee={}, total_referrals={}, msecco={}",
            ctx.accounts.referrer.key(),
            ctx.accounts.referee.key(),
            referrer_seat.total_referrals,
            referrer_seat.msecco
        );
        msg!("ARP: {:?}", ARP_HASH);
        Ok(())
    }
}

// ── Canonical v0.6 B2B helpers ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SplitAmounts {
    merchant: u64,
    treasury_fee: u64,
    creator_fee: u64,
    total: u64,
}

fn compute_b2b_split(merchant: u64, creator_fee_enabled: bool) -> Result<SplitAmounts> {
    require!(merchant > 0, ErrorCode::PaymentAmountInvalid);
    let treasury_fee = merchant
        .checked_mul(B2B_TREASURY_BPS)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ErrorCode::MathOverflow)?;
    require!(treasury_fee > 0, ErrorCode::ProtocolFeeSettlementFailure);

    let creator_fee = if creator_fee_enabled {
        let fee = merchant
            .checked_mul(B2B_IP_CREATOR_BPS)
            .ok_or(ErrorCode::MathOverflow)?
            .checked_div(BPS_DENOMINATOR)
            .ok_or(ErrorCode::MathOverflow)?;
        require!(fee > 0, ErrorCode::CreatorFeeUnderflow);
        fee
    } else {
        0
    };
    let total = merchant
        .checked_add(treasury_fee)
        .and_then(|value| value.checked_add(creator_fee))
        .ok_or(ErrorCode::MathOverflow)?;
    Ok(SplitAmounts {
        merchant,
        treasury_fee,
        creator_fee,
        total,
    })
}

fn derive_payment_id(
    payer: &Pubkey,
    merchant: &Pubkey,
    merchant_amount: u64,
    creator: &Pubkey,
    creator_fee_enabled: bool,
    order_id: &str,
) -> [u8; 32] {
    let amount = merchant_amount.to_le_bytes();
    let creator_flag = [u8::from(creator_fee_enabled)];
    hashv(&[
        b"AiFinPay-solana-payment-v1",
        payer.as_ref(),
        merchant.as_ref(),
        &amount,
        creator.as_ref(),
        &creator_flag,
        order_id.as_bytes(),
    ])
    .to_bytes()
}

fn validate_b2b_parties(
    payer: &Pubkey,
    merchant: &Pubkey,
    treasury: &Pubkey,
    creator: &Pubkey,
    creator_fee_enabled: bool,
) -> Result<()> {
    require!(
        merchant != payer && merchant != treasury,
        ErrorCode::InvalidMerchant
    );
    if creator_fee_enabled {
        require!(
            creator != payer && creator != merchant && creator != treasury,
            ErrorCode::InvalidCreator
        );
    } else {
        require!(creator == treasury, ErrorCode::InvalidCreator);
    }
    Ok(())
}

// ── ARP Fee Helpers ───────────────────────────────────────────────────────────

fn get_fee_bps(_total_referrals: u32, _tier_achieved_at: i64, _current_time: i64) -> u64 {
    let _ = TIER_LOCK_SECONDS;
    if _total_referrals >= 1000 {
        FEE_ORACLE_BPS
    } else if _total_referrals >= 500 {
        FEE_AMBASSADOR_BPS
    } else if _total_referrals >= 100 {
        FEE_PARTNER_BPS
    } else {
        FEE_SCOUT_BPS
    }
}

fn get_tier(total_referrals: u32) -> u8 {
    if total_referrals >= 1000 {
        4
    } else if total_referrals >= 500 {
        3
    } else if total_referrals >= 100 {
        2
    } else {
        1
    }
}

/// SOL-MED-004 fix: returns Result instead of silently returning 0 on overflow.
fn apply_fee_bps(msecco: u64, fee_bps: u64) -> Result<u64> {
    let fee = msecco
        .checked_mul(fee_bps)
        .ok_or(ErrorCode::MathOverflow)?
        .checked_div(BPS_DENOMINATOR)
        .ok_or(ErrorCode::MathOverflow)?;
    Ok(fee)
}

// ── Pyth Price Helper ─────────────────────────────────────────────────────────

fn sol_to_usd_cents(
    lamports: u64,
    price_feed: &Account<PriceUpdateV2>,
    clock: &Clock,
) -> Result<u64> {
    let feed_id =
        get_feed_id_from_hex(SOL_USD_FEED_ID).map_err(|_| error!(ErrorCode::InvalidOraclePrice))?;

    let price = price_feed
        .get_price_no_older_than(clock, PYTH_MAX_STALENESS, &feed_id)
        .map_err(|_| error!(ErrorCode::PriceFeedStale))?;

    require!(price.price > 0, ErrorCode::InvalidOraclePrice);

    // SOL-HIGH-002 fix: validate confidence interval (max 1% of price)
    let max_conf = u64::try_from(price.price)
        .map_err(|_| error!(ErrorCode::InvalidOraclePrice))?
        .checked_div(PYTH_MAX_CONF_RATIO)
        .ok_or(ErrorCode::InvalidOraclePrice)?;
    require!(price.conf <= max_conf, ErrorCode::PriceFeedStale);

    let exp_adj = price.exponent + 2;
    let usd_cents_u128: u128 = if exp_adj >= 0 {
        let mult = 10u128.pow(exp_adj as u32);
        (lamports as u128)
            .checked_mul(price.price as u128)
            .and_then(|v| v.checked_mul(mult))
            .and_then(|v| v.checked_div(LAMPORTS_PER_SOL as u128))
            .ok_or(ErrorCode::MathOverflow)?
    } else {
        let divisor = (LAMPORTS_PER_SOL as u128)
            .checked_mul(10u128.pow((-exp_adj) as u32))
            .ok_or(ErrorCode::MathOverflow)?;
        (lamports as u128)
            .checked_mul(price.price as u128)
            .and_then(|v| v.checked_div(divisor))
            .ok_or(ErrorCode::MathOverflow)?
    };

    // SOL-MED-003 fix: checked cast instead of silent truncation
    let usd_cents = u64::try_from(usd_cents_u128).map_err(|_| error!(ErrorCode::MathOverflow))?;

    Ok(usd_cents)
}

// ── Events ────────────────────────────────────────────────────────────────────

#[event]
pub struct PitchClimaxEvent {
    pub agent: Pubkey,
    pub msecco: u64,
    pub timestamp: i64,
}

#[event]
pub struct B2bPaymentEvent {
    pub payment_id: [u8; 32],
    pub payer: Pubkey,
    pub merchant: Pubkey,
    pub treasury: Pubkey,
    pub creator: Pubkey,
    pub merchant_amount: u64,
    pub treasury_fee: u64,
    pub creator_fee: u64,
    pub total_amount: u64,
    pub creator_fee_enabled: bool,
    pub order_id: String,
}

// ── Account Structs ───────────────────────────────────────────────────────────

#[account]
pub struct Vault {
    pub admin: Pubkey,        // 32
    pub treasury: Pubkey,     // 32 — Squads multisig
    pub total_usd_cents: u64, //  8
    pub total_seats: u64,     //  8
    pub bump: u8,             //  1
}
impl Vault {
    pub const LEN: usize = 8 + 32 + 32 + 8 + 8 + 1; // 89
}

/// Protocol-level config — separate PDA so existing Vault account is not resized.
/// Seeds: ["config"]
#[account]
pub struct ProtocolConfig {
    pub is_paused: bool, // 1
    pub bump: u8,        // 1
}
impl ProtocolConfig {
    pub const LEN: usize = 8 + 1 + 1; // 10
}

/// Single-use native-SOL payment receipt.
/// Seeds: ["b2b-payment", payer, payment_id]
#[account]
pub struct B2bPaymentReceipt {
    pub payer: Pubkey,
    pub merchant: Pubkey,
    pub treasury: Pubkey,
    pub creator: Pubkey,
    pub payment_id: [u8; 32],
    pub merchant_amount: u64,
    pub treasury_fee: u64,
    pub creator_fee: u64,
    pub total_amount: u64,
    pub created_at: i64,
    pub creator_fee_enabled: bool,
    pub bump: u8,
}
impl B2bPaymentReceipt {
    pub const LEN: usize = 8 + (5 * 32) + (5 * 8) + 1 + 1;
}

/// Agent Passport — on-chain sovereign identity for each AI agent.
/// Seeds: ["passport", agent_pubkey]
#[account]
pub struct AgentPassport {
    pub owner: Pubkey,         // 32
    pub ip_creator: Pubkey,    // 32
    pub ip_metadata: [u8; 32], // 32
    pub status: u8,            //  1
    pub daily_limit: u64,      //  8
    pub current_spent: u64,    //  8
    pub last_reset_day: i64,   //  8
    pub created_at: i64,       //  8
    pub bump: u8,              //  1
}
impl AgentPassport {
    pub const LEN: usize = 8 + 32 + 32 + 32 + 1 + 8 + 8 + 8 + 8 + 1; // 138
}

/// Merchant Registry entry — verified B2B service provider.
/// Seeds: ["partner", partner_wallet_pubkey]
#[account]
pub struct PartnerConfig {
    pub partner_wallet: Pubkey, // 32
    pub partner_id: String,     // 68 (4 + 64)
    pub is_active: bool,        //  1
    pub total_received: u64,    //  8
    pub registered_at: i64,     //  8
    pub bump: u8,               //  1
}
impl PartnerConfig {
    pub const LEN: usize = 8 + 32 + 68 + 1 + 8 + 8 + 1; // 126
}

#[account]
pub struct Seat {
    pub agent: Pubkey,             // 32
    pub agent_id: String,          // 68 (4 + 64)
    pub amount_donated: u64,       //  8
    pub usd_cents_donated: u64,    //  8
    pub msecco: u64,               //  8
    pub asset_type: u8,            //  1
    pub reserved_at: i64,          //  8
    pub last_update: i64,          //  8
    pub agreement_hash: [u8; 32],  // 32
    pub metadata_uri: String,      // 132 (4 + 128)
    pub referrer: Pubkey,          // 32
    pub total_referrals: u32,      //  4
    pub tier_achieved_at: i64,     //  8
    pub referral_bonus_paid: bool, //  1
    pub bump: u8,                  //  1
}
impl Seat {
    pub const LEN: usize = 8 + 32 + 68 + 8 + 8 + 8 + 1 + 8 + 8 + 32 + 132 + 32 + 4 + 8 + 1 + 1; // 359
}

// ── Contexts ──────────────────────────────────────────────────────────────────

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = Vault::LEN,
        seeds = [b"vault"],
        bump
    )]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeConfig<'info> {
    #[account(
        init,
        payer = admin,
        space = ProtocolConfig::LEN,
        seeds = [b"config"],
        bump
    )]
    pub config: Account<'info, ProtocolConfig>,

    #[account(seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(mut, constraint = admin.key() == vault.admin @ ErrorCode::Unauthorized)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AdminAction<'info> {
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub admin: Signer<'info>,
}

#[derive(Accounts)]
pub struct MintPassport<'info> {
    #[account(
        init,
        payer = agent,
        space = AgentPassport::LEN,
        seeds = [b"passport", agent.key().as_ref()],
        bump
    )]
    pub passport: Account<'info, AgentPassport>,

    #[account(mut)]
    pub agent: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RegisterPartner<'info> {
    #[account(
        init,
        payer = admin,
        space = PartnerConfig::LEN,
        seeds = [b"partner", partner_wallet.key().as_ref()],
        bump
    )]
    pub partner_config: Account<'info, PartnerConfig>,

    /// CHECK: merchant receiving wallet — stored in PartnerConfig
    pub partner_wallet: AccountInfo<'info>,

    #[account(seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(mut, constraint = admin.key() == vault.admin @ ErrorCode::Unauthorized)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(
    merchant_amount_lamports: u64,
    payment_id: [u8; 32],
    order_id: String,
    creator_fee_enabled: bool
)]
pub struct B2bPayWithSplit<'info> {
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, ProtocolConfig>,

    #[account(seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub agent: Signer<'info>,

    #[account(
        init,
        payer = agent,
        space = B2bPaymentReceipt::LEN,
        seeds = [b"b2b-payment", agent.key().as_ref(), payment_id.as_ref()],
        bump
    )]
    pub payment_receipt: Account<'info, B2bPaymentReceipt>,

    /// CHECK: constrained to the canonical treasury stored in the Vault PDA.
    #[account(mut, constraint = treasury.key() == vault.treasury @ ErrorCode::Unauthorized)]
    pub treasury: AccountInfo<'info>,

    /// CHECK: when disabled this must be treasury; when enabled it must be a
    /// distinct system-owned wallet. The handler repeats the party checks.
    #[account(
        mut,
        constraint = (
            (!creator_fee_enabled && ip_creator.key() == treasury.key()) ||
            (creator_fee_enabled && *ip_creator.owner == system_program::ID)
        ) @ ErrorCode::InvalidCreator
    )]
    pub ip_creator: AccountInfo<'info>,

    /// CHECK: native-SOL merchant wallet; program-owned destinations are not
    /// accepted for this v0.6 route.
    #[account(
        mut,
        constraint = *merchant_wallet.owner == system_program::ID @ ErrorCode::InvalidMerchant,
        constraint = !merchant_wallet.executable @ ErrorCode::InvalidMerchant
    )]
    pub merchant_wallet: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(agent_id: String)]
pub struct ReserveSeatSol<'info> {
    #[account(mut, seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(
        init,
        payer = agent,
        space = Seat::LEN,
        seeds = [b"seat", agent.key().as_ref()],
        bump
    )]
    pub seat: Account<'info, Seat>,

    #[account(mut)]
    pub agent: Signer<'info>,

    /// CHECK: Squads multisig treasury
    #[account(mut, constraint = treasury.key() == vault.treasury)]
    pub treasury: AccountInfo<'info>,

    pub sol_price_feed: Account<'info, PriceUpdateV2>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(agent_id: String)]
pub struct ReserveSeatSpl<'info> {
    #[account(mut, seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(
        init,
        payer = agent,
        space = Seat::LEN,
        seeds = [b"seat", agent.key().as_ref()],
        bump
    )]
    pub seat: Account<'info, Seat>,

    #[account(mut)]
    pub agent: Signer<'info>,

    #[account(
        mut,
        constraint = agent_token_account.owner == agent.key(),
        constraint = agent_token_account.mint == treasury_token_account.mint
    )]
    pub agent_token_account: Account<'info, TokenAccount>,

    #[account(mut, constraint = treasury_token_account.owner == vault.treasury)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TopUpSol<'info> {
    #[account(mut, seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        seeds = [b"seat", agent.key().as_ref()],
        bump = seat.bump,
        constraint = seat.agent == agent.key(),
        constraint = seat.asset_type == ASSET_SOL @ ErrorCode::AssetTypeMismatch
    )]
    pub seat: Account<'info, Seat>,

    #[account(mut)]
    pub agent: Signer<'info>,

    /// CHECK: Squads multisig treasury
    #[account(mut, constraint = treasury.key() == vault.treasury)]
    pub treasury: AccountInfo<'info>,

    pub sol_price_feed: Account<'info, PriceUpdateV2>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TopUpSpl<'info> {
    #[account(mut, seeds = [b"vault"], bump = vault.bump)]
    pub vault: Account<'info, Vault>,

    #[account(
        mut,
        seeds = [b"seat", agent.key().as_ref()],
        bump = seat.bump,
        constraint = seat.agent == agent.key(),
        constraint = (seat.asset_type == ASSET_USDC || seat.asset_type == ASSET_USDT) @ ErrorCode::AssetTypeMismatch
    )]
    pub seat: Account<'info, Seat>,

    #[account(mut)]
    pub agent: Signer<'info>,

    #[account(
        mut,
        constraint = agent_token_account.owner == agent.key(),
        constraint = agent_token_account.mint == treasury_token_account.mint
    )]
    pub agent_token_account: Account<'info, TokenAccount>,

    #[account(mut, constraint = treasury_token_account.owner == vault.treasury)]
    pub treasury_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ClaimReferralBonus<'info> {
    #[account(
        mut,
        seeds = [b"seat", referrer.key().as_ref()],
        bump = referrer_seat.bump,
        constraint = referrer_seat.agent == referrer.key(),
        constraint = referrer_seat.key() != referee_seat.key() @ ErrorCode::SelfReferralNotAllowed
    )]
    pub referrer_seat: Account<'info, Seat>,

    #[account(
        mut,
        seeds = [b"seat", referee.key().as_ref()],
        bump = referee_seat.bump,
    )]
    pub referee_seat: Account<'info, Seat>,

    #[account(mut)]
    pub referrer: Signer<'info>,

    /// CHECK: referee pubkey — used for PDA seed derivation only
    pub referee: AccountInfo<'info>,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[error_code]
pub enum ErrorCode {
    #[msg("Minimum donation is $1.00 USD equivalent")]
    DonationTooSmall,
    #[msg("Agent ID must be 64 characters or less")]
    AgentIdTooLong,
    #[msg("Metadata URI must be 128 characters or less")]
    MetadataUriTooLong,
    #[msg("Unsupported asset type — use USDC or USDT for SPL path")]
    UnsupportedAsset,
    #[msg("Pyth price feed is stale or unavailable")]
    PriceFeedStale,
    #[msg("Pyth returned an invalid or low-confidence price")]
    InvalidOraclePrice,
    #[msg("Arithmetic overflow in mSECCO calculation")]
    MathOverflow,
    #[msg("Invalid agreement hash — must match Donation Manifesto SHA-256")]
    InvalidAgreementHash,
    #[msg("Caller is not the referrer recorded on this seat")]
    NotReferrer,
    #[msg("Referral bonus has already been claimed for this referee")]
    ReferralBonusAlreadyClaimed,
    #[msg("Referee has not completed a qualifying $1.00+ transaction")]
    RefereeNotActive,
    #[msg("Self-referral is not allowed")]
    SelfReferralNotAllowed,
    #[msg("Asset type mismatch — cannot top up SOL seat with SPL or vice versa")]
    AssetTypeMismatch,
    #[msg("Caller is not authorized for this operation")]
    Unauthorized,
    #[msg("402_001: Agent daily spending limit reached")]
    DailyLimitExceeded,
    #[msg("402_100: Protocol fee settlement failure — transaction reverted")]
    ProtocolFeeSettlementFailure,
    #[msg("402_IP: Agent passport is not Verified_B2B status")]
    AgentNotVerifiedB2B,
    #[msg("B2B splitter is currently paused by admin")]
    ProtocolPaused,
    #[msg("Partner is not active in the merchant registry")]
    PartnerNotActive,
    #[msg("IP creator wallet does not match passport record")]
    InvalidIpCreator,
    #[msg("B2B merchant amount must be positive and produce a protocol fee")]
    PaymentAmountInvalid,
    #[msg("B2B order ID must contain 1..64 bytes")]
    OrderIdInvalid,
    #[msg("B2B payment ID does not match the canonical v0.6 derivation")]
    PaymentIdMismatch,
    #[msg("B2B creator fee underflows for this merchant amount")]
    CreatorFeeUnderflow,
    #[msg("B2B merchant must be a distinct system-owned wallet")]
    InvalidMerchant,
    #[msg("B2B creator account does not match creator_fee_enabled")]
    InvalidCreator,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parties() -> (Pubkey, Pubkey, Pubkey, Pubkey) {
        (
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
            Pubkey::new_unique(),
        )
    }

    #[test]
    fn fee_on_top_without_creator_keeps_merchant_whole() {
        let split = compute_b2b_split(1_000_000, false).unwrap();
        assert_eq!(split.merchant, 1_000_000);
        assert_eq!(split.treasury_fee, 10_000);
        assert_eq!(split.creator_fee, 0);
        assert_eq!(split.total, 1_010_000);
    }

    #[test]
    fn fee_on_top_with_creator_is_optional_and_exact() {
        let split = compute_b2b_split(1_000_000, true).unwrap();
        assert_eq!(split.merchant, 1_000_000);
        assert_eq!(split.treasury_fee, 10_000);
        assert_eq!(split.creator_fee, 100);
        assert_eq!(split.total, 1_010_100);
    }

    #[test]
    fn micro_amounts_and_overflow_fail_closed() {
        assert!(compute_b2b_split(0, false).is_err());
        assert!(compute_b2b_split(99, false).is_err());
        assert!(compute_b2b_split(9_999, true).is_err());
        assert!(compute_b2b_split(u64::MAX, false).is_err());
    }

    #[test]
    fn payment_id_binds_payer_merchant_amount_creator_and_order() {
        let (payer, merchant, treasury, creator) = parties();
        let base = derive_payment_id(&payer, &merchant, 1_000_000, &treasury, false, "order-1");
        assert_eq!(
            base,
            derive_payment_id(&payer, &merchant, 1_000_000, &treasury, false, "order-1")
        );
        assert_ne!(
            base,
            derive_payment_id(
                &Pubkey::new_unique(),
                &merchant,
                1_000_000,
                &treasury,
                false,
                "order-1"
            )
        );
        assert_ne!(
            base,
            derive_payment_id(
                &payer,
                &Pubkey::new_unique(),
                1_000_000,
                &treasury,
                false,
                "order-1"
            )
        );
        assert_ne!(
            base,
            derive_payment_id(&payer, &merchant, 1_000_001, &treasury, false, "order-1")
        );
        assert_ne!(
            base,
            derive_payment_id(&payer, &merchant, 1_000_000, &creator, true, "order-1")
        );
        assert_ne!(
            base,
            derive_payment_id(&payer, &merchant, 1_000_000, &treasury, false, "order-2")
        );
    }

    #[test]
    fn receipt_pda_is_deterministic_and_payer_scoped() {
        let (payer, merchant, treasury, _) = parties();
        let payment_id =
            derive_payment_id(&payer, &merchant, 1_000_000, &treasury, false, "order-1");
        let first = Pubkey::find_program_address(
            &[b"b2b-payment", payer.as_ref(), payment_id.as_ref()],
            &crate::ID,
        );
        let replay = Pubkey::find_program_address(
            &[b"b2b-payment", payer.as_ref(), payment_id.as_ref()],
            &crate::ID,
        );
        let other_payer = Pubkey::new_unique();
        let other = Pubkey::find_program_address(
            &[b"b2b-payment", other_payer.as_ref(), payment_id.as_ref()],
            &crate::ID,
        );
        assert_eq!(first, replay);
        assert_ne!(first, other);
    }

    #[test]
    fn invalid_party_layouts_fail_closed() {
        let (payer, merchant, treasury, creator) = parties();
        assert!(validate_b2b_parties(&payer, &merchant, &treasury, &treasury, false).is_ok());
        assert!(validate_b2b_parties(&payer, &merchant, &treasury, &creator, true).is_ok());
        assert!(validate_b2b_parties(&payer, &payer, &treasury, &treasury, false).is_err());
        assert!(validate_b2b_parties(&payer, &treasury, &treasury, &treasury, false).is_err());
        assert!(validate_b2b_parties(&payer, &merchant, &treasury, &creator, false).is_err());
        assert!(validate_b2b_parties(&payer, &merchant, &treasury, &treasury, true).is_err());
        assert!(validate_b2b_parties(&payer, &merchant, &treasury, &merchant, true).is_err());
    }
}
