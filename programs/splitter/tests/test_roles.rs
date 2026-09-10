//! litesvm integration tests for role management, pause/unpause, and route config.
//!
//! These tests write Config + ProfilesIndex accounts directly into the SVM
//! (bypassing `initialize`) so the deployer gate is not a factor.
//!
//! Error-path tests use `assert!(result.is_err())` only — the current
//! platform-tools/litesvm combination panics with "capacity overflow" when
//! Anchor's `require!` returns an error, so error codes cannot be pinned.
//!
//! Requires `target/deploy/splitter.so` (run `cargo build-sbf` first).

use anchor_lang::prelude::Pubkey;
use anchor_lang::AnchorSerialize;
use litesvm::LiteSVM;
use solana_instruction::{AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

const CONFIG_SEED: &[u8] = b"config";
const PROFILES_INDEX_SEED: &[u8] = b"profiles-index";

fn sighash(name: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{name}"));
    hasher.finalize()[..8].try_into().unwrap()
}

fn account_sighash() -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update("account:Config");
    hasher.finalize()[..8].try_into().unwrap()
}

fn profiles_account_sighash() -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update("account:ProfilesIndex");
    hasher.finalize()[..8].try_into().unwrap()
}

struct Env {
    svm: LiteSVM,
    program_id: Pubkey,
    config: Pubkey,
    profiles: Pubkey,
    admin: Keypair,
}

