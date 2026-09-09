use anchor_lang::prelude::*;

/// Solana-native message domain tag. Included in the signed message prefix to
/// make the digest domain-separated and versioned. Not EIP-712.
pub const MESSAGE_DOMAIN_TAG: &[u8] = b"AiFinPay-Solana-v1.4";

/// Basis-point denominator.
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Maximum quote lifetime in seconds. A quote with `valid_until` further
/// than this many seconds in the future is rejected at settlement time.
/// SOL-LOW-004.
pub const MAX_QUOTE_LIFETIME_SECONDS: i64 = 60 * 60;

/// Per-route fee caps.
pub const MAX_TREASURY_BPS: u16 = 500;
pub const MAX_IP_CREATOR_BPS: u16 = 100;
/// Aggregate fee cap (treasury + IP creator). Cannot exceed 100% or the merchant amount underflow.
pub const MAX_AGGREGATE_BPS: u16 = 1_000;

/// Canonical route identifiers (keccak256 of the route name).
/// Matches the EVM v1.4 deployment exactly.
pub const ROUTE_AGENT_X402: [u8; 32] = [
    0x8d, 0xc5, 0x05, 0xbe, 0x33, 0x5e, 0x56, 0x5d, 0x2a, 0x5e, 0x2c, 0x96, 0x05, 0x7c, 0x7f, 0xb0,
    0xca, 0xff, 0x7c, 0x50, 0x09, 0xf6, 0x1b, 0x82, 0xac, 0x3e, 0xf5, 0xe7, 0xa9, 0xec, 0x0f, 0x1e,
];
pub const ROUTE_MERCHANT_AIFP1: [u8; 32] = [
    0xb9, 0xdb, 0xf5, 0x87, 0xb0, 0xdf, 0x69, 0x87, 0x0d, 0xf1, 0xe6, 0x0b, 0x22, 0xfb, 0xa0, 0x31,
    0x7f, 0x53, 0xeb, 0x19, 0xd7, 0x8a, 0x57, 0x3a, 0xbf, 0x94, 0xfc, 0x38, 0x4a, 0x33, 0x9a, 0x89,
];

/// Token-list capacity.
pub const MAX_TOKENS: usize = 16;
/// Maximum number of route profiles.
pub const MAX_ROUTES: usize = 32;

/// PDA seeds.
pub const CONFIG_SEED: &[u8] = b"config";
pub const TOKEN_LIST_SEED: &[u8] = b"token-list";
pub const ROUTE_PROFILE_SEED: &[u8] = b"route-profile";
pub const PAYER_NONCE_SEED: &[u8] = b"payer-nonce";
pub const CONSUMED_NONCE_SEED: &[u8] = b"consumed-nonce";
pub const PROFILES_INDEX_SEED: &[u8] = b"profiles-index";

/// Authorized deployer. This pubkey is the only signer allowed to call `initialize`.
/// MUST be set to the real deployer/multisig pubkey before mainnet deployment.
/// CI guard: build.rs fails the build if this equals `Pubkey::default()`.
/// The bytes below are the placeholder deployer multisig
/// (`DEPLOYERmXsG4qfz9rYp9Vc4Qv7Vz3aqCnLhVtNkRwSjTbU`) and MUST be
/// replaced before mainnet deployment.
pub const DEPLOYER: Pubkey = Pubkey::new_from_array([
    0xDE, 0xA0, 0xD0, 0xBE, 0x57, 0x14, 0x59, 0xCC, 0x1D, 0xDE, 0xE5, 0x53, 0xA1, 0x57, 0x29, 0x6B,
    0x17, 0xD0, 0x8F, 0x8F, 0x6B, 0x42, 0xC2, 0x6B, 0x35, 0x70, 0x2F, 0xC4, 0xB4, 0x00, 0x00, 0x01,
]);
