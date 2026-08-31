# Solana Contract

Primary instructions: node_modules/@daochild/agents-config/AGENTS.md — read in full and follow unless overridden below.

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

All CI steps run in the `splitter` subdirectory (not repo root):
1. `cargo fmt --check`
2. `cargo test --locked`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo build-sbf`

## Project Structure

```
programs/splitter/     # Single Anchor program (cdylib + lib)
  src/
    lib.rs            # declare_id, #[program] module, inline unit tests
    instructions/    # Handler implementations (initialize, settle_*, pause, route config, etc.)
    state.rs          # Accounts structs (RouteProfile, WhitelistedTokens, etc.)
    constants.rs      # Route IDs (ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1), seed constants
    error.rs          # Error codes
    utils.rs          # quote_hash, split_gross, ecrecover helpers
scripts/              # Empty (no scripts yet)
```

## Architecture Notes

- Program ID: `G6neYBZe8AzMvNPcewBhmzLao4CtZqc2uPVzPQBbAdrT`
- Uses `anchor-lang` 1.1.2 with `init-if-needed` feature
- Uses `anchor-spl` with token feature for SPL token operations
- Tests use `litesvm` (Solana program unit test framework), NOT Anchor's JS test harness
- Route IDs (ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1) are hardcoded constants that must match EVM v1.4
- Signer role uses secp256k1 ecrecover (solana-secp256k1-recover, solana-keccak-hasher)

## Key Constraints

- Uses `resolver = "2"` in workspace Cargo.toml
- `skip_local_validator = true` in Anchor.toml (local testing uses litesvm)
- Wallet for localnet: `~/.config/solana/id.json`
