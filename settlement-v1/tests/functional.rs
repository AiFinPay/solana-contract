// Functional tests against the real runtime, running the COMPILED .so in the
// SBF VM (processor = None + SBF_OUT_DIR). Running natively via processor!()
// is not possible here: cross-program `invoke` (the transfers) panics with
// "only supported with target_os = solana" on the host. The VM path exercises
// the real CPIs.
//
// Focus: the H-1 dust-hardening fix on the RECEIPT side — PDA initialization
// state is defined by DATA, never by lamports, so dusting a predicted receipt
// address cannot block a legitimate payment, while a real written receipt
// still rejects as replay. Every negative test asserts the program's own
// stable error code.
//
// The CONFIG-side H-1 fix (initialize_config dusting) requires the upgradeable
// loader's Program/ProgramData accounts, which solana-program-test does not
// install cleanly; that path is verified at deploy time (see DEPLOY-RUNBOOK.md
// step 3 + the paused-negative test). Not faked here.
//
// Requires the built artifact. The test binary sets SBF_OUT_DIR to
// target/deploy; run `cargo-build-sbf` first (scripts/verify-build.sh does).

use solana_program_test::{BanksClient, ProgramTest};
use solana_sdk::{
    account::Account,
    hash::Hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_instruction, system_program,
    transaction::{Transaction, TransactionError},
};

const CONFIG_SEED: &[u8] = b"aifinpay-settlement-config-v1";
const RECEIPT_SEED: &[u8] = b"aifinpay-settlement-receipt-v1";
const CONFIG_MAGIC: &[u8; 8] = b"AIFPCFG1";
const CONFIG_LEN: usize = 138;
const RECEIPT_LEN: usize = 170;

const ERR_PAUSED: u32 = 7006;
const ERR_EXPIRED: u32 = 7007;
const ERR_INVALID_TREASURY: u32 = 7011;
const ERR_REPLAY: u32 = 7014;

// Rent-exempt minimum for a 0-data system account (mainnet/genesis default).
// A dust attack must send at least this — a smaller transfer is rejected by
// the runtime's rent check — but it is still a cheap grief, so the fix matters.
const RENT_EXEMPT_ZERO: u64 = 890_880;

fn program_id() -> Pubkey {
    Pubkey::new_from_array([7u8; 32])
}
fn config_pda(pid: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[CONFIG_SEED], pid).0
}
fn receipt_pda(pid: &Pubkey, payment_id: &[u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[RECEIPT_SEED, payment_id], pid).0
}

fn ix_pay_native(
    pid: &Pubkey,
    payer: &Pubkey,
    merchant: &Pubkey,
    treasury: &Pubkey,
    route: u8,
    payment_id: &[u8; 32],
    gross: u64,
    valid_until: i64,
) -> Instruction {
    let mut data = vec![10u8, route];
    data.extend_from_slice(payment_id);
    data.extend_from_slice(&gross.to_le_bytes());
    data.extend_from_slice(&valid_until.to_le_bytes());
    Instruction {
        program_id: *pid,
        accounts: vec![
            AccountMeta::new(*payer, true),
            AccountMeta::new(*merchant, false),
            AccountMeta::new(*treasury, false),
            AccountMeta::new_readonly(config_pda(pid), false),
            AccountMeta::new(receipt_pda(pid, payment_id), false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
        data,
    }
}

/// A config account exactly as Config::store writes it, so payment tests run
/// without initialize_config (which needs upgrade-authority plumbing).
fn config_account(pid: &Pubkey, admin: &Pubkey, treasury: &Pubkey, paused: bool) -> Account {
    let mut data = vec![0u8; CONFIG_LEN];
    data[0..8].copy_from_slice(CONFIG_MAGIC);
    data[8] = 1;
    data[9..41].copy_from_slice(admin.as_ref());
    data[41..73].copy_from_slice(treasury.as_ref());
    data[73..105].copy_from_slice(Pubkey::new_from_array([2u8; 32]).as_ref());
    data[105..137].copy_from_slice(Pubkey::new_from_array([3u8; 32]).as_ref());
    data[137] = u8::from(paused);
    Account {
        lamports: 10_000_000,
        data,
        owner: *pid,
        executable: false,
        rent_epoch: 0,
    }
}

fn custom(code: u32) -> TransactionError {
    TransactionError::InstructionError(0, InstructionError::Custom(code))
}

async fn send(
    banks: &mut BanksClient,
    payer: &Keypair,
    blockhash: Hash,
    ix: Instruction,
) -> Result<(), TransactionError> {
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[payer], blockhash);
    banks.process_transaction(tx).await.map_err(|e| e.unwrap())
}

/// Pre-fund a fresh recipient to rent-exemption so it can receive a small
/// payment delta (the runtime rejects leaving a system account below rent).
/// Real merchants/treasuries already exist and are rent-exempt; this models
/// that. Returns the balance after funding.
async fn prefund(banks: &mut BanksClient, payer: &Keypair, blockhash: Hash, who: &Pubkey) -> u64 {
    let ix = system_instruction::transfer(&payer.pubkey(), who, RENT_EXEMPT_ZERO);
    send(banks, payer, blockhash, ix).await.expect("prefund");
    banks.get_account(*who).await.unwrap().unwrap().lamports
}

struct Ctx {
    banks: BanksClient,
    payer: Keypair,
    blockhash: Hash,
    pid: Pubkey,
    treasury: Pubkey,
}

async fn payment_ctx(paused: bool) -> Ctx {
    // Point program-test at the compiled artifact and run it in the VM.
    std::env::set_var("SBF_OUT_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/target/deploy"));
    let pid = program_id();
    let treasury = Pubkey::new_unique();
    let admin = Pubkey::new_unique();
    let mut pt = ProgramTest::new("aifinpay_settlement_v1", pid, None);
    pt.add_account(config_pda(&pid), config_account(&pid, &admin, &treasury, paused));
    pt.add_account(
        treasury,
        Account { lamports: 1_000_000, data: vec![], owner: system_program::id(), executable: false, rent_epoch: 0 },
    );
    let (banks, payer, blockhash) = pt.start().await;
    Ctx { banks, payer, blockhash, pid, treasury }
}

