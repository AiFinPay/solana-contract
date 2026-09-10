# AiFinPay Solana Splitter

Signed, multi-route gross settlement program on Solana. v1.4 — the
Solana counterpart of the AiFinPay EVM splitter.

A payer submits a signed quote; the program splits the gross amount into
a merchant leg, an optional protocol-treasury fee, and an optional
IP-creator royalty, crediting each leg atomically in a single
transaction.

## Program ID

```
56cRuWVNt5KXRgvA4m6wroB4D45A3SjvowZVZXYBw3Mr
```

## Build

```bash
anchor build
anchor keys sync
```

Produces `target/deploy/splitter.so`.

## Test

```bash
cargo test
```

Runs inline unit tests (`lib.rs`) and the `litesvm` integration test
(`programs/splitter/tests/test_initialize.rs`).

## Lint

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The CI pipeline in `.github/workflows/ci.yml` runs all four checks
(formatting, tests, clippy, SBF build) on every PR and push to `main` /
`dev`.

## Deployment

Deploy scripts live under `scripts/` organized by cluster:

```
scripts/
├── devnet/
│   ├── devnet-deploy.sh          # Deploy to devnet (file keypair)
│   ├── calculate-deploy-cost.sh  # Estimate deploy cost
│   ├── calculate-initialize-cost.sh  # Estimate initialize cost
│   ├── simulate-devnet-deploy.sh     # Dry-run deploy
│   ├── initialize-devnet-splitter.ts # Initialize after deploy
│   ├── configure-route.ts            # Configure settlement routes
│   └── check-devnet-splitter.ts      # Verify deployment state
├── mainnet/
│   ├── mainnet-deploy.sh         # Deploy to mainnet (Ledger)
│   ├── calculate-deploy-cost.sh  # Estimate deploy cost
│   ├── calculate-initialize-cost.sh  # Estimate initialize cost
│   ├── simulate-mainnet-deploy.sh    # Dry-run deploy
│   ├── initialize-mainnet-splitter.ts
│   ├── configure-mainnet-route.ts
│   └── check-mainnet-splitter.ts
└── localnet/
    └── localnet-deploy.sh        # Deploy to local validator
```

### Estimate costs

Before deploying, check the estimated SOL required:

```bash
# Deploy cost (program rent + transaction fees)
./scripts/devnet/calculate-deploy-cost.sh

# Initialize cost (PDA rent for Config, TokenList, ProfilesIndex)
./scripts/devnet/calculate-initialize-cost.sh
```

Replace `devnet` with `mainnet` for mainnet estimates.

### Deploy to devnet

```bash
# 1. Fund the deployer
solana airdrop 2 --keypair keypairs/devnet-deployer.json --url devnet

# 2. Deploy
./scripts/devnet/devnet-deploy.sh

# 3. Initialize (set required env vars first)
export ADMIN_PUBKEY="<admin solana address>"
export PAUSER_PUBKEY="<pauser solana address>"
export TREASURY_PUBKEY="<treasury solana address>"
export SIGNER_ETH_PUBKEY="<secp256k1 uncompressed 64-byte hex (no 0x)>"
export STABLECOINS="[<comma-separated mint pubkeys>]"
export ROUTE_IDS='["<route_id_hex_1>","<route_id_hex_2>"]'
export TREASURY_BPS='[<bps list>]'
export IP_CREATOR_BPS='[<bps list>]'

pnpm exec ts-node scripts/devnet/initialize-devnet-splitter.ts

# 4. Verify
pnpm exec ts-node scripts/devnet/check-devnet-splitter.ts
```

### Deploy to localnet

```bash
# Start local validator
solana-test-validator --reset

# Deploy (auto-airdrops if balance is low)
./scripts/localnet/localnet-deploy.sh
```

### Deploy to mainnet (Ledger)

Before using a Ledger for deployment:

1. Close Ledger Live.
2. Connect the Ledger via USB, unlock it, and open the **Solana** app
   (device must show "Application is ready").
3. Fund the Ledger wallet with at least ~3 SOL.

