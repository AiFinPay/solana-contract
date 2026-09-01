# `splitter_light` — Program-level agent instructions

This file is the local, program-specific companion to the repo-level
[`AGENTS.md`](../../AGENTS.md). It captures the rules and context that
apply **only** when editing or auditing `programs/splitter_light/`.

> Scope: this program is the minimal hardcoded variant of the AiFinPay
> Solana splitter. For the full registry-based variant, see
> [`programs/splitter/AGENTS.md`](../splitter/AGENTS.md).

## Dev commands (program scope)

```bash
# From repo root — all commands target the whole workspace.
cargo test --package splitter_light
cargo fmt --check --package splitter_light
cargo clippy --package splitter_light --all-targets -- -D warnings
cargo build-sbf --package splitter_light
```

## Program identity

- Name: `splitter_light`
- Version: `0.1.0`
- Program ID: `7vGTUXSmooih99MuzQELyaeFeZmuo4QcstS7T9Jv7yyR`
- Crate type: `cdylib + lib`

## Source layout

```
programs/splitter_light/
├── Cargo.toml
├── AGENTS.md          # this file
├── README.md          # Program-level quick reference
└── src/
    ├── lib.rs         # declare_id, #[program] dispatch, inline unit tests
    ├── instructions.rs
    ├── instructions/
    │   ├── settle_native.rs
    │   ├── settle_stable.rs
    │   └── set_signer.rs
    ├── state.rs       # Config, PayerNonce, ConsumedNonce, Quote
    ├── constants.rs   # EIP-712 fields, route IDs, USDC/USDT mints, treasury, initial signer
    ├── error.rs       # ErrorCode
    └── utils.rs       # digest, recover_signer, split_gross, set_signer_digest, events
```

## Architecture notes specific to `splitter_light`

- **No on-chain admin / pauser / treasury registry.** All configuration is
  hardcoded in `constants.rs`:
  - `PROTOCOL_TREASURY` — single treasury wallet.
  - `USDC_MINT`, `USDT_MINT` — only whitelisted stablecoins.
  - `RouteProfile::AGENT` and `RouteProfile::MERCHANT` — fixed fee schedules.
- **Signer rotation via ECDSA only.** `set_signer` accepts a signature from
  the *current* signer over a digest of `(current_signer, new_signer)`. There is no
  Solana admin fallback.
- **Lazy Config PDA.** `Config` is created on the first settlement or
  `set_signer` call via `init_if_needed`.
- **Replay protection:** `PayerNonce` + `ConsumedNonce` PDAs with
  `init_if_needed`, identical to `splitter`.
- **Native SOL:** direct lamport mutation.
- **SPL stablecoins:** `anchor_spl::token::transfer` through
  `remaining_accounts` (`[payer_ata, merchant_ata, treasury_ata,
  (ip_creator_ata)?]`).

## Cross-chain sacred constants

These MUST match the EVM v1.4 deployment. Any change is a coordinated
upgrade:

- `EIP712_NAME`, `EIP712_VERSION`
- `DOMAIN_TYPEHASH`, `QUOTE_TYPEHASH`
- `ROUTE_AGENT_X402`, `ROUTE_MERCHANT_AIFP1`
- `MAX_TREASURY_BPS = 500`, `MAX_IP_CREATOR_BPS = 100`
- The field order in `quote_hash()`

## Deployment-critical placeholders

Two constants in `constants.rs` are intentionally placeholder values and
**must** be replaced before mainnet deployment:

1. `PROTOCOL_TREASURY` — currently set to the system program address. This
   must be a real protocol-owned wallet.
2. `INITIAL_SIGNER` — currently a non-curve placeholder (`0x01…||0x02…`).
   This must be a valid secp256k1 uncompressed public key.

Do not deploy to mainnet without replacing both and re-running tests + an
audit.

## Key constraints for agents

1. **Keep the trust model minimal.** Adding an on-chain admin role is a
   large design change; document it in an ADR and update both the
   repo-level `ARCHITECTURE.md` and `SECURITY.md`.
2. **Signer rotation is ECDSA-gated.** Any change to `set_signer` must
   preserve: (a) binding to the current signer, (b) high-s rejection,
   (c) no Solana signer authority granting rotation.
3. **Instruction wiring:** every new instruction needs three edits:
   - `src/instructions/<name>.rs`
   - `src/instructions.rs`
   - `src/lib.rs` dispatch

## Testing rules

- Add pure-function unit tests to `src/lib.rs`.
- Add instruction-level / litesvm tests to a new `tests/` directory when
  the first integration test is added.
- `cargo build-sbf --package splitter_light` must produce
  `target/deploy/splitter_light.so` before tests load it.

## Audit skill

For non-trivial code changes, run the `senior-software-architect` skill
(and `senior-solidity-auditor` when cross-chain constants change). If the
required skill is missing, create it first per the repo-level AGENTS.md
Audit Rule.
