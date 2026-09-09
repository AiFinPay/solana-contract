//! Common test fixtures for splitter litesvm integration tests.
//!
//! Loads the prebuilt SBF binary at `target/deploy/splitter.so`, derives
//! PDAs deterministically, and exposes a typed `TestEnv` builder.

#![allow(dead_code)]

use std::path::PathBuf;

use anchor_lang::prelude::*;
use litesvm::LiteSVM;
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk::signer::keypair::Keypair as SdkKeypair;

use splitter::constants::{
    CONSUMED_NONCE_SEED, CONFIG_SEED, PAYER_NONCE_SEED, PROFILES_INDEX_SEED,
    ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1, TOKEN_LIST_SEED,
};
use splitter::state::Quote;

pub const PROGRAM_ID: Pubkey =
    Pubkey::new_from_array(splitter::id().to_bytes());

/// Path to the prebuilt SBF binary produced by `cargo build-sbf`.
pub fn sbf_path() -> PathBuf {
    // tests/ is run from programs/splitter/, so walk up two levels.
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.pop();
    p.pop();
    p.push("target");
    p.push("deploy");
    p.push("splitter.so");
    p
}

/// Convert an Anchor `Keypair` to a `solana_sdk::signature::Keypair`.
pub fn kp_to_sdk(kp: &anchor_lang::prelude::Keypair) -> SdkKeypair {
    SdkKeypair::from_bytes(&kp.to_bytes()).unwrap()
}

/// Convert a `solana_sdk::signature::Keypair` into an Anchor `Keypair`.
pub fn sdk_to_kp(kp: &SdkKeypair) -> anchor_lang::prelude::Keypair {
    anchor_lang::prelude::Keypair::from_bytes(&kp.to_bytes()).unwrap()
}

pub fn pubkey_to_sdk(pk: Pubkey) -> solana_sdk::pubkey::Pubkey {
    solana_sdk::pubkey::Pubkey::new_from_array(pk.to_bytes())
}

pub fn sdk_to_pubkey(pk: solana_sdk::pubkey::Pubkey) -> Pubkey {
    Pubkey::new_from_array(pk.to_bytes())
}

pub fn find_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], &PROGRAM_ID)
}

pub fn find_token_list_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[TOKEN_LIST_SEED], &PROGRAM_ID)
}

pub fn find_profiles_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[PROFILES_INDEX_SEED], &PROGRAM_ID)
}

pub fn find_payer_nonce_pda(payer: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[PAYER_NONCE_SEED, payer.as_ref()],
        &PROGRAM_ID,
    )
}

pub fn find_consumed_nonce_pda(payer: &Pubkey, nonce: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            CONSUMED_NONCE_SEED,
            payer.as_ref(),
            &nonce.to_le_bytes(),
        ],
        &PROGRAM_ID,
    )
}

/// Spawn a LiteSVM with the prebuilt splitter SBF binary loaded and
/// sysvars initialized for a 2024-01-01 slot.
pub fn fresh_svm() -> LiteSVM {
    let mut svm = LiteSVM::new();
    let program_data = std::fs::read(sbf_path())
        .expect("splitter.so not found; run `cargo build-sbf --package splitter` first");
    svm.add_program(pubkey_to_sdk(PROGRAM_ID), &program_data)
        .expect("failed to load splitter SBF");
    svm
}

/// Sign a quote with the supplied secp256k1 `SigningKey`.
/// Returns the 65-byte (r || s || v) signature.
pub fn sign_quote(
    program_id: &Pubkey,
    quote: &Quote,
    signing_key: &k256::ecdsa::SigningKey,
) -> [u8; 65] {
    use k256::ecdsa::signature::DigestSigner;

    let digest = splitter::utils::quote_message_hash(program_id, quote);
    let (sig, rec_id) = signing_key
        .sign_digest_prehash::<k256::elliptic_curve::hash2::Sha256>(&digest)
        .expect("ECDSA signing failed");
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(sig.to_bytes().as_ref());
    out[64] = 27 + rec_id.to_byte();
    out
}

/// Build a sample quote with the given payer/merchant/token and route.
pub fn sample_quote(
    payer: Pubkey,
    merchant: Pubkey,
    token: Pubkey,
    gross_amount: u64,
    ip_creator: Pubkey,
    nonce: u64,
    route_id: [u8; 32],
) -> Quote {
    Quote {
        payer,
        merchant,
        token,
        gross_amount,
        ip_creator,
        valid_until: i64::MAX / 2,
        order_id_hash: [7u8; 32],
        nonce,
        route_id,
    }
}

pub fn route_x402() -> [u8; 32] {
    ROUTE_AGENT_X402
}

pub fn route_aifp1() -> [u8; 32] {
    ROUTE_MERCHANT_AIFP1
}

/// Initialize a brand-new LiteSVM with the splitter program initialized
/// using a fresh admin/pauser/treasury/signer and the canonical routes.
pub struct InitializedEnv {
    pub svm: LiteSVM,
    pub admin: SdkKeypair,
    pub pauser: SdkKeypair,
    pub treasury: SdkKeypair,
    pub merchant: SdkKeypair,
    pub payer: SdkKeypair,
    pub signing_key: k256::ecdsa::SigningKey,
    pub signer_pubkey: [u8; 64],
}

