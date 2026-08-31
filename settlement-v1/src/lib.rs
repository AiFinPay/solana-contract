#![allow(clippy::too_many_arguments)]

use anchor_lang::solana_program::{
    account_info::{next_account_info, AccountInfo},
    bpf_loader_upgradeable::{self, UpgradeableLoaderState},
    clock::Clock,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program::{invoke, invoke_signed},
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    system_instruction,
    system_program,
    sysvar::{rent::Rent, Sysvar},
};
use anchor_spl::token::spl_token::{
    self,
    instruction as token_instruction,
    state::{Account as SplTokenAccount, Mint},
};

#[cfg(not(feature = "no-entrypoint"))]
entrypoint!(process_instruction);

const CONFIG_SEED: &[u8] = b"aifinpay-settlement-config-v1";
const RECEIPT_SEED: &[u8] = b"aifinpay-settlement-receipt-v1";
const CONFIG_MAGIC: &[u8; 8] = b"AIFPCFG1";
const RECEIPT_MAGIC: &[u8; 8] = b"AIFPRCP1";
const CONFIG_VERSION: u8 = 1;
const RECEIPT_VERSION: u8 = 1;
const CONFIG_LEN: usize = 8 + 1 + 32 + 32 + 32 + 32 + 1;
const RECEIPT_LEN: usize = 8 + 1 + 1 + 32 + 32 + 32 + 32 + 8 + 8 + 8 + 8;
const ROUTE_AIFP1: u8 = 1;
const ROUTE_AIFP2: u8 = 2;
const BPS_DENOM: u64 = 10_000;
const AIFP1_TREASURY_BPS: u64 = 100;

// Custom errors are stable wire values for SDK/E2E assertions.
const ERR_INVALID_INSTRUCTION: u32 = 7000;
const ERR_INVALID_ACCOUNTS: u32 = 7001;
const ERR_INVALID_PDA: u32 = 7002;
const ERR_UNAUTHORIZED: u32 = 7003;
const ERR_ALREADY_INITIALIZED: u32 = 7004;
const ERR_NOT_INITIALIZED: u32 = 7005;
const ERR_PAUSED: u32 = 7006;
const ERR_EXPIRED: u32 = 7007;
const ERR_INVALID_ROUTE: u32 = 7008;
const ERR_AMOUNT_TOO_SMALL: u32 = 7009;
const ERR_OVERFLOW: u32 = 7010;
const ERR_INVALID_TREASURY: u32 = 7011;
const ERR_INVALID_TOKEN: u32 = 7012;
const ERR_INVALID_TOKEN_ACCOUNT: u32 = 7013;
const ERR_REPLAY: u32 = 7014;
const ERR_UPGRADE_AUTHORITY_REQUIRED: u32 = 7015;
const ERR_INVALID_LOADER_STATE: u32 = 7016;

