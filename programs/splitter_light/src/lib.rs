use anchor_lang::prelude::*;

pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

pub use constants::*;
pub use error::ErrorCode;
pub use instructions::*;
pub use state::*;
pub use utils::*;

declare_id!("7vGTUXSmooih99MuzQELyaeFeZmuo4QcstS7T9Jv7yyR");

#[program]
pub mod splitter_light {
    use super::*;

    pub fn settle_native(
        ctx: Context<SettleNative>,
        nonce: u64,
        quote: Quote,
        signature: [u8; 65],
    ) -> Result<()> {
        crate::instructions::settle_native::handle_settle_native(ctx, nonce, quote, signature)
    }

    pub fn settle_stable<'a>(
        ctx: Context<'a, SettleStable<'a>>,
        nonce: u64,
        quote: Quote,
        signature: [u8; 65],
    ) -> Result<()> {
        crate::instructions::settle_stable::handle_settle_stable(ctx, nonce, quote, signature)
    }

    pub fn set_signer(
        ctx: Context<SetSigner>,
        new_signer: [u8; 64],
        signature: [u8; 65],
    ) -> Result<()> {
        crate::instructions::set_signer::handle_set_signer(ctx, new_signer, signature)
    }
}
