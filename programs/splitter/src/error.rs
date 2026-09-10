use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Caller is not authorized for this operation")]
    Unauthorized,
    #[msg("Amount must be greater than zero")]
    ZeroAmount,
    #[msg("Merchant address must be non-zero")]
    ZeroMerchant,
    #[msg("Treasury address must be non-zero")]
    ZeroTreasury,
    #[msg("Token is not whitelisted for stable settlement")]
    UnsupportedToken,
    #[msg("Gross amount is too small to produce a non-zero royalty")]
    PaymentTooSmallForRoyalty,
    #[msg("Gross amount is too small to produce a non-zero treasury fee")]
    PaymentTooSmallForTreasury,
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
    #[msg("IP creator account does not match the quote")]
    IPCreatorMismatch,
    #[msg("Recovered signer does not hold SIGN_OPERATOR_ROLE")]
    InvalidSigner,
    #[msg("Signature is malformed or recovery failed")]
    InvalidSignature,
    #[msg("Signature length must be 65 bytes")]
    InvalidSignatureLength,
    #[msg("Quote has expired")]
    SignatureExpired,
    #[msg("Quote payer must match transaction signer")]
    InvalidPayer,
    #[msg("Quote merchant must match the provided merchant account")]
    MerchantMismatch,
    #[msg("Quote nonce does not match payer nonce")]
    InvalidNonce,
    #[msg("Quote nonce has already been consumed")]
    NonceAlreadyConsumed,
    #[msg("Nonce overflow")]
    NonceOverflow,
    #[msg("Native settlement requires token == Pubkey::default()")]
    InvalidTokenForNative,
    #[msg("Route is disabled")]
    RouteDisabled,
    #[msg("Unknown route")]
    UnknownRoute,
    #[msg("Treasury fee basis points exceeds maximum")]
    TreasuryFeeTooHigh,
    #[msg("IP creator fee basis points exceeds maximum")]
    IPCreatorFeeTooHigh,
    #[msg("Aggregate fees exceed maximum")]
    AggregateFeeTooHigh,
    #[msg("Fees exceed gross amount")]
    FeeExceedsGross,
    #[msg("Signer address must be non-zero")]
    ZeroSigner,
    #[msg("Pauser address must be non-zero")]
    ZeroPauser,
    // Legacy, no longer raised: role separation (admin/pauser/treasury
    // distinctness) was removed — a single multisig may hold all roles.
    // Kept in place so error codes after it keep their pinned numbers.
    #[msg("Pauser and signer must be distinct")]
    PauserEqualsSigner,
    // Legacy, no longer raised (see above). Kept for pinned error codes.
    #[msg("Admin and signer/pauser must be distinct")]
    AdminEqualsSigner,
    #[msg("Admin address must be non-zero")]
    ZeroAdmin,
    #[msg("Deployer address must be non-zero")]
    ZeroDeployer,
    #[msg("Caller is not the authorized deployer")]
    InvalidDeployer,
    #[msg("Token and flag array lengths must match")]
    ArrayLengthMismatch,
    #[msg("Token list is full")]
    TokenListFull,
    #[msg("Program is currently paused")]
    ProtocolPaused,
    #[msg("Profile route_treasury must be non-zero when set")]
    RouteTreasuryZero,
    #[msg("Stablecoin mint address must be non-zero")]
    ZeroStablecoin,
    #[msg("Duplicate token mint in update")]
    DuplicateToken,
    #[msg("Settlement accounts must be distinct from the payer and from each other")]
    DuplicateSettlementAccount,
    #[msg("Settlement recipient is a protocol PDA; funds would be locked")]
    ProtocolAccountMisuse,
    #[msg("Token account owner does not match expected owner")]
    TokenAccountOwnerMismatch,
    #[msg("Mint account is not a valid SPL Mint owned by the token program")]
    InvalidMint,
    #[msg("Settlement destination account is not writable")]
    DestinationNotWritable,
    #[msg("Recovery ID out of range; expected 0 or 1")]
    InvalidRecoveryId,
}
