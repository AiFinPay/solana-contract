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
cargo build-sbf
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

## Deploy with Ledger

Before using a Ledger for deployment:

1. Close Ledger Live.
2. Connect the Ledger via USB, unlock it, and open the **Solana** app
   (device must show "Application is ready").
3. Make sure the deployer account on the Ledger is funded with SOL on the
   target cluster.

Check the wallet address that will pay for deployment:

```bash
# default derivation path (key=0)
solana-keygen pubkey usb://ledger?key=0
```

If you have several Ledger devices, use a fully-qualified keypair URL:

```bash
# resolve the signer URL first
solana resolve-signer usb://ledger?key=0/0
# example output (wallet IDs vary):
# usb://ledger/BsNsvfXqQTtJnagwFWdBS7FBXgnsK8VZ5CmuznN85swK?key=0/0
```

Build the program binary:

```bash
cargo build-sbf --package splitter
```

### Deploy the program binary

Ledger is only used to **sign the deployment transactions**; the program
keypair must still be a normal Solana keypair file (Ledger cannot expose
the private key needed by `solana program deploy`). Generate the program
keypair locally:

```bash
solana-keygen new --no-passphrase -s -o target/deploy/splitter-keypair.json
```

Then deploy, passing the Ledger URL as the deployer signer:

```bash
solana program deploy target/deploy/splitter.so \
  --program-id target/deploy/splitter-keypair.json \
  --keypair usb://ledger?key=0 \
  --url https://api.devnet.solana.com
```

Approve each transaction on the Ledger when prompted. The command prints
program ID and deployment signature.

### Initialize with the deployer keypair on Ledger

The deployer (`DEPLOYER` constant) must match the Ledger address used for
`initialize`. If the program ID is the canonical one, use the TS helper
script with the Ledger keypair URL exported as `DEPLOYER_KEYPAIR`:

```bash
export DEPLOYER_KEYPAIR="usb://ledger?key=0"
export CLUSTER="devnet"
# required env vars for initialize-devnet-splitter.ts
export ADMIN_PUBKEY="<admin solana address>"
export PAUSER_PUBKEY="<pauser solana address>"
export TREASURY_PUBKEY="<treasury solana address>"
export SIGNER_ETH_PUBKEY="<secp256k1 uncompressed 64-byte hex (no 0x)>"
export STABLECOINS="[<comma-separated mint pubkeys>]"
export ROUTE_IDS='["<route_id_hex_1>","<route_id_hex_2>"]'
export TREASURY_BPS='[<bps list>]'
export IP_CREATOR_BPS='[<bps list>]'

pnpm exec ts-node scripts/initialize-devnet-splitter.ts
```

Approve the transaction on the Ledger when prompted.

### Useful Ledger commands

```bash
# show balance of the Ledger deployer account
solana balance <DEPLOYER_ADDRESS> --url https://api.devnet.solana.com

# airdrop on devnet (if the faucet is available)
solana airdrop 2 <DEPLOYER_ADDRESS> --url https://api.devnet.solana.com

# close a deployed program (recovers rent), signed by Ledger
solana program close <PROGRAM_ID> \
  --keypair usb://ledger?key=0 \
  --url https://api.devnet.solana.com \
  --bypass-warning
```

> **Note for zsh users:** the `?` in `usb://ledger?key=0` is interpreted by
> zsh. Either escape it (`usb://ledger\?key=0`) or disable zsh globbing:
> `unsetopt nomatch`.

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