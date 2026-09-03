# Solana Contract

Primary instructions: node_modules/@daochild/agents-config/AGENTS.md — read in
full and follow unless overridden below.

When this repository is opened with **opencode**, also read
[`.opencode/AGENTS.md`](./.opencode/AGENTS.md) for opencode-specific workflow,
permission, and security instructions. In case of conflict, the opencode file
overrides this file.

Anchor/Sealevel Solana program (v1.4) for signed, multi-route gross settlement.

## Dev Commands

```bash
# Rust toolchain is pinned to 1.89.0 (rust-toolchain.toml)
cargo test                              # Run all tests (defined in Anchor.toml scripts)
cargo fmt --check                      # Check formatting
cargo clippy --all-targets -- -D warnings  # Lint
cargo build-sbf                        # SBF build gate (required before deploy)
```

## CI

All CI steps run from the repo root (workspace-level):
1. `cargo fmt --check`
2. `cargo test --locked`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo build-sbf`

## Project Structure

```
programs/splitter/     # Full-featured canonical Anchor program (cdylib + lib)
  AGENTS.md           # Program-level agent instructions
  README.md           # Program-level quick reference
  src/
    lib.rs            # declare_id, #[program] module, inline unit tests
    instructions/    # Handler implementations (initialize, settle_*, pause, route config, etc.)
    state.rs          # Accounts structs (RouteProfile, WhitelistedTokens, etc.)
    constants.rs      # Route IDs (ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1), seed constants
    error.rs          # Error codes
    utils.rs          # quote_hash, split_gross, ecrecover helpers
  tests/              # litesvm integration tests
scripts/              # Deployment / utility scripts
```

## Architecture Notes

- Program ID: `G6neYBZe8AzMvNPcewBhmzLao4CtZqc2uPVzPQBbAdrT`
- Uses `anchor-lang` 1.1.2 with `init-if-needed` feature
- Uses `anchor-spl` with token feature for SPL token operations
- Tests use `litesvm` (Solana program unit test framework), NOT Anchor's JS test harness
- Route IDs (ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1) are hardcoded constants that must match EVM v1.4
- Signer role uses secp256k1 ecrecover (solana-secp256k1-recover, solana-keccak-hasher)
- Only the canonical `splitter` program lives under `programs/`.
- See [`programs/splitter/AGENTS.md`](./programs/splitter/AGENTS.md) for
  program-specific rules.

## Key Constraints

- Uses `resolver = "2"` in workspace Cargo.toml
- `skip_local_validator = true` in Anchor.toml (local testing uses litesvm)
- Wallet for localnet: `~/.config/solana/id.json`