fn err(code: u32) -> ProgramError {
    ProgramError::Custom(code)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Config {
    admin: Pubkey,
    treasury: Pubkey,
    usdc_mint: Pubkey,
    usdt_mint: Pubkey,
    paused: bool,
}

impl Config {
    fn load(account: &AccountInfo) -> Result<Self, ProgramError> {
        let data = account.try_borrow_data()?;
        if data.len() != CONFIG_LEN || &data[0..8] != CONFIG_MAGIC || data[8] != CONFIG_VERSION {
            return Err(err(ERR_NOT_INITIALIZED));
        }
        Ok(Self {
            admin: pubkey_at(&data, 9)?,
            treasury: pubkey_at(&data, 41)?,
            usdc_mint: pubkey_at(&data, 73)?,
            usdt_mint: pubkey_at(&data, 105)?,
            paused: data[137] != 0,
        })
    }

    fn store(&self, account: &AccountInfo) -> ProgramResult {
        let mut data = account.try_borrow_mut_data()?;
        if data.len() != CONFIG_LEN {
            return Err(ProgramError::InvalidAccountData);
        }
        data.fill(0);
        data[0..8].copy_from_slice(CONFIG_MAGIC);
        data[8] = CONFIG_VERSION;
        data[9..41].copy_from_slice(self.admin.as_ref());
        data[41..73].copy_from_slice(self.treasury.as_ref());
        data[73..105].copy_from_slice(self.usdc_mint.as_ref());
        data[105..137].copy_from_slice(self.usdt_mint.as_ref());
        data[137] = u8::from(self.paused);
        Ok(())
    }
}

fn pubkey_at(data: &[u8], offset: usize) -> Result<Pubkey, ProgramError> {
    let bytes: [u8; 32] = data
        .get(offset..offset + 32)
        .ok_or(ProgramError::InvalidAccountData)?
        .try_into()
        .map_err(|_| ProgramError::InvalidAccountData)?;
    Ok(Pubkey::new_from_array(bytes))
}

fn read_pubkey(input: &[u8], offset: usize) -> Result<Pubkey, ProgramError> {
    pubkey_at(input, offset)
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64, ProgramError> {
    Ok(u64::from_le_bytes(
        input
            .get(offset..offset + 8)
            .ok_or_else(|| err(ERR_INVALID_INSTRUCTION))?
            .try_into()
            .map_err(|_| err(ERR_INVALID_INSTRUCTION))?,
    ))
}

fn read_i64(input: &[u8], offset: usize) -> Result<i64, ProgramError> {
    Ok(i64::from_le_bytes(
        input
            .get(offset..offset + 8)
            .ok_or_else(|| err(ERR_INVALID_INSTRUCTION))?
            .try_into()
            .map_err(|_| err(ERR_INVALID_INSTRUCTION))?,
    ))
}

fn read_payment_id(input: &[u8], offset: usize) -> Result<[u8; 32], ProgramError> {
    input
        .get(offset..offset + 32)
        .ok_or_else(|| err(ERR_INVALID_INSTRUCTION))?
        .try_into()
        .map_err(|_| err(ERR_INVALID_INSTRUCTION))
}

fn assert_signer(account: &AccountInfo) -> ProgramResult {
    if !account.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    Ok(())
}

fn config_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], program_id)
}

fn receipt_pda(program_id: &Pubkey, payment_id: &[u8; 32]) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[RECEIPT_SEED, payment_id], program_id)
}

fn assert_config_account(program_id: &Pubkey, account: &AccountInfo) -> Result<u8, ProgramError> {
    let (expected, bump) = config_pda(program_id);
    if expected != *account.key {
        return Err(err(ERR_INVALID_PDA));
    }
    Ok(bump)
}

fn assert_upgrade_authority(
    program_id: &Pubkey,
    authority: &AccountInfo,
    program_account: &AccountInfo,
    programdata_account: &AccountInfo,
) -> ProgramResult {
    assert_signer(authority)?;
    if program_account.key != program_id
        || !program_account.executable
        || *program_account.owner != bpf_loader_upgradeable::id()
        || *programdata_account.owner != bpf_loader_upgradeable::id()
    {
        return Err(err(ERR_INVALID_LOADER_STATE));
    }

    let program_state: UpgradeableLoaderState = bincode::deserialize(&program_account.try_borrow_data()?)
        .map_err(|_| err(ERR_INVALID_LOADER_STATE))?;
    let expected_programdata = match program_state {
        UpgradeableLoaderState::Program { programdata_address } => programdata_address,
        _ => return Err(err(ERR_INVALID_LOADER_STATE)),
    };
    if expected_programdata != *programdata_account.key {
        return Err(err(ERR_INVALID_LOADER_STATE));
    }

    let programdata_state: UpgradeableLoaderState = bincode::deserialize(&programdata_account.try_borrow_data()?)
        .map_err(|_| err(ERR_INVALID_LOADER_STATE))?;
    match programdata_state {
        UpgradeableLoaderState::ProgramData {
            upgrade_authority_address: Some(upgrade_authority),
            ..
        } if upgrade_authority == *authority.key => Ok(()),
        UpgradeableLoaderState::ProgramData {
            upgrade_authority_address: None,
            ..
        } => Err(err(ERR_UPGRADE_AUTHORITY_REQUIRED)),
        _ => Err(err(ERR_UNAUTHORIZED)),
    }
}