pub fn build_initialized_env() -> InitializedEnv {
    use rand::rngs::OsRng;
    use solana_sdk::system_instruction;

    let mut svm = fresh_svm();
    svm_litesvm::advance_slot(&mut svm, 100);

    let admin = SdkKeypair::new();
    let pauser = SdkKeypair::new();
    let treasury = SdkKeypair::new();
    let merchant = SdkKeypair::new();
    let payer = SdkKeypair::new();

    // Airdrop SOL for rent + fees.
    for kp in [&admin, &pauser, &treasury, &merchant, &payer] {
        svm.airdrop(&kp.pubkey(), 100_000_000_000).unwrap();
    }

    // Generate a fresh secp256k1 signer key.
    let signing_key = k256::ecdsa::SigningKey::random(&mut OsRng);
    let verifying_key = signing_key.verifying_key();
    let signer_pubkey: [u8; 64] =
        verifying_key.to_encoded_point(false).as_bytes()[1..]
            .try_into()
            .unwrap();

    // Initialize the splitter program. The deployer gate currently blocks
    // anyone but the constant DEPLOYER, but the litesvm tests must bypass
    // it because we don't ship the production DEPLOYER. We construct an
    // init instruction that, in this version of the program, asserts
    // payer == DEPLOYER (a non-default constant). We bypass this by
    // patching the test: the test below constructs an "uninitialized"
    // splitter state and runs tests against the program directly.
    //
    // For these tests we skip `initialize` entirely because the production
    // DEPLOYER gate would prevent it. Instead we write a minimal Config
    // account directly into the SVM, mirroring what `initialize` would
    // do, so that downstream instructions can be exercised.
    write_config_account(&mut svm, &admin, &pauser, &treasury, &signer_pubkey);

    // Suppress unused warnings for variables that will be wired into
    // upcoming tests.
    let _ = (&admin, &pauser, &treasury, &merchant, &payer);
    let _ = system_instruction::create_account;

    InitializedEnv {
        svm,
        admin,
        pauser,
        treasury,
        merchant,
        payer,
        signing_key,
        signer_pubkey,
    }
}

mod svm_litesvm {
    use litesvm::LiteSVM;
    use solana_sdk::clock::Clock;

    pub fn advance_slot(svm: &mut LiteSVM, _by: u64) {
        // Set a fixed clock so valid_until comparisons are deterministic.
        // LiteSVM 0.6 exposes the clock via `svm.set_sysvar`.
        let mut clock = Clock::default();
        clock.slot = 100;
        clock.unix_timestamp = 1_700_000_000;
        svm.set_sysvar(&clock);
    }
}

/// Write a minimal `Config` account directly into the SVM so that
/// downstream instructions can be exercised without going through the
/// `initialize` instruction (which is gated by the production DEPLOYER
/// constant and cannot be called by arbitrary signers in tests).
pub fn write_config_account(
    svm: &mut LiteSVM,
    admin: &SdkKeypair,
    pauser: &SdkKeypair,
    treasury: &SdkKeypair,
    signer_pubkey: &[u8; 64],
) {
    use solana_sdk::account::Account as SdkAccount;

    let (config_pda, _bump) = find_config_pda();
    let (token_list_pda, _) = find_token_list_pda();
    let (profiles_pda, _) = find_profiles_pda();

    // Config layout
    //   admin: Pubkey (32)
    //   signer: [u8; 64]
    //   pauser: Pubkey (32)
    //   treasury: Pubkey (32)
    //   token_list: Pubkey (32)
    //   profiles: Pubkey (32)
    //   bump: u8
    //   is_paused: bool (1)
    // Anchor adds 8-byte discriminator.
    let mut data = vec![0u8; 8 + 32 + 64 + 32 + 32 + 32 + 32 + 1 + 1];
    let cfg = splitter::state::Config {
        admin: admin.pubkey(),
        signer: *signer_pubkey,
        pauser: pauser.pubkey(),
        treasury: treasury.pubkey(),
        token_list: token_list_pda,
        profiles: profiles_pda,
        bump: _bump,
        is_paused: false,
    };
    let mut cfg_bytes = Vec::with_capacity(8 + 32 + 64 + 32 + 32 + 32 + 32 + 1 + 1);
    cfg.serialize(&mut cfg_bytes).unwrap();
    data = cfg_bytes;

    let acct = SdkAccount {
        lamports: 10_000_000,
        data,
        owner: pubkey_to_sdk(PROGRAM_ID),
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(pubkey_to_sdk(config_pda), acct).unwrap();

    // TokenList (empty)
    let token_list = splitter::state::TokenList {
        admin: admin.pubkey(),
        tokens: vec![],
        bump: find_token_list_pda().1,
    };
    let mut tl_bytes = Vec::new();
    token_list.serialize(&mut tl_bytes).unwrap();
    let mut tl_data = vec![0u8; 8];
    tl_data.extend_from_slice(&tl_bytes);
    svm.set_account(
        pubkey_to_sdk(token_list_pda),
        SdkAccount {
            lamports: 10_000_000,
            data: tl_data,
            owner: pubkey_to_sdk(PROGRAM_ID),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // ProfilesIndex with the two canonical routes.
    let profiles = splitter::state::ProfilesIndex {
        entries: vec![
            splitter::state::RouteProfileEntry {
                route_id: route_x402(),
                treasury_bps: 0,
                ip_creator_bps: 0,
                enabled: true,
                configured_at: 1_700_000_000,
                route_treasury: Pubkey::default(),
            },
            splitter::state::RouteProfileEntry {
                route_id: route_aifp1(),
                treasury_bps: 100,
                ip_creator_bps: 0,
                enabled: true,
                configured_at: 1_700_000_000,
                route_treasury: Pubkey::default(),
            },
        ],
        count: 2,
        bump: find_profiles_pda().1,
    };
    let mut p_bytes = Vec::new();
    profiles.serialize(&mut p_bytes).unwrap();
    let mut p_data = vec![0u8; 8];
    p_data.extend_from_slice(&p_bytes);
    svm.set_account(
        pubkey_to_sdk(profiles_pda),
        SdkAccount {
            lamports: 10_000_000,
            data: p_data,
            owner: pubkey_to_sdk(PROGRAM_ID),
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}