# AiFinPay Solana Settlement v1 — production release candidate

This is a **new settlement-only Solana program**. It does not replace the legacy
`contract/` monolith and it does not implement Agent Passport identity. AIFP-3
identity is global/off-chain with signed wallet bindings; this program only moves
value and records replay-proof settlement receipts.

## Canonical economics

| route byte | route | payer gross | merchant | AiFinPay treasury | creator |
|---|---|---:|---:|---:|---:|
| `1` | AIFP-1 merchant AI-traffic monetisation | 100% | 99% | 1% | 0% |
| `2` | AIFP-2 / x402 agent payment | 100% | 100% | 0% | 0% |

Fees are **deducted from gross**. There is no fee-on-top path and no caller-
selected basis-points argument.

## Assets

The program supports:

- native SOL;
- exactly two SPL Token mints stored in the program config as `usdc_mint` and
  `usdt_mint`.

For mainnet initialization use issuer-confirmed mints only. At the 2026-08-16
release review these are:

- USDC: `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v`
- USD₮: `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB`

Do not substitute a bridged or lookalike mint without a new reviewed config
change.

## Security model

- `initialize_config` verifies that the initializer is the **upgrade authority
  of the running program** by reading the BPF Upgradeable Loader Program and
  ProgramData accounts. This prevents first-caller config takeover.
- Initialization creates the config PDA and starts `paused=true`.
- Admin can pause/unpause, rotate admin, and rotate treasury.
- AIFP-1 rejects a gross amount whose 1% treasury leg rounds to zero.
- Every payment creates a PDA from `("aifinpay-settlement-receipt-v1",
  payment_id)`. A second use of the same payment id fails.
- `valid_until` is checked against the Solana Clock before any transfer.
- Token settlement accepts only the two configured mints; payer/merchant/
  treasury token-account ownership and mint are checked before transfer.
- Treasury token account must be owned by the configured treasury authority.
- All state creation + transfers happen atomically in one Solana transaction.

## Build

```bash
cargo test --manifest-path settlement-v1/Cargo.toml
cargo build-sbf --manifest-path settlement-v1/Cargo.toml
```

The program intentionally contains **no compile-time `declare_id!`**. It uses
the runtime `program_id` supplied by Solana, so the deployer can generate a new
program keypair without committing a secret key or editing source.

## Mainnet deployment sequence

1. Freeze the reviewed commit SHA.
2. Run the test/build commands above from that SHA.
3. Generate a dedicated program keypair on the deployment machine:
   `solana-keygen new --outfile aifinpay-settlement-v1-program.json`.
4. Deploy the exact `.so` using that keypair and keep upgrade authority with the
   designated deployment authority until initialization and verification are
   complete.
5. Derive the config PDA from seed `aifinpay-settlement-config-v1` and the new
   program id.
6. Call instruction tag `0` with the approved treasury, USDC mint, and USD₮
   mint. The initializer must sign and must equal the program upgrade authority.
7. Read the config account back. It **must be paused**.
8. Save program id, deployment transaction/signature, program-data hash, config
   PDA, upgrade authority, treasury and mints in the canonical deployment
   evidence.
9. Run one minimal AIFP-2 native test, one AIFP-1 native test, one USDC test and
   one USD₮ test while still in a controlled release window.
10. Verify receipt PDA creation, gross split and replay rejection for every
    test. Only then unpause for production routing.

Do not mark Solana payments live from a successful deploy alone.

## Instruction wire format

All integers are little-endian.

### `0` — initialize config

Data: `tag:u8 | treasury:Pubkey | usdc_mint:Pubkey | usdt_mint:Pubkey`

Accounts in order:

1. upgrade authority — signer, writable (pays config rent)
2. config PDA — writable
3. program account — executable
4. program-data account — BPF Upgradeable Loader ProgramData
5. system program

### `1` — pause/unpause

Data: `tag:u8 | paused:u8` (`0` or `1`)

Accounts: admin signer, config PDA writable.

### `2` — rotate admin

Data: `tag:u8 | new_admin:Pubkey`

Accounts: current admin signer, config PDA writable.

### `3` — rotate treasury

Data: `tag:u8 | new_treasury:Pubkey`

Accounts: admin signer, config PDA writable.

### `10` — native SOL settlement

Data: `tag:u8 | route:u8 | payment_id:[u8;32] | gross_lamports:u64 |
valid_until:i64`

Accounts:

1. payer signer + writable
2. merchant writable
3. configured treasury writable
4. config PDA
5. receipt PDA writable
6. system program

### `11` — SPL USDC/USD₮ settlement

Data: `tag:u8 | route:u8 | payment_id:[u8;32] | gross_token_units:u64 |
valid_until:i64`

Accounts:

1. payer signer
2. payer token account writable
3. merchant token account writable
4. treasury token account writable
5. configured mint
6. config PDA
7. receipt PDA writable
8. SPL Token program
9. system program

## Release status

**Source-level production release candidate only.** No production activation is
claimed until the exact reviewed artifact is deployed, initialized, verified,
paid E2E-tested and then unpaused.
