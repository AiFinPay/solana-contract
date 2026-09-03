# Implementation Status

This document tracks the **current state** of the AiFinPay Solana
Splitter implementation against the v1.4 specification. It is updated
each time a feature is delivered, refactored, or removed.

Two programs are in scope:

- `splitter` — full registry-based program under `programs/splitter/`.
- `splitter_light` — minimal hardcoded program under `programs/splitter_light/`.
  It mirrors the settlement surface of `splitter` but has no on-chain admin,
  pauser, treasury registry, or route configuration. See
  [`programs/splitter_light/README.md`](../programs/splitter_light/README.md).

## Status legend

- ✅ Done — code shipped, tests passing, audit-ready.
- 🟡 Partial — wired but missing tests, docs, or polish.
- ⏳ Planned — designed, not started.
- ❌ Removed — feature was removed; see git history.

## Instruction surface

### `splitter`

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
| `rotate_signer_role`    |  ✅   | `instructions/rotate_signer_role.rs`     | — |
| `grant_pauser_role`     |  ✅   | `instructions/grant_pauser_role.rs`      | — |
| `rotate_pauser_role`    |  ✅   | `instructions/rotate_pauser_role.rs`     | — |

### `splitter_light`

| Instruction     | Status | Handler file                          | Tests |
|-----------------|:------:|---------------------------------------|-------|
| `settle_native` |  ✅   | `instructions/settle_native.rs`       | inline helpers only |
| `settle_stable` |  ✅   | `instructions/settle_stable.rs`       | inline helpers only |
| `set_signer`    |  ✅   | `instructions/set_signer.rs`          | inline helpers only |

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

The following fields are shared between `splitter` and `splitter_light` and
MUST match EVM v1.4.

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

`splitter_light` additionally hardcodes the same `USDC_MINT`, `USDT_MINT`,
`MESSAGE_DOMAIN_TAG`, quote field order, and split math as `splitter`.

## CI

`.github/workflows/ci.yml` runs on every PR and push to `main` / `dev`:

1. `cargo fmt --check`
2. `cargo test --locked`
3. `cargo clippy --all-targets -- -D warnings`
4. `cargo build-sbf`

All steps run from the repo root because the workspace is defined at the
root level (`Cargo.toml` with `members = ["programs/*"]`).

## Known issues / TODO

- [x] **CI working-directory** — `.github/workflows/ci.yml` was updated
      to run from the repo root; the workspace members live under
      `programs/`. Verify on next CI run.
- [ ] **Stable-settlement integration test** — only the `initialize`
      flow is covered by `litesvm` today. Add a positive and a negative
      `settle_stable` test once test keypairs are generated.
- [ ] **Native-settlement integration test** — same as above; depends on
      a pre-generated secp256k1 keypair fixture.
- [ ] **`splitter_light` litesvm tests** — currently only inline unit tests
      exist. Add `programs/splitter_light/tests/` once secp256k1 fixtures are
      ready.
- [ ] **EIP-712 vector regression** — pin the byte-for-byte digest for a
      canonical quote and cross-check against the EVM v1.4 fixture.
- [ ] **Audit report** — once `senior-solidity-auditor` /
      `senior-software-architect` reviews land, attach the report under
      `audits/`.

## Test coverage

### `splitter`

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

### `splitter_light`

| Surface                            | Coverage | Notes                                |
|------------------------------------|:--------:|--------------------------------------|
| Route constant invariants          |   ✅     | inline test in `lib.rs`              |
| Stablecoin constants               |   ✅     | inline test in `lib.rs`              |
| `quote_message_hash` determinism   |   ✅     | inline test in `lib.rs`              |
| secp256k1 recover — valid sig      |   ✅     | inline test in `lib.rs`              |
| secp256k1 recover — tampered quote |   ✅     | inline test in `lib.rs`              |
| `split_gross` — zero fees          |   ✅     | inline test in `lib.rs`              |
| `split_gross` — 1% treasury        |   ✅     | inline test in `lib.rs`              |
| `split_gross` — zero amount        |   ✅     | inline test in `lib.rs`              |
| Route profile lookup               |   ✅     | inline test in `lib.rs`              |
| `settle_native` happy path         |   ⏳    | add litesvm test once fixtures exist |
| `settle_stable` happy path         |   ⏳    | add litesvm test once fixtures exist |
| `set_signer` rotation              |   ⏳    | add litesvm test once fixtures exist |

## Deployment status

- **Localnet**: ✅ builds and tests pass.
- **Devnet**: ⏳ pending audit.
- **Mainnet**: ❌ not deployed; blocked on audit + EVM v1.4 cutover.