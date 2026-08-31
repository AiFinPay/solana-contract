# Implementation Status

This document tracks the **current state** of the AiFinPay Solana
Splitter implementation against the v1.4 specification. It is updated
each time a feature is delivered, refactored, or removed.

## Status legend

- ✅ Done — code shipped, tests passing, audit-ready.
- 🟡 Partial — wired but missing tests, docs, or polish.
- ⏳ Planned — designed, not started.
- ❌ Removed — feature was removed; see git history.

## Instruction surface

| Instruction             | Status | Handler file                            | Tests |
|-------------------------|:-----:|------------------------------------------|-------|
| `initialize`            |  ✅   | `instructions/initialize.rs`             | litesvm happy path |
| `settle_native`         |  ✅   | `instructions/settle_native.rs`          | inline helpers only |
| `settle_stable`         |  ✅   | `instructions/settle_stable.rs`          | inline helpers only |
| `quote_total`           |  ✅   | `instructions/quote_total.rs`            | inline helpers only |
| `pause` / `unpause`     |  ✅   | `instructions/pause.rs`, `unpause.rs`    | — |
| `set_treasury`          |  ✅   | `instructions/set_treasury.rs`           | — |
| `configure_route`       |  ✅   | `instructions/configure_route.rs`        | — |
| `enable_route`          |  ✅   | `instructions/enable_route.rs`           | — |
| `disable_route`         |  ✅   | `instructions/disable_route.rs`          | — |
| `set_whitelisted_tokens`|  ✅   | `instructions/set_whitelisted_tokens.rs` | — |
| `grant_signer_role`     |  ✅   | `instructions/grant_signer_role.rs`      | — |
| `revoke_signer_role`    |  ✅   | `instructions/revoke_signer_role.rs`     | — |
| `grant_pauser_role`     |  ✅   | `instructions/grant_pauser_role.rs`      | — |
| `revoke_pauser_role`    |  ✅   | `instructions/revoke_pauser_role.rs`     | — |
| `increment`             |  🟡   | `instructions/increment.rs`              | — (legacy, see note) |

### Note on `increment`

`instructions/increment.rs` is leftover scaffolding from the original
Anchor template (`create-anchor-cli`). It references a `Counter` state
type that does not exist in the v1.4 program. It is **not** wired into
`lib.rs` and cannot be invoked. Scheduled for removal in the next
cleanup pass.

## State accounts

| Account                | Status | Owner         |
|------------------------|:------:|----------------|
| `Config`               |  ✅    | admin          |
| `TokenList`            |  ✅    | admin          |
| `RouteProfileEntry`    |  ✅    | admin          |
| `ProfilesIndex`        |  ✅    | admin          |
| `PayerNonce`           |  ✅    | implicit (PDA) |
| `ConsumedNonce`        |  ✅    | implicit (PDA) |
| `Quote` (instruction payload) | ✅ | n/a |
| `Payment` event        |  ✅    | emitted on success |
| `TreasuryUpdated` event|  ✅    | emitted on `set_treasury` |

## Cross-chain parity

| Field                  | Status     | Notes                                  |
|------------------------|:----------:|----------------------------------------|
| `EIP712_NAME`          | ✅ match   | `AiFinPayB2BSplitter`                  |
| `EIP712_VERSION`       | ✅ match   | `"1"`                                  |
| `DOMAIN_TYPEHASH`      | ✅ match   | EVM v1.4 deployment                    |
| `QUOTE_TYPEHASH`       | ✅ match   | EVM v1.4 deployment                    |
| `ROUTE_AGENT_X402`     | ✅ match   | EVM v1.4 deployment                    |
| `ROUTE_MERCHANT_AIFP1` | ✅ match   | EVM v1.4 deployment                    |
| `MAX_TREASURY_BPS`     | ✅ match   | 500                                    |
| `MAX_IP_CREATOR_BPS`   | ✅ match   | 100                                    |

## CI

`.github/workflows/ci.yml` runs on every PR and push to `main` / `dev`:

1. `cargo fmt --check` (working dir: `splitter`)
2. `cargo test --locked` (working dir: `splitter`)
3. `cargo clippy --all-targets -- -D warnings` (working dir: `splitter`)
4. `cargo build-sbf` (working dir: `splitter`)

> Note: the workflow currently has `working-directory: splitter` set, but
> this repo does not have a top-level `splitter/` directory — the actual
> Cargo workspace is at the repo root. This is a known issue tracked
> below; local CI commands work because they target the root workspace.

## Known issues / TODO

- [ ] **CI working-directory** — `.github/workflows/ci.yml` references a
      `splitter/` subdirectory that does not exist. Fix by either (a)
      restructuring into a Cargo workspace subdir matching the workflow,
      or (b) updating the workflow to drop the working-directory
      override. Tracked.
- [ ] **`instructions/increment.rs`** — leftover template code, see
      note above.
- [ ] **Stable-settlement integration test** — only the `initialize`
      flow is covered by `litesvm` today. Add a positive and a negative
      `settle_stable` test once test keypairs are generated.
- [ ] **Native-settlement integration test** — same as above; depends on
      a pre-generated secp256k1 keypair fixture.
- [ ] **EIP-712 vector regression** — pin the byte-for-byte digest for a
      canonical quote and cross-check against the EVM v1.4 fixture.
- [ ] **Audit report** — once `senior-solidity-auditor` /
      `senior-software-architect` reviews land, attach the report under
      `audits/`.

## Test coverage

| Surface                            | Coverage | Notes                                |
|------------------------------------|:--------:|--------------------------------------|
| `quote_hash` determinism           |   ✅     | inline test in `lib.rs`              |
| Route constant invariants          |   ✅     | inline test in `lib.rs`              |
| `split_gross` — zero fees          |   ✅     | inline test                          |
| `split_gross` — 1% treasury        |   ✅     | inline test                          |
| `split_gross` — zero amount        |   ✅     | inline test                          |
| `split_gross` — missing IP creator |   ✅     | inline test                          |
| `initialize` (litesvm)            |   ✅     | `tests/test_initialize.rs`           |
| `settle_native` happy path         |   ⏳    |                                      |
| `settle_native` invalid signature  |   ⏳    |                                      |
| `settle_stable` happy path         |   ⏳    |                                      |
| `settle_stable` unsupported mint   |   ⏳    |                                      |
| `quote_total` view                 |   ⏳    |                                      |
| Pause / unpause                    |   ⏳    |                                      |
| Role rotation invariants           |   ⏳    |                                      |

## Deployment status

- **Localnet**: ✅ builds and tests pass.
- **Devnet**: ⏳ pending audit.
- **Mainnet**: ❌ not deployed; blocked on audit + EVM v1.4 cutover.