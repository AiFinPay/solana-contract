use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Merchant address must be non-zero")]
    ZeroMerchant,
    #[msg("Recovered signer does not match the trusted signer")]
    InvalidSigner,
    #[msg("Signature is malformed or recovery failed")]
    InvalidSignature,
    #[msg("Quote has expired")]
    SignatureExpired,
    #[msg("Quote payer must match transaction signer")]
    InvalidPayer,
    #[msg("Quote nonce does not match payer nonce")]
    InvalidNonce,
    #[msg("Quote nonce has already been consumed")]
    NonceAlreadyConsumed,
    #[msg("Nonce overflow")]
    NonceOverflow,
    #[msg("Native settlement requires token == Pubkey::default()")]
    InvalidTokenForNative,
    #[msg("Stable settlement requires a whitelisted mint (USDC or USDT)")]
    UnsupportedToken,
    #[msg("Unknown route")]
    UnknownRoute,
    #[msg("Signer address must be non-zero")]
    ZeroSigner,
    #[msg("Treasury fee basis points exceeds maximum")]
    TreasuryFeeTooHigh,
    #[msg("IP creator fee basis points exceeds maximum")]
    IPCreatorFeeTooHigh,
    #[msg("Native transfer to merchant failed")]
    MerchantTransferFailed,
    #[msg("Native transfer to treasury failed")]
    TreasuryTransferFailed,
    #[msg("Native transfer to IP creator failed")]
    IPCreatorTransferFailed,
    #[msg("Sent lamports do not match quote gross_amount")]
    IncorrectNativeValue,
    #[msg("Route requires an IP creator address")]
    MissingIPCreator,
    #[msg("Gross amount is too small to produce a non-zero treasury fee")]
    PaymentTooSmallForTreasury,
    #[msg("Gross amount is too small to produce a non-zero royalty")]
    PaymentTooSmallForRoyalty,
}
