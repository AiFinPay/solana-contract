use anchor_lang::prelude::*;

/// Single global config — only stores the current secp256k1 signer.
/// Created lazily via `init_if_needed` on first interaction. Everything else
/// (treasury, stablecoins, route profiles) is hardcoded in code.
#[account]
#[derive(InitSpace)]
pub struct Config {
    pub signer: [u8; 64],
    pub bump: u8,
    pub initialized: bool,
}

/// Per-payer monotonic nonce counter.
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
