# AiFinPay Solana Splitter

Signed, multi-route gross settlement program on Solana. v1.4 — the
Solana counterpart of the AiFinPay EVM splitter.

A payer submits a signed quote; the program splits the gross amount into
a merchant leg, an optional protocol-treasury fee, and an optional
IP-creator royalty, crediting each leg atomically in a single
transaction.

## Program ID

```
G6neYBZe8AzMvNPcewBhmzLao4CtZqc2uPVzPQBbAdrT
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

## Overview

- Two Anchor **v1.1.2** programs:
  - `programs/splitter/` — full-featured registry-based splitter.
  - `programs/splitter_light/` — minimal hardcoded SOL + USDC/USDT variant.
- Two settlement routes at v1.4:
  - `ROUTE_AGENT_X402` — agent-to-agent, default fees 0 / 0 bps.
  - `ROUTE_MERCHANT_AIFP1` — merchant, default fee 100 / 0 bps.
- Fee caps: treasury ≤ 500 bps (5 %), IP creator ≤ 100 bps (1 %).
- Signer verification via secp256k1 ecrecover (`solana-secpx256k1-recover`,
  `solana-keccak-hasher`).
- Tests use `litesvm`, not Anchor's JS harness.

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
- [`programs/splitter_light/README.md`](./programs/splitter_light/README.md) — light program reference.

## Repository Layout

```
.
├── programs/splitter/         # Full-featured Anchor program (cdylib + lib)
│   ├── src/
│   │   ├── lib.rs             # declare_id, #[program] entrypoints, inline tests
│   │   ├── constants.rs       # EIP-712 fields, route IDs, seeds, caps
│   │   ├── state.rs           # Config, TokenList, profiles, nonces, Quote
│   │   ├── error.rs           # ErrorCode variants
│   │   ├── utils.rs           # digest, recover_signer, split_gross, events
│   │   └── instructions/      # one file per instruction handler
│   ├── tests/test_initialize.rs   # litesvm integration test
│   ├── Cargo.toml
│   ├── AGENTS.md              # Program-level agent instructions
│   └── README.md              # Program-level quick reference
├── programs/splitter_light/   # Minimal hardcoded variant
│   ├── src/                   # Same module layout
│   ├── Cargo.toml
│   ├── AGENTS.md              # Program-level agent instructions
│   └── README.md              # Program-level quick reference
├── docs/                      # Business logic, implementation status, ADRs
├── .github/workflows/ci.yml   # fmt + test + clippy + build-sbf
├── Anchor.toml                # `skip_local_validator = true`
├── Cargo.toml                 # workspace root
└── rust-toolchain.toml        # pins Rust 1.89.0
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