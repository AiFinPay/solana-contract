use anchor_lang::prelude::*;

use crate::{
    constants::*,
    error::ErrorCode,
    state::{Config, ProfilesIndex, RouteProfileEntry, TokenList},
};

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Only the hardcoded deployer may initialize the canonical singleton PDAs.
    /// Enforced both via `address = DEPLOYER` constraint and a runtime check in
    /// the handler (belt-and-braces).
    #[account(
        mut,
        address = DEPLOYER @ ErrorCode::InvalidDeployer,
    )]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + Config::INIT_SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,

    #[account(
        init,
        payer = payer,
        space = 8 + TokenList::INIT_SPACE,
        seeds = [TOKEN_LIST_SEED],
        bump
    )]
    pub token_list: Account<'info, TokenList>,

    #[account(
        init,
        payer = payer,
        space = 8 + ProfilesIndex::INIT_SPACE,
        seeds = [crate::constants::PROFILES_INDEX_SEED],
        bump
    )]
    pub profiles: Account<'info, ProfilesIndex>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct InitializeParams {
    pub admin: Pubkey,
    pub signer: [u8; 64],
    pub pauser: Pubkey,
    pub treasury: Pubkey,
    pub stablecoins: Vec<Pubkey>,
    pub route_ids: Vec<[u8; 32]>,
    pub treasury_bps: Vec<u16>,
    pub ip_creator_bps: Vec<u16>,
}

pub fn handle_initialize(ctx: Context<Initialize>, params: InitializeParams) -> Result<()> {
    let clock = Clock::get()?;

    // The deployer gate is enforced unconditionally. The `DEPLOYER` constant
    // must be set to the real deployer/multisig pubkey before mainnet
    // deployment. A `build.rs` guard fails the build if DEPLOYER equals
    // `Pubkey::default()` in non-test profiles.
    require!(!DEPLOYER.eq(&Pubkey::default()), ErrorCode::ZeroDeployer);
    require!(
        ctx.accounts.payer.key().eq(&DEPLOYER),
        ErrorCode::InvalidDeployer
    );

    require!(!params.admin.eq(&Pubkey::default()), ErrorCode::ZeroAdmin);
    require!(params.signer != [0u8; 64], ErrorCode::ZeroSigner);
    require!(!params.pauser.eq(&Pubkey::default()), ErrorCode::ZeroPauser);
    require!(
        !params.treasury.eq(&Pubkey::default()),
        ErrorCode::ZeroTreasury
    );

    // No role-separation check: admin, pauser and treasury may share one
    // address (e.g. a single Squads multisig vault). Separation, if wanted,
    // is an operational policy, not an on-chain invariant.

    let route_count = params.route_ids.len();
    require!(route_count > 0, ErrorCode::UnknownRoute);
    require!(
        route_count == params.treasury_bps.len() && route_count == params.ip_creator_bps.len(),
        ErrorCode::ArrayLengthMismatch
    );
    require!(route_count <= MAX_ROUTES, ErrorCode::UnknownRoute);
    require!(
        params.stablecoins.len() <= MAX_TOKENS,
        ErrorCode::TokenListFull
    );

    for mint in &params.stablecoins {
        require!(!mint.eq(&Pubkey::default()), ErrorCode::ZeroStablecoin);
    }

    let mut entries = Vec::with_capacity(route_count);
    for i in 0..route_count {
        let treasury_bps = params.treasury_bps[i];
        let ip_creator_bps = params.ip_creator_bps[i];
        require!(
            treasury_bps <= MAX_TREASURY_BPS,
            ErrorCode::TreasuryFeeTooHigh
        );
        require!(
            ip_creator_bps <= MAX_IP_CREATOR_BPS,
            ErrorCode::IPCreatorFeeTooHigh
        );
        let aggregate_bps = treasury_bps as u32 + ip_creator_bps as u32;
        require!(
            aggregate_bps <= MAX_AGGREGATE_BPS as u32,
            ErrorCode::AggregateFeeTooHigh
        );

        entries.push(RouteProfileEntry {
            route_id: params.route_ids[i],
            treasury_bps,
            ip_creator_bps,
            enabled: true,
            configured_at: clock.unix_timestamp,
            route_treasury: Pubkey::default(),
        });
    }

    let config = &mut ctx.accounts.config;
    config.admin = params.admin;
    config.signer = params.signer;
    config.pauser = params.pauser;
    config.treasury = params.treasury;
    config.token_list = ctx.accounts.token_list.key();
    config.profiles = ctx.accounts.profiles.key();
    config.bump = ctx.bumps.config;
    config.is_paused = false;

    let token_list = &mut ctx.accounts.token_list;
    token_list.admin = config.key();
    token_list.tokens = params.stablecoins;
    token_list.bump = ctx.bumps.token_list;

    let profiles = &mut ctx.accounts.profiles;
    profiles.entries = entries;
    profiles.count = route_count as u8;
    profiles.bump = ctx.bumps.profiles;

    msg!("AiFinPay Solana splitter v1.4 initialized");
    Ok(())
}