fn create_pda_account<'a>(
    payer: &AccountInfo<'a>,
    pda: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    program_id: &Pubkey,
    space: usize,
    signer_seeds: &[&[u8]],
) -> ProgramResult {
    if *system.key != system_program::id() {
        return Err(err(ERR_INVALID_ACCOUNTS));
    }
    // Initialization state is defined by DATA, never by lamports: only this
    // program can PDA-sign an allocate, so data at this address is ours alone.
    // Anyone can transfer lamports to the address first ("dusting") — that must
    // not be able to block creation, or a 1-lamport transfer bricks the config
    // PDA forever and griefs chosen receipt ids.
    if !pda.data_is_empty() || pda.owner == program_id {
        return Err(err(ERR_ALREADY_INITIALIZED));
    }
    let rent = Rent::get()?.minimum_balance(space);
    let existing = pda.lamports();
    if existing == 0 {
        invoke_signed(
            &system_instruction::create_account(
                payer.key,
                pda.key,
                rent,
                space as u64,
                program_id,
            ),
            &[payer.clone(), pda.clone(), system.clone()],
            &[signer_seeds],
        )
    } else {
        // Dusted: top up to rent-exemption if short, then allocate + assign
        // under the PDA's own signature (the account stays system-owned until
        // the assign, which is exactly what allocate/assign require).
        if existing < rent {
            invoke(
                &system_instruction::transfer(payer.key, pda.key, rent - existing),
                &[payer.clone(), pda.clone(), system.clone()],
            )?;
        }
        invoke_signed(
            &system_instruction::allocate(pda.key, space as u64),
            &[pda.clone(), system.clone()],
            &[signer_seeds],
        )?;
        invoke_signed(
            &system_instruction::assign(pda.key, program_id),
            &[pda.clone(), system.clone()],
            &[signer_seeds],
        )
    }
}

fn split_gross(route: u8, gross: u64) -> Result<(u64, u64), ProgramError> {
    if gross == 0 {
        return Err(err(ERR_AMOUNT_TOO_SMALL));
    }
    match route {
        ROUTE_AIFP1 => {
            let treasury = gross
                .checked_mul(AIFP1_TREASURY_BPS)
                .ok_or_else(|| err(ERR_OVERFLOW))?
                / BPS_DENOM;
            if treasury == 0 {
                return Err(err(ERR_AMOUNT_TOO_SMALL));
            }
            let merchant = gross.checked_sub(treasury).ok_or_else(|| err(ERR_OVERFLOW))?;
            Ok((merchant, treasury))
        }
        ROUTE_AIFP2 => Ok((gross, 0)),
        _ => Err(err(ERR_INVALID_ROUTE)),
    }
}

fn assert_live(config: &Config, valid_until: i64) -> ProgramResult {
    if config.paused {
        return Err(err(ERR_PAUSED));
    }
    if valid_until < Clock::get()?.unix_timestamp {
        return Err(err(ERR_EXPIRED));
    }
    Ok(())
}

fn create_receipt<'a>(
    program_id: &Pubkey,
    payer: &AccountInfo<'a>,
    receipt: &AccountInfo<'a>,
    system: &AccountInfo<'a>,
    payment_id: &[u8; 32],
    route: u8,
    merchant: &Pubkey,
    token: &Pubkey,
    gross: u64,
    merchant_amount: u64,
    treasury_amount: u64,
) -> ProgramResult {
    let (expected, bump) = receipt_pda(program_id, payment_id);
    if expected != *receipt.key {
        return Err(err(ERR_INVALID_PDA));
    }
    // Replay = a receipt was WRITTEN here (data), or the account already
    // belongs to this program. Lamports alone are not a receipt: anyone can
    // dust the address, and that must not block a legitimate payment.
    if !receipt.data_is_empty() || receipt.owner == program_id {
        return Err(err(ERR_REPLAY));
    }
    let bump_seed = [bump];
    let seeds: &[&[u8]] = &[RECEIPT_SEED, payment_id, &bump_seed];
    create_pda_account(payer, receipt, system, program_id, RECEIPT_LEN, seeds)?;

    let timestamp = Clock::get()?.unix_timestamp;
    let mut data = receipt.try_borrow_mut_data()?;
    data.fill(0);
    data[0..8].copy_from_slice(RECEIPT_MAGIC);
    data[8] = RECEIPT_VERSION;
    data[9] = route;
    data[10..42].copy_from_slice(payment_id);
    data[42..74].copy_from_slice(payer.key.as_ref());
    data[74..106].copy_from_slice(merchant.as_ref());
    data[106..138].copy_from_slice(token.as_ref());
    data[138..146].copy_from_slice(&gross.to_le_bytes());
    data[146..154].copy_from_slice(&merchant_amount.to_le_bytes());
    data[154..162].copy_from_slice(&treasury_amount.to_le_bytes());
    data[162..170].copy_from_slice(&timestamp.to_le_bytes());
    Ok(())
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let tag = *instruction_data.first().ok_or_else(|| err(ERR_INVALID_INSTRUCTION))?;
    match tag {
        0 => initialize_config(program_id, accounts, instruction_data),
        1 => set_paused(program_id, accounts, instruction_data),
        2 => set_admin(program_id, accounts, instruction_data),
        3 => set_treasury(program_id, accounts, instruction_data),
        10 => pay_native(program_id, accounts, instruction_data),
        11 => pay_token(program_id, accounts, instruction_data),
        _ => Err(err(ERR_INVALID_INSTRUCTION)),
    }
}

