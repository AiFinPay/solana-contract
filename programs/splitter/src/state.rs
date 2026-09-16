use anchor_lang::prelude::*;

use crate::constants::{MAX_ROUTES, MAX_TOKENS};

/// Global configuration and RBAC registry.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub admin: Pubkey,
    pub signer: [u8; 64], // secp256k1 uncompressed public key
    pub pauser: Pubkey,
    pub treasury: Pubkey,
    pub token_list: Pubkey,
    pub profiles: Pubkey,
    pub bump: u8,
    pub is_paused: bool,
}

/// Whitelisted stablecoin mints.
#[account]
#[derive(InitSpace)]
pub struct TokenList {
    pub admin: Pubkey,
    #[max_len(MAX_TOKENS)]
    pub tokens: Vec<Pubkey>,
    pub bump: u8,
}

impl TokenList {
    pub fn is_allowed(&self, mint: Pubkey) -> bool {
        self.tokens.iter().any(|t| t.eq(&mint))
    }
}

/// One route profile entry.
#[account]
#[derive(InitSpace)]
pub struct RouteProfileEntry {
    pub route_id: [u8; 32],
    pub treasury_bps: u16,
    pub ip_creator_bps: u16,
    pub enabled: bool,
    pub configured_at: i64,
    pub route_treasury: Pubkey,
}

/// Registry of all route profiles.
#[account]
#[derive(InitSpace)]
pub struct ProfilesIndex {
    #[max_len(MAX_ROUTES)]
    pub entries: Vec<RouteProfileEntry>,
    pub count: u8,
    pub bump: u8,
}

/// Per-payer monotonic nonce.
#[account]
#[derive(InitSpace)]
pub struct PayerNonce {
    pub payer: Pubkey,
    pub nonce: u64,
    pub bump: u8,
}

/// Marker that a (payer, nonce) pair has been consumed.
#[account]
#[derive(InitSpace)]
pub struct ConsumedNonce {
    pub payer: Pubkey,
    pub nonce: u64,
    pub consumed: bool,
    pub bump: u8,
}

/// Signed quote submitted by the payer.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Quote {
    pub payer: Pubkey,
    pub merchant: Pubkey,
    pub token: Pubkey,
    pub gross_amount: u64,
    pub ip_creator: Pubkey,
    pub valid_until: i64,
    pub order_id_hash: [u8; 32],
    pub nonce: u64,
    pub route_id: [u8; 32],
}