// --- H-1 receipt side: the whole reason these tests exist -------------------

#[tokio::test]
async fn dusted_receipt_pda_does_not_block_payment() {
    let mut c = payment_ctx(false).await;
    let merchant = Pubkey::new_unique();
    let payment_id = [11u8; 32];
    let rcpt = receipt_pda(&c.pid, &payment_id);
    let m0 = prefund(&mut c.banks, &c.payer, c.blockhash, &merchant).await;
    let t0 = c.banks.get_account(c.treasury).await.unwrap().unwrap().lamports;

    // Attacker dusts the predicted receipt address (rent-exempt minimum — the
    // smallest transfer the runtime will accept to a fresh account).
    let dust = system_instruction::transfer(&c.payer.pubkey(), &rcpt, RENT_EXEMPT_ZERO);
    send(&mut c.banks, &c.payer, c.blockhash, dust).await.expect("dust transfer");

    // Payment must still settle.
    let ix = ix_pay_native(&c.pid, &c.payer.pubkey(), &merchant, &c.treasury, 1, &payment_id, 10_000, i64::MAX);
    send(&mut c.banks, &c.payer, c.blockhash, ix).await.expect("payment on dusted receipt PDA");

    let acct = c.banks.get_account(rcpt).await.unwrap().expect("receipt account");
    assert_eq!(acct.owner, c.pid);
    assert_eq!(acct.data.len(), RECEIPT_LEN);
    assert_eq!(&acct.data[0..8], b"AIFPRCP1");

    // exact 99/1 split, asserted as deltas
    let m = c.banks.get_account(merchant).await.unwrap().expect("merchant");
    assert_eq!(m.lamports - m0, 9_900);
    let t = c.banks.get_account(c.treasury).await.unwrap().unwrap();
    assert_eq!(t.lamports - t0, 100);
}

#[tokio::test]
async fn real_replay_is_still_rejected() {
    let mut c = payment_ctx(false).await;
    let merchant = Pubkey::new_unique();
    let payment_id = [12u8; 32];
    prefund(&mut c.banks, &c.payer, c.blockhash, &merchant).await;
    let ix = ix_pay_native(&c.pid, &c.payer.pubkey(), &merchant, &c.treasury, 1, &payment_id, 10_000, i64::MAX);
    send(&mut c.banks, &c.payer, c.blockhash, ix.clone()).await.expect("first payment");

    let m_before = c.banks.get_account(merchant).await.unwrap().unwrap().lamports;
    let new_hash = c.banks.get_latest_blockhash().await.unwrap();
    let res = send(&mut c.banks, &c.payer, new_hash, ix).await;
    assert_eq!(res.unwrap_err(), custom(ERR_REPLAY));
    let m_after = c.banks.get_account(merchant).await.unwrap().unwrap().lamports;
    assert_eq!(m_before, m_after, "replay must not move value");
}

// --- existing gates, asserted by exact error code ---------------------------

#[tokio::test]
async fn paused_config_rejects_payment() {
    let mut c = payment_ctx(true).await;
    let ix = ix_pay_native(&c.pid, &c.payer.pubkey(), &Pubkey::new_unique(), &c.treasury, 1, &[13u8; 32], 10_000, i64::MAX);
    let res = send(&mut c.banks, &c.payer, c.blockhash, ix).await;
    assert_eq!(res.unwrap_err(), custom(ERR_PAUSED));
}

#[tokio::test]
async fn expired_quote_rejects_payment() {
    let mut c = payment_ctx(false).await;
    let ix = ix_pay_native(&c.pid, &c.payer.pubkey(), &Pubkey::new_unique(), &c.treasury, 1, &[14u8; 32], 10_000, 1);
    let res = send(&mut c.banks, &c.payer, c.blockhash, ix).await;
    assert_eq!(res.unwrap_err(), custom(ERR_EXPIRED));
}

#[tokio::test]
async fn wrong_treasury_rejects_payment() {
    let mut c = payment_ctx(false).await;
    let attacker = Pubkey::new_unique();
    let ix = ix_pay_native(&c.pid, &c.payer.pubkey(), &Pubkey::new_unique(), &attacker, 1, &[15u8; 32], 10_000, i64::MAX);
    let res = send(&mut c.banks, &c.payer, c.blockhash, ix).await;
    assert_eq!(res.unwrap_err(), custom(ERR_INVALID_TREASURY));
}

#[tokio::test]
async fn aifp2_pays_merchant_in_full() {
    let mut c = payment_ctx(false).await;
    let merchant = Pubkey::new_unique();
    let m0 = prefund(&mut c.banks, &c.payer, c.blockhash, &merchant).await;
    let t_before = c.banks.get_account(c.treasury).await.unwrap().unwrap().lamports;
    let ix = ix_pay_native(&c.pid, &c.payer.pubkey(), &merchant, &c.treasury, 2, &[16u8; 32], 5_000, i64::MAX);
    send(&mut c.banks, &c.payer, c.blockhash, ix).await.expect("AIFP-2 payment");
    assert_eq!(c.banks.get_account(merchant).await.unwrap().unwrap().lamports - m0, 5_000);
    assert_eq!(c.banks.get_account(c.treasury).await.unwrap().unwrap().lamports, t_before);
}