// tag=0 | treasury[32] | usdc_mint[32] | usdt_mint[32]
fn initialize_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    if input.len() != 97 {
        return Err(err(ERR_INVALID_INSTRUCTION));
    }
    let mut it = accounts.iter();
    let authority = next_account_info(&mut it)?;
    let config_account = next_account_info(&mut it)?;
    let program_account = next_account_info(&mut it)?;
    let programdata_account = next_account_info(&mut it)?;
    let system = next_account_info(&mut it)?;
    assert_upgrade_authority(program_id, authority, program_account, programdata_account)?;
    let bump = assert_config_account(program_id, config_account)?;

    let treasury = read_pubkey(input, 1)?;
    let usdc_mint = read_pubkey(input, 33)?;
    let usdt_mint = read_pubkey(input, 65)?;
    if treasury == Pubkey::default() || usdc_mint == Pubkey::default() || usdt_mint == Pubkey::default() {
        return Err(err(ERR_INVALID_ACCOUNTS));
    }
    if usdc_mint == usdt_mint {
        return Err(err(ERR_INVALID_TOKEN));
    }

    let bump_seed = [bump];
    let seeds: &[&[u8]] = &[CONFIG_SEED, &bump_seed];
    create_pda_account(authority, config_account, system, program_id, CONFIG_LEN, seeds)?;
    Config {
        admin: *authority.key,
        treasury,
        usdc_mint,
        usdt_mint,
        paused: true, // fail closed: explicit unpause after deploy + inspection.
    }
    .store(config_account)?;

    msg!("AIFP_SETTLEMENT_CONFIG_INITIALIZED admin={} treasury={} paused=true", authority.key, treasury);
    Ok(())
}

// tag=1 | paused[1]
fn set_paused(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    if input.len() != 2 || input[1] > 1 {
        return Err(err(ERR_INVALID_INSTRUCTION));
    }
    let mut it = accounts.iter();
    let admin = next_account_info(&mut it)?;
    let config_account = next_account_info(&mut it)?;
    assert_signer(admin)?;
    assert_config_account(program_id, config_account)?;
    let mut config = Config::load(config_account)?;
    if config.admin != *admin.key {
        return Err(err(ERR_UNAUTHORIZED));
    }
    config.paused = input[1] == 1;
    config.store(config_account)?;
    msg!("AIFP_SETTLEMENT_PAUSED value={}", config.paused);
    Ok(())
}

// tag=2 | new_admin[32]
fn set_admin(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    if input.len() != 33 {
        return Err(err(ERR_INVALID_INSTRUCTION));
    }
    let mut it = accounts.iter();
    let admin = next_account_info(&mut it)?;
    let config_account = next_account_info(&mut it)?;
    assert_signer(admin)?;
    assert_config_account(program_id, config_account)?;
    let mut config = Config::load(config_account)?;
    if config.admin != *admin.key {
        return Err(err(ERR_UNAUTHORIZED));
    }
    let next = read_pubkey(input, 1)?;
    if next == Pubkey::default() {
        return Err(err(ERR_INVALID_ACCOUNTS));
    }
    config.admin = next;
    config.store(config_account)?;
    msg!("AIFP_SETTLEMENT_ADMIN_CHANGED new_admin={}", next);
    Ok(())
}

