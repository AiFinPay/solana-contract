use {
    anchor_lang::{
        prelude::Pubkey,
        solana_program::{instruction::Instruction, system_program},
        AccountDeserialize, InstructionData, ToAccountMetas,
    },
    litesvm::LiteSVM,
    solana_keypair::Keypair,
    solana_message::{Message, VersionedMessage},
    solana_signer::Signer,
    solana_transaction::versioned::VersionedTransaction,
};

fn build_program_so_path() -> String {
    // Anchor copies the final .so to target/deploy/splitter.so.
    let base = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    format!("{}/../../target/deploy/splitter.so", base)
}

#[test]
fn test_initialize_and_quote_total() {
    let program_id = splitter::id();
    let payer = Keypair::new();
    let admin = Keypair::new();
    let signer = [0xabu8; 64];
    let pauser = Keypair::new();
    let treasury = Keypair::new();

    let config = Pubkey::find_program_address(&[splitter::constants::CONFIG_SEED], &program_id).0;
    let token_list =
        Pubkey::find_program_address(&[splitter::constants::TOKEN_LIST_SEED], &program_id).0;
    let profiles = Pubkey::find_program_address(
        &[splitter::constants::PROFILES_INDEX_SEED],
        &program_id,
    )
    .0;

    let mut svm = LiteSVM::new();
    let so_path = build_program_so_path();
    let bytes = std::fs::read(&so_path).unwrap_or_else(|_| {
        panic!("Could not read splitter.so at {}. Run `anchor build` first.", so_path)
    });
    svm.add_program(program_id, &bytes).unwrap();
    svm.airdrop(&payer.pubkey(), 1_000_000_000).unwrap();

    let params = splitter::instructions::initialize::InitializeParams {
        admin: admin.pubkey(),
        signer,
        pauser: pauser.pubkey(),
        treasury: treasury.pubkey(),
        stablecoins: vec![],
        route_ids: vec![
            splitter::constants::ROUTE_AGENT_X402,
            splitter::constants::ROUTE_MERCHANT_AIFP1,
        ],
        treasury_bps: vec![0, 100],
        ip_creator_bps: vec![0, 0],
    };

    let instruction = Instruction::new_with_bytes(
        program_id,
        &splitter::instruction::Initialize { params }.data(),
        splitter::accounts::Initialize {
            payer: payer.pubkey(),
            config,
            token_list,
            profiles,
            system_program: system_program::ID,
        }
        .to_account_metas(None),
    );

    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(&[instruction], Some(&payer.pubkey()), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), &[&payer]).unwrap();
    svm.send_transaction(tx).unwrap();

    let config_account = svm.get_account(&config).unwrap();
    let mut data: &[u8] = &config_account.data;
    let config_state = splitter::state::Config::try_deserialize(&mut data).unwrap();
    assert_eq!(config_state.admin, admin.pubkey());
    assert_eq!(config_state.signer, signer);
    assert_eq!(config_state.treasury, treasury.pubkey());
    assert!(!config_state.is_paused);

    let profiles_account = svm.get_account(&profiles).unwrap();
    data = &profiles_account.data;
    let profiles_state = splitter::state::ProfilesIndex::try_deserialize(&mut data).unwrap();
    assert_eq!(profiles_state.entries.len(), 2);
    assert!(profiles_state.entries[0].enabled);
    assert!(profiles_state.entries[1].enabled);
    assert_eq!(profiles_state.entries[1].treasury_bps, 100);
}