```bash
# Deploy (requires DEPLOYER env or --deployer flag)
SPLITTER_DEPLOYER="<your-ledger-pubkey>" ./scripts/mainnet/mainnet-deploy.sh

# Or with explicit flags
./scripts/mainnet/mainnet-deploy.sh \
  --deployer "<your-ledger-pubkey>" \
  --keypair "usb://ledger?key=0"
```

The mainnet script requires typing `DEPLOY-MAINNET` to confirm.

### Useful commands

```bash
# Check balance
solana balance <WALLET_ADDRESS> --url https://api.devnet.solana.com

# Airdrop on devnet
solana airdrop 2 --keypair keypairs/devnet-deployer.json --url devnet

# Close a deployed program (recovers rent)
solana program close <PROGRAM_ID> --keypair <KEYPAIR> --url devnet
```

## Overview

The single canonical Anchor **v1.1.2** program is:

- `programs/splitter/` — full-featured registry-based splitter.

Two settlement routes at v1.4:

- `ROUTE_AGENT_X402` — agent-to-agent, default fees 0 / 0 bps.
- `ROUTE_MERCHANT_AIFP1` — merchant, default fee 100 / 0 bps.

Fee caps: treasury ≤ 500 bps (5 %), IP creator ≤ 100 bps (1 %).
Signer verification via secp256k1 ecrecover (`solana-secpx256k1-recover`,
`solana-keccak-hasher`).
Tests use `litesvm`, not Anchor's JS harness.

## Documentation

- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — module layout, on-chain
  state, settlement flows, RBAC.
- [`CONTRIBUTING.md`](./CONTRIBUTING.md) — workflow, code conventions,
  cross-chain parity rules.
- [`SECURITY.md`](./SECURITY.md) — threat model, security-critical
  invariants, incident response.
- [`docs/BUSINESS_LOGIC.md`](./docs/BUSINESS_LOGIC.md) — business rules
  in operations-friendly language.
- [`docs/IMPLEMENTATION.md`](./docs/IMPLEMENTATION.md) — feature status,
  coverage, known issues.
- [`docs/adr/`](./docs/adr/) — Architecture Decision Records.
- [`AGENTS.md`](./AGENTS.md) — agent / opencode instructions.
- [`programs/splitter/README.md`](./programs/splitter/README.md) — full splitter program reference.

## Repository Layout

```
.
├── programs/splitter/          # Full-featured Anchor program (cdylib + lib)
│   ├── src/
│   │   ├── lib.rs              # declare_id, #[program] entrypoints, inline tests
│   │   ├── constants.rs        # EIP-712 fields, route IDs, seeds, caps
│   │   ├── state.rs            # Config, TokenList, profiles, nonces, Quote
│   │   ├── error.rs            # ErrorCode variants
│   │   ├── utils.rs            # digest, recover_signer, split_gross, events
│   │   └── instructions/       # one file per instruction handler
│   ├── tests/test_initialize.rs    # litesvm integration test
│   ├── Cargo.toml
│   ├── AGENTS.md               # Program-level agent instructions
│   └── README.md               # Program-level quick reference
├── docs/                       # Business logic, implementation status, ADRs
├── .github/workflows/ci.yml    # fmt + test + clippy + build-sbf
├── Anchor.toml                 # `skip_local_validator = true`
├── Cargo.toml                  # workspace root
└── rust-toolchain.toml         # pins Rust 1.89.0
```

## Cross-Chain Parity

This program is intentionally byte-compatible with the EVM v1.4
deployment. The following fields are part of the cross-chain contract
and changing them is a coordinated upgrade:

- EIP-712 `name`, `version`, `DOMAIN_TYPEHASH`, `QUOTE_TYPEHASH`.
- Route IDs `ROUTE_AGENT_X402`, `ROUTE_MERCHANT_AIFP1`.
- Fee caps `MAX_TREASURY_BPS = 500`, `MAX_IP_CREATOR_BPS = 100`.
- The order of fields inside `quote_hash()`.

See [`CONTRIBUTING.md` §7](./CONTRIBUTING.md#7-cross-chain-parity-is-sacred).

## License

TBD.