// tag=3 | new_treasury[32]
fn set_treasury(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    if input.len() != 33 {
        return Err(err(ERR_INVALID_INSTRUCTION));
    }
    let mut it = accounts.iter();
    let admin = next_account_info(&mut it)?;
    let config_account = next_account_info(&mut it)?;
    assert_signer(admin)?;
    assert_config_account(program_id, config_account)?;
    let mut config = Config::load(config_account)?;
    if config.admin != *admin.key {
        return Err(err(ERR_UNAUTHORIZED));
    }
    let next = read_pubkey(input, 1)?;
    if next == Pubkey::default() {
        return Err(err(ERR_INVALID_TREASURY));
    }
    config.treasury = next;
    config.store(config_account)?;
    msg!("AIFP_SETTLEMENT_TREASURY_CHANGED new_treasury={}", next);
    Ok(())
}

// tag=10 | route[1] | payment_id[32] | gross_lamports[8] | valid_until[8]
// Accounts: payer(s,w), merchant(w), treasury(w), config, receipt(w), system_program
fn pay_native(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    if input.len() != 50 {
        return Err(err(ERR_INVALID_INSTRUCTION));
    }
    let route = input[1];
    let payment_id = read_payment_id(input, 2)?;
    let gross = read_u64(input, 34)?;
    let valid_until = read_i64(input, 42)?;
    let (merchant_amount, treasury_amount) = split_gross(route, gross)?;

    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let merchant = next_account_info(&mut it)?;
    let treasury = next_account_info(&mut it)?;
    let config_account = next_account_info(&mut it)?;
    let receipt = next_account_info(&mut it)?;
    let system = next_account_info(&mut it)?;

    assert_signer(payer)?;
    assert_config_account(program_id, config_account)?;
    let config = Config::load(config_account)?;
    assert_live(&config, valid_until)?;
    if *treasury.key != config.treasury {
        return Err(err(ERR_INVALID_TREASURY));
    }
    if !merchant.is_writable || !treasury.is_writable || !payer.is_writable || !receipt.is_writable {
        return Err(err(ERR_INVALID_ACCOUNTS));
    }

    create_receipt(
        program_id,
        payer,
        receipt,
        system,
        &payment_id,
        route,
        merchant.key,
        &Pubkey::default(),
        gross,
        merchant_amount,
        treasury_amount,
    )?;

    invoke(
        &system_instruction::transfer(payer.key, merchant.key, merchant_amount),
        &[payer.clone(), merchant.clone(), system.clone()],
    )?;
    if treasury_amount > 0 {
        invoke(
            &system_instruction::transfer(payer.key, treasury.key, treasury_amount),
            &[payer.clone(), treasury.clone(), system.clone()],
        )?;
    }

    msg!(
        "AIFP_PAYMENT route={} payment_id={} payer={} merchant={} token=SOL gross={} merchant_amount={} treasury_amount={} creator_amount=0 valid_until={}",
        route,
        bs58::encode(payment_id).into_string(),
        payer.key,
        merchant.key,
        gross,
        merchant_amount,
        treasury_amount,
        valid_until
    );
    Ok(())
}