fn setup(extra_signers: &[&Keypair]) -> Env {
    let elf_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/deploy/splitter.so");
    let elf = std::fs::read(&elf_path).expect("missing splitter.so; run cargo build-sbf");
    let program_id = Pubkey::new_from_array(splitter::ID.to_bytes());
    let mut svm = LiteSVM::new();
    svm.add_program(program_id, &elf).unwrap();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();
    for s in extra_signers {
        svm.airdrop(&s.pubkey(), 10_000_000_000).unwrap();
    }

    let (config, bump) = Pubkey::find_program_address(&[CONFIG_SEED], &program_id);
    let (profiles, profiles_bump) =
        Pubkey::find_program_address(&[PROFILES_INDEX_SEED], &program_id);

    let cfg = splitter::Config {
        admin: admin.pubkey(),
        signer: [7u8; 64],
        pauser: Pubkey::new_unique(),
        treasury: Pubkey::new_unique(),
        token_list: Pubkey::new_unique(),
        profiles,
        bump,
        is_paused: false,
    };
    let mut body = Vec::new();
    cfg.serialize(&mut body).unwrap();
    let mut data = account_sighash().to_vec();
    data.extend_from_slice(&body);
    svm.set_account(
        config,
        solana_account::Account {
            lamports: 10_000_000,
            data,
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    // ProfilesIndex pre-allocated for MAX_ROUTES (32) entries so
    // configure_route can push a new route without AccountDidNotSerialize.
    // Layout: discriminator(8) + vec_len(4) + 32 * entry(77) + count(1) + bump(1) = 2479
    let max_profiles_data_len = 8 + 4 + 32 * 77 + 1 + 1;
    let mut p_data = vec![0u8; max_profiles_data_len];
    p_data[..8].copy_from_slice(&profiles_account_sighash());
    let profiles_entry = splitter::state::ProfilesIndex {
        entries: vec![splitter::state::RouteProfileEntry {
            route_id: splitter::ROUTE_AGENT_X402,
            treasury_bps: 0,
            ip_creator_bps: 0,
            enabled: true,
            configured_at: 1_700_000_000,
            route_treasury: Pubkey::default(),
        }],
        count: 1,
        bump: profiles_bump,
    };
    let mut p_body = Vec::new();
    profiles_entry.serialize(&mut p_body).unwrap();
    p_data[8..8 + p_body.len()].copy_from_slice(&p_body);
    svm.set_account(
        profiles,
        solana_account::Account {
            lamports: 10_000_000,
            data: p_data,
            owner: program_id,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();

    Env {
        svm,
        program_id,
        config,
        profiles,
        admin,
    }
}

/// Send an instruction with [config, signer] account order.
#[allow(clippy::result_large_err)]
fn send_ix(
    env: &mut Env,
    ix_name: &str,
    args: &[u8],
    signer: &Keypair,
) -> litesvm::types::TransactionResult {
    let mut data = sighash(ix_name).to_vec();
    data.extend_from_slice(args);
    let accounts = vec![
        AccountMeta::new(env.config, false),
        AccountMeta::new_readonly(signer.pubkey(), true),
    ];
    let ix = Instruction {
        program_id: env.program_id,
        accounts,
        data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx)
}

/// Send an instruction with [config, profiles, signer] account order.
#[allow(clippy::result_large_err)]
fn send_ix_with_profiles(
    env: &mut Env,
    ix_name: &str,
    args: &[u8],
    signer: &Keypair,
) -> litesvm::types::TransactionResult {
    let mut data = sighash(ix_name).to_vec();
    data.extend_from_slice(args);
    let accounts = vec![
        AccountMeta::new(env.config, false),
        AccountMeta::new(env.profiles, false),
        AccountMeta::new_readonly(signer.pubkey(), true),
    ];
    let ix = Instruction {
        program_id: env.program_id,
        accounts,
        data,
    };
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&signer.pubkey()),
        &[signer],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx)
}

fn stored_config(env: &Env) -> Vec<u8> {
    env.svm.get_account(&env.config).unwrap().data.clone()
}

fn stored_profiles(env: &Env) -> Vec<u8> {
    env.svm.get_account(&env.profiles).unwrap().data.clone()
}

fn admin_clone(env: &Env) -> Keypair {
    Keypair::new_from_array(<[u8; 32]>::try_from(env.admin.to_bytes()[..32].to_vec()).unwrap())
}

// ---------------------------------------------------------------------------
// rotate_admin_role
// ---------------------------------------------------------------------------

#[test]
fn rotate_admin_role_success() {
    let new_admin = Pubkey::new_unique();
    let mut env = setup(&[]);
    let before = stored_config(&env);
    let admin = admin_clone(&env);

    let res = send_ix(&mut env, "rotate_admin_role", &new_admin.to_bytes(), &admin);
    assert!(res.is_ok(), "rotate_admin failed: {res:?}");

    let after = stored_config(&env);
    // admin field is at offset 8..40 (8-byte discriminator + 32-byte pubkey)
    assert_eq!(&after[8..40], new_admin.as_ref());
    // signer, pauser, treasury unchanged
    assert_eq!(&after[0..8], &before[0..8]);
    assert_eq!(&after[40..], &before[40..]);
}

#[test]
fn rotate_admin_role_rejects_non_admin() {
    let stranger = Keypair::new();
    let mut env = setup(&[&stranger]);
    let admin_bytes = env.admin.pubkey().to_bytes();

    let res = send_ix(
        &mut env,
        "rotate_admin_role",
        &Pubkey::new_unique().to_bytes(),
        &stranger,
    );
    assert!(res.is_err());
    // State must not change.
    assert_eq!(&stored_config(&env)[8..40], &admin_bytes);
}

// ---------------------------------------------------------------------------
// rotate_pauser_role
// ---------------------------------------------------------------------------

#[test]
fn rotate_pauser_role_success() {
    let new_pauser = Pubkey::new_unique();
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let res = send_ix(
        &mut env,
        "rotate_pauser_role",
        &new_pauser.to_bytes(),
        &admin,
    );
    assert!(res.is_ok(), "rotate_pauser failed: {res:?}");

    let after = stored_config(&env);
    // pauser field is at offset 104..136
    assert_eq!(&after[104..136], &new_pauser.to_bytes());
}

// ---------------------------------------------------------------------------
// grant_pauser_role
// ---------------------------------------------------------------------------

#[test]
fn grant_pauser_role_success() {
    let new_pauser = Pubkey::new_unique();
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let res = send_ix(
        &mut env,
        "grant_pauser_role",
        &new_pauser.to_bytes(),
        &admin,
    );
    assert!(res.is_ok(), "grant_pauser failed: {res:?}");
    assert_eq!(&stored_config(&env)[104..136], &new_pauser.to_bytes());
}

// ---------------------------------------------------------------------------
// set_treasury
// ---------------------------------------------------------------------------

#[test]
fn set_treasury_success() {
    let new_treasury = Pubkey::new_unique();
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let res = send_ix(&mut env, "set_treasury", &new_treasury.to_bytes(), &admin);
    assert!(res.is_ok(), "set_treasury failed: {res:?}");
    // treasury field is at offset 136..168
    assert_eq!(&stored_config(&env)[136..168], &new_treasury.to_bytes());
}

// ---------------------------------------------------------------------------
// grant_signer_role
// ---------------------------------------------------------------------------

#[test]
fn grant_signer_role_success() {
    let new_signer = [9u8; 64];
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let res = send_ix(&mut env, "grant_signer_role", &new_signer, &admin);
    assert!(res.is_ok(), "grant_signer failed: {res:?}");
    // signer field is at offset 40..104
    assert_eq!(&stored_config(&env)[40..104], &new_signer);
}

// ---------------------------------------------------------------------------
// rotate_signer_role
// ---------------------------------------------------------------------------

#[test]
fn rotate_signer_role_success() {
    let new_signer = [0xABu8; 64];
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let res = send_ix(&mut env, "rotate_signer_role", &new_signer, &admin);
    assert!(res.is_ok(), "rotate_signer failed: {res:?}");
    assert_eq!(&stored_config(&env)[40..104], &new_signer);
}

// ---------------------------------------------------------------------------
// pause / unpause
// ---------------------------------------------------------------------------

#[test]
fn pause_success() {
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let res = send_ix(&mut env, "pause", &[], &admin);
    assert!(res.is_ok(), "pause failed: {res:?}");

    let data = stored_config(&env);
    // is_paused is the last byte of the Config account
    assert_eq!(data[data.len() - 1], 1, "is_paused should be true");
}

#[test]
fn unpause_success() {
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    // Pause first.
    let res = send_ix(&mut env, "pause", &[], &admin);
    assert!(res.is_ok());
    assert_eq!(stored_config(&env).last(), Some(&1u8));

    // Unpause.
    let res = send_ix(&mut env, "unpause", &[], &admin);
    assert!(res.is_ok(), "unpause failed: {res:?}");
    assert_eq!(stored_config(&env).last(), Some(&0u8));
}

#[test]
fn unpause_by_pauser_success() {
    let pauser = Keypair::new();
    let mut env = setup(&[&pauser]);
    let admin = admin_clone(&env);

    // Set pauser to our keypair.
    let res = send_ix(
        &mut env,
        "grant_pauser_role",
        &pauser.pubkey().to_bytes(),
        &admin,
    );
    assert!(res.is_ok());

    // Pause (admin).
    let res = send_ix(&mut env, "pause", &[], &admin);
    assert!(res.is_ok());

    // Unpause (pauser).
    let res = send_ix(&mut env, "unpause", &[], &pauser);
    assert!(res.is_ok(), "unpause by pauser failed: {res:?}");
    assert_eq!(stored_config(&env).last(), Some(&0u8));
}

#[test]
fn pause_rejects_unauthorized() {
    let stranger = Keypair::new();
    let mut env = setup(&[&stranger]);

    let res = send_ix(&mut env, "pause", &[], &stranger);
    assert!(res.is_err());
    // Must still be unpaused.
    assert_eq!(stored_config(&env).last(), Some(&0u8));
}

// ---------------------------------------------------------------------------
// configure_route
// ---------------------------------------------------------------------------

fn make_configure_args(
    route_id: [u8; 32],
    treasury_bps: u16,
    ip_creator_bps: u16,
    route_treasury: Pubkey,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&route_id);
    data.extend_from_slice(&treasury_bps.to_le_bytes());
    data.extend_from_slice(&ip_creator_bps.to_le_bytes());
    data.extend_from_slice(&route_treasury.to_bytes());
    data
}

#[test]
fn configure_route_creates_new() {
    let new_route = [0xAAu8; 32];
    let treasury = Pubkey::new_unique();
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let args = make_configure_args(new_route, 100, 50, treasury);
    let res = send_ix_with_profiles(&mut env, "configure_route", &args, &admin);
    assert!(res.is_ok(), "configure_route create failed: {res:?}");

    let data = stored_profiles(&env);
    let count = u32::from_le_bytes(data[8..12].try_into().unwrap());
    assert_eq!(count, 2, "should now have 2 routes (default + new)");
}

#[test]
fn configure_route_updates_existing() {
    let mut env = setup(&[]);
    let admin = admin_clone(&env);
    let new_bps: u16 = 250;
    let treasury = Pubkey::new_unique();

    let args = make_configure_args(splitter::ROUTE_AGENT_X402, new_bps, 0, treasury);
    let res = send_ix_with_profiles(&mut env, "configure_route", &args, &admin);
    assert!(res.is_ok(), "configure_route update failed: {res:?}");

    let data = stored_profiles(&env);
    let count = u32::from_le_bytes(data[8..12].try_into().unwrap());
    assert_eq!(count, 1, "should still have 1 route");
}

// ---------------------------------------------------------------------------
// disable_route / enable_route
// ---------------------------------------------------------------------------

#[test]
fn disable_route_success() {
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    let mut args = Vec::new();
    args.extend_from_slice(&splitter::ROUTE_AGENT_X402);
    let res = send_ix_with_profiles(&mut env, "disable_route", &args, &admin);
    assert!(res.is_ok(), "disable_route failed: {res:?}");

    let data = stored_profiles(&env);
    assert_eq!(data[48], 0, "enabled should be false");
}

#[test]
fn enable_route_success() {
    let mut env = setup(&[]);
    let admin = admin_clone(&env);

    // Disable first.
    let mut args = Vec::new();
    args.extend_from_slice(&splitter::ROUTE_AGENT_X402);
    let res = send_ix_with_profiles(&mut env, "disable_route", &args, &admin);
    assert!(res.is_ok());
    let data = stored_profiles(&env);
    assert_eq!(data[48], 0);

    // Re-enable.
    let res = send_ix_with_profiles(&mut env, "enable_route", &args, &admin);
    assert!(res.is_ok(), "enable_route failed: {res:?}");
    let data = stored_profiles(&env);
    assert_eq!(data[48], 1, "enabled should be true");
}
