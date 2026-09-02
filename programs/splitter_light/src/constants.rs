use anchor_lang::prelude::*;

/// Solana-native message domain tag. Included in the signed message prefix to
/// make the digest domain-separated and versioned. Not EIP-712.
pub const MESSAGE_DOMAIN_TAG: &[u8] = b"AiFinPay-Solana-v1.4";

/// Basis-point denominator.
pub const BPS_DENOMINATOR: u64 = 10_000;

/// Hardcoded fee caps.
pub const MAX_TREASURY_BPS: u16 = 500;
pub const MAX_IP_CREATOR_BPS: u16 = 100;

/// Canonical route identifiers (keccak256 of the route name).
/// MUST match EVM v1.4 exactly.
pub const ROUTE_AGENT_X402: [u8; 32] = [
    0x8d, 0xc5, 0x05, 0xbe, 0x33, 0x5e, 0x56, 0x5d, 0x2a, 0x5e, 0x2c, 0x96, 0x05, 0x7c, 0x7f, 0xb0,
    0xca, 0xff, 0x7c, 0x50, 0x09, 0xf6, 0x1b, 0x82, 0xac, 0x3e, 0xf5, 0xe7, 0xa9, 0xec, 0x0f, 0x1e,
];
pub const ROUTE_MERCHANT_AIFP1: [u8; 32] = [
    0xb9, 0xdb, 0xf5, 0x87, 0xb0, 0xdf, 0x69, 0x87, 0x0d, 0xf1, 0xe6, 0x0b, 0x22, 0xfb, 0xa0, 0x31,
    0x7f, 0x53, 0xeb, 0x19, 0xd7, 0x8a, 0x57, 0x3a, 0xbf, 0x94, 0xfc, 0x38, 0x4a, 0x33, 0x9a, 0x89,
];

/// Hardcoded SPL stablecoin mints (USDC, USDT) — mainnet-beta addresses.
/// The set_signer / settle_stable handlers accept ONLY these two mints.
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const USDT_MINT: Pubkey = pubkey!("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB");

/// Hardcoded protocol treasury wallet (receives treasury fees for every route).
///
/// ⚠️ DEPLOYMENT-CRITICAL PLACEHOLDER: this is *not* a real treasury wallet.
/// It is a syntactically valid placeholder (`[0xAF; 32]`) that lets the program
/// compile and pass tests, but any treasury fees sent here would be lost. Replace
/// with the real protocol-owned wallet pubkey before mainnet deployment.
pub const PROTOCOL_TREASURY: Pubkey = Pubkey::new_from_array([0xAF; 32]);

/// Hardcoded initial secp256k1 signer (uncompressed pubkey, 64 bytes).
///
/// ⚠️ DEPLOYMENT-CRITICAL PLACEHOLDER: this is the secp256k1 generator point G
/// (private key = 1). It is a valid curve point so `set_signer` and settlement
/// tests can run, but it is publicly known. The deployer **must** replace this
/// with their own bootstrap secp256k1 public key before deploying to mainnet.
pub const INITIAL_SIGNER: [u8; 64] = [
    // x-coordinate of secp256k1 generator point G
    0x79, 0xBE, 0x66, 0x7E, 0xF9, 0xDC, 0xBB, 0xAC, 0x55, 0xA0, 0x62, 0x95, 0xCE, 0x87, 0x0B, 0x07,
    0x02, 0x9B, 0xFC, 0xDB, 0x2D, 0xCE, 0x28, 0xD9, 0x59, 0xF2, 0x81, 0x5B, 0x16, 0xF8, 0x17, 0x98,
    // y-coordinate of secp256k1 generator point G
    0x48, 0x3A, 0xDA, 0x77, 0x26, 0xA3, 0xC4, 0x65, 0x5D, 0xA4, 0xFB, 0xFC, 0x0E, 0x11, 0x08, 0xA8,
    0xFD, 0x17, 0xB4, 0x48, 0xA6, 0x85, 0x54, 0x19, 0x9C, 0x47, 0xD0, 0x8F, 0xFB, 0x10, 0xD4, 0xB8,
];

/// PDA seeds.
pub const CONFIG_SEED: &[u8] = b"config";
pub const PAYER_NONCE_SEED: &[u8] = b"payer-nonce";
pub const CONSUMED_NONCE_SEED: &[u8] = b"consumed-nonce";