// tag=11 | route[1] | payment_id[32] | gross_token_units[8] | valid_until[8]
// Accounts: payer(s,w), payer_token(w), merchant_token(w), treasury_token(w),
//           mint, config, receipt(w), token_program, system_program
fn pay_token(program_id: &Pubkey, accounts: &[AccountInfo], input: &[u8]) -> ProgramResult {
    if input.len() != 50 {
        return Err(err(ERR_INVALID_INSTRUCTION));
    }
    let route = input[1];
    let payment_id = read_payment_id(input, 2)?;
    let gross = read_u64(input, 34)?;
    let valid_until = read_i64(input, 42)?;
    let (merchant_amount, treasury_amount) = split_gross(route, gross)?;

    let mut it = accounts.iter();
    let payer = next_account_info(&mut it)?;
    let payer_token = next_account_info(&mut it)?;
    let merchant_token = next_account_info(&mut it)?;
    let treasury_token = next_account_info(&mut it)?;
    let mint_account = next_account_info(&mut it)?;
    let config_account = next_account_info(&mut it)?;
    let receipt = next_account_info(&mut it)?;
    let token_program = next_account_info(&mut it)?;
    let system = next_account_info(&mut it)?;

    assert_signer(payer)?;
    assert_config_account(program_id, config_account)?;
    let config = Config::load(config_account)?;
    assert_live(&config, valid_until)?;
    if *token_program.key != spl_token::id() || *mint_account.owner != spl_token::id() {
        return Err(err(ERR_INVALID_TOKEN));
    }
    if *mint_account.key != config.usdc_mint && *mint_account.key != config.usdt_mint {
        return Err(err(ERR_INVALID_TOKEN));
    }

    let payer_state = SplTokenAccount::unpack(&payer_token.try_borrow_data()?)
        .map_err(|_| err(ERR_INVALID_TOKEN_ACCOUNT))?;
    let merchant_state = SplTokenAccount::unpack(&merchant_token.try_borrow_data()?)
        .map_err(|_| err(ERR_INVALID_TOKEN_ACCOUNT))?;
    let treasury_state = SplTokenAccount::unpack(&treasury_token.try_borrow_data()?)
        .map_err(|_| err(ERR_INVALID_TOKEN_ACCOUNT))?;
    let mint_state = Mint::unpack(&mint_account.try_borrow_data()?)
        .map_err(|_| err(ERR_INVALID_TOKEN))?;

    if payer_state.owner != *payer.key
        || payer_state.mint != *mint_account.key
        || merchant_state.mint != *mint_account.key
        || treasury_state.mint != *mint_account.key
        || treasury_state.owner != config.treasury
    {
        return Err(err(ERR_INVALID_TOKEN_ACCOUNT));
    }
    if !payer_token.is_writable || !merchant_token.is_writable || !treasury_token.is_writable || !receipt.is_writable {
        return Err(err(ERR_INVALID_ACCOUNTS));
    }

    create_receipt(
        program_id,
        payer,
        receipt,
        system,
        &payment_id,
        route,
        &merchant_state.owner,
        mint_account.key,
        gross,
        merchant_amount,
        treasury_amount,
    )?;

    let transfer_merchant = token_instruction::transfer_checked(
        token_program.key,
        payer_token.key,
        mint_account.key,
        merchant_token.key,
        payer.key,
        &[],
        merchant_amount,
        mint_state.decimals,
    )?;
    invoke(
        &transfer_merchant,
        &[
            payer_token.clone(),
            mint_account.clone(),
            merchant_token.clone(),
            payer.clone(),
            token_program.clone(),
        ],
    )?;

    if treasury_amount > 0 {
        let transfer_treasury = token_instruction::transfer_checked(
            token_program.key,
            payer_token.key,
            mint_account.key,
            treasury_token.key,
            payer.key,
            &[],
            treasury_amount,
            mint_state.decimals,
        )?;
        invoke(
            &transfer_treasury,
            &[
                payer_token.clone(),
                mint_account.clone(),
                treasury_token.clone(),
                payer.clone(),
                token_program.clone(),
            ],
        )?;
    }

    msg!(
        "AIFP_PAYMENT route={} payment_id={} payer={} merchant_owner={} token={} gross={} merchant_amount={} treasury_amount={} creator_amount=0 valid_until={}",
        route,
        bs58::encode(payment_id).into_string(),
        payer.key,
        merchant_state.owner,
        mint_account.key,
        gross,
        merchant_amount,
        treasury_amount,
        valid_until
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aifp1_is_gross_inclusive_99_1_0() {
        let (merchant, treasury) = split_gross(ROUTE_AIFP1, 10_000).unwrap();
        assert_eq!(merchant, 9_900);
        assert_eq!(treasury, 100);
    }

    #[test]
    fn aifp2_is_zero_percent() {
        let (merchant, treasury) = split_gross(ROUTE_AIFP2, 1).unwrap();
        assert_eq!(merchant, 1);
        assert_eq!(treasury, 0);
    }

    #[test]
    fn aifp1_rejects_round_to_zero_fee() {
        assert_eq!(split_gross(ROUTE_AIFP1, 99), Err(err(ERR_AMOUNT_TOO_SMALL)));
    }

    #[test]
    fn route_is_not_dynamic_basis_points() {
        assert_eq!(split_gross(0, 10_000), Err(err(ERR_INVALID_ROUTE)));
        assert_eq!(split_gross(3, 10_000), Err(err(ERR_INVALID_ROUTE)));
    }

    #[test]
    fn receipt_size_matches_layout() {
        assert_eq!(RECEIPT_LEN, 170);
        assert_eq!(CONFIG_LEN, 138);
    }
}
