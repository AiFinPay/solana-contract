# `splitter` — Program-level agent instructions

This file is the local, program-specific companion to the repo-level
[`AGENTS.md`](../../AGENTS.md). It captures the rules and context that
apply **only** when editing or auditing `programs/splitter/`.

> Scope: this program is the only supported AiFinPay Solana splitter v1.4.

## Dev commands (program scope)

```bash
# From repo root — all commands target the whole workspace.
cargo test --package splitter
cargo fmt --check --package splitter
cargo clippy --package splitter --all-targets -- -D warnings
cargo build-sbf --package splitter
```

## Program identity

- Name: `splitter`
- Version: `1.4.0`
- Program ID: `G6neYBZe8AzMvNPcewBhmzLao4CtZqc2uPVzPQBbAdrT`
- Crate type: `cdylib + lib`

## Source layout

```
programs/splitter/
├── Cargo.toml
├── AGENTS.md            # this file
├── README.md            # Program-level quick reference
├── src/
│   ├── lib.rs           # declare_id, #[program] dispatch, inline unit tests
│   ├── instructions.rs  # re-exports every instruction module
│   ├── instructions/    # one file per Anchor instruction
│   │   ├── initialize.rs
│   │   ├── settle_native.rs
│   │   ├── settle_stable.rs
│   │   ├── quote_total.rs
│   │   ├── pause.rs / unpause.rs
│   │   ├── set_treasury.rs
│   │   ├── configure_route.rs
│   │   ├── enable_route.rs / disable_route.rs
│   │   ├── set_whitelisted_tokens.rs
│   │   └── grant_*_role / rotate_*_role
│   ├── state.rs         # #[account] structs + Quote
│   ├── constants.rs     # route IDs, PDA seeds, fee caps
│   ├── error.rs         # ErrorCode
│   └── utils.rs         # digest, recover_signer, split_gross, events
└── tests/
    └── test_initialize.rs   # litesvm integration test
```

## Architecture notes specific to `splitter`

- **Full RBAC**: `admin`, `pauser`, `signer` (secp256k1), `treasury`.
- **On-chain registry**: `TokenList` + `ProfilesIndex` of
  `Vec<RouteProfileEntry>`.
- **Replay protection**: `PayerNonce` + `ConsumedNonce` PDAs with
  `init_if_needed`.
- **Solana-native digest**: `quote_message_hash` uses SHA-256 over a
  domain-tagged, program-bound Borsh payload; signer recovery uses
  `solana-secp256k1-recover`.
- **Native SOL**: direct lamport mutation after duplicate-account checks.
- **SPL stablecoins**: `anchor_spl::token::transfer` via `remaining_accounts`.

## Cross-chain sacred constants

These MUST match the EVM v1.4 deployment. Any change is a coordinated
upgrade:

- `ROUTE_AGENT_X402`, `ROUTE_MERCHANT_AIFP1`
- `MAX_TREASURY_BPS = 500`, `MAX_IP_CREATOR_BPS = 100`
- The field order in `Quote` Borsh serialization used by
  `quote_message_hash()`

## Key constraints for agents

1. **DEPLOYER gating**: `initialize` is gated to the hardcoded `DEPLOYER`
   constant in production; the placeholder `Pubkey::default()` is only for
   local tests. Do not remove this gate without a documented replacement.
2. **Role separation checks are weak by design**: comparing a secp256k1 X
   coordinate to a Solana `Pubkey` is semantically meaningless. Treat
   admin/pauser/signer separation as an **operational** invariant, not a
   code invariant.
3. **Instruction wiring**: every new instruction needs three edits:
   - `src/instructions/<name>.rs`
   - `src/instructions.rs`
   - `src/lib.rs` dispatch

## Testing rules

- Add pure-function unit tests to `src/lib.rs`.
- Add instruction-level / litesvm tests to `tests/`.
- `cargo build-sbf --package splitter` must produce
  `target/deploy/splitter.so` before `cargo test` loads it.

## Audit skill

For non-trivial code changes, run the `senior-software-architect` skill
(and `senior-solidity-auditor` when cross-chain constants change). If the
required skill is missing, create it first per the repo-level AGENTS.md
Audit Rule.
