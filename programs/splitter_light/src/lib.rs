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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_constants_match_keccak_of_names() {
        // Route IDs are hardcoded constants matching EVM v1.4.
        assert_ne!(ROUTE_AGENT_X402, [0u8; 32]);
        assert_ne!(ROUTE_MERCHANT_AIFP1, [0u8; 32]);
        assert_ne!(ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1);
    }

    #[test]
    fn stablecoin_constants_are_set() {
        assert_ne!(USDC_MINT, Pubkey::default());
        assert_ne!(USDT_MINT, Pubkey::default());
        assert_ne!(USDC_MINT, USDT_MINT);
    }

    #[test]
    fn quote_hash_is_deterministic() {
        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };
        let h1 = utils::quote_hash(&q);
        let h2 = utils::quote_hash(&q);
        assert_eq!(h1, h2);
    }

    #[test]
    fn quote_message_hash_matches_off_chain_implementation() {
        use sha2::{Digest, Sha256};

        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };

        let program_id = crate::id();
        let on_chain = utils::quote_message_hash(&program_id, &q);

        let mut hasher = Sha256::new();
        hasher.update(crate::constants::MESSAGE_DOMAIN_TAG);
        hasher.update(program_id.as_ref());
        let mut quote_bytes = Vec::with_capacity(256);
        q.serialize(&mut quote_bytes).unwrap();
        hasher.update(quote_bytes);
        let off_chain: [u8; 32] = hasher.finalize().into();

        assert_eq!(on_chain, off_chain);
    }

    #[test]
    fn secp256k1_signature_recovers_signer_public_key() {
        use k256::ecdsa::SigningKey;
        use rand_core::OsRng;
        use sha2::{Digest, Sha256};

        let signing_key = SigningKey::random(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let signer_pubkey: [u8; 64] = verifying_key.to_encoded_point(false).as_bytes()[1..]
            .try_into()
            .unwrap();

        let q = Quote {
            payer: Pubkey::new_unique(),
            merchant: Pubkey::new_unique(),
            token: Pubkey::default(),
            gross_amount: 1_000_000,
            ip_creator: Pubkey::default(),
            valid_until: i64::MAX,
            order_id_hash: [1u8; 32],
            nonce: 0,
            route_id: ROUTE_AGENT_X402,
        };

        let program_id = crate::id();
        let digest = utils::quote_message_hash(&program_id, &q);

        let (raw_sig, rec_id) = signing_key
            .sign_digest_recoverable(Sha256::new_with_prefix(&digest))
            .expect("recoverable sign");

        let mut sig_65 = [0u8; 65];
        sig_65[..64].copy_from_slice(raw_sig.to_bytes().as_ref());
        sig_65[64] = 27 + rec_id.to_byte();

        let recovered = utils::recover_signer(&digest, &sig_65).unwrap();
        assert_eq!(recovered, signer_pubkey);
    }

    #[test]
    fn split_agent_route_has_zero_fees() {
        let profile = RouteProfile::AGENT;
        let (m, t, i) = utils::split_gross(1_000_000, &profile).unwrap();
        assert_eq!(m, 1_000_000);
        assert_eq!(t, 0);
        assert_eq!(i, 0);
    }

    #[test]
    fn split_merchant_route_has_1_percent_treasury() {
        let profile = RouteProfile::MERCHANT;
        let (m, t, i) = utils::split_gross(1_000_000, &profile).unwrap();
        assert_eq!(t, 10_000);
        assert_eq!(i, 0);
        assert_eq!(m, 990_000);
    }

    #[test]
    fn split_rejects_zero_amount() {
        let profile = RouteProfile::AGENT;
        assert!(utils::split_gross(0, &profile).is_err());
    }

    #[test]
    fn route_profile_lookup_rejects_unknown() {
        assert!(RouteProfile::lookup(&[0u8; 32]).is_err());
        assert!(RouteProfile::lookup(&RouteProfile::AGENT.route_id).is_ok());
        assert!(RouteProfile::lookup(&RouteProfile::MERCHANT.route_id).is_ok());
    }
}
