# Contributing

Thank you for your interest in the AiFinPay Solana Splitter. This document
covers the day-to-day workflow, code conventions, and review expectations
for changes that touch on-chain settlement code.

## 1. Prerequisites

- Rust **1.89.0** — pinned via `rust-toolchain.toml`. The repo will refuse
  to build on any other channel.
- `cargo-build-sbf` — installed with the Solana CLI toolchain
  (`sh -c "$(curl -sSfL https://release.anza.xyz/v2.0.0/install)"`).
- `anchor-cli` ≥ 0.31 — used only as a build orchestrator; tests do **not**
  require Anchor's JS harness.
- A local Solana wallet at `~/.config/solana/id.json` (used by Anchor's
  provider config).

## 2. First-Time Setup

```bash
git clone <repo-url>
cd solana-contract
pnpm install                       # pulls @daochild/agents-config only
cargo build-sbf                    # builds target/deploy/splitter.so
cargo test                         # inline unit tests + litesvm integration test
```

## 3. Development Workflow

1. **Create a branch** from `main` (or `dev`) named `feat/<slug>`,
   `fix/<slug>`, `audit/<slug>`, or `chore/<slug>`.
2. **Make your change** in `programs/splitter/` or `programs/splitter_light/`.
3. **Run the local checks** before pushing (see §5).
4. **Push** and open a pull request against the original branch.
5. CI must pass (`fmt --check`, `cargo test --locked`, `clippy`, `build-sbf`).
6. **At least one human approval** is required for any change touching
   settlement semantics, fee caps, role rotation, or the EIP-712 digest.
7. Squash-merge once green.

## 4. Code Conventions

- **Edition**: 2021. `resolver = "2"` in workspace `Cargo.toml`.
- **Lints**: `cargo clippy --all-targets -- -D warnings` MUST pass. CI fails
  on any warning.
- **Formatting**: `cargo fmt` (no nightly tweaks). The CI runs
  `cargo fmt --check`.
- **No comments inside code unless explicitly required.** Use doc comments
  (`///`) for public items and module-level intent. Internal logic should
  be self-describing.
- **Error variants**: add a new `ErrorCode` variant with a clear `#[msg]`
  whenever you introduce a new revert condition. Never reuse a generic
  variant like `Unauthorized` for distinct conditions.
- **Constants** live in `constants.rs`; **no magic numbers** inside
  handlers.
- **New instructions** are placed in their own file under
  `programs/splitter/src/instructions/<name>.rs`, wired up in
  `instructions.rs`, and dispatched from `lib.rs`. Each instruction file
  contains:
  - the `#[derive(Accounts)]` struct,
  - parameter structs (with `AnchorSerialize/AnchorDeserialize`),
  - a `handle_<verb>` function returning `Result<()>` (or
    `Result<ReturnType>` for view instructions).
- **New state types** are declared in `state.rs` with `#[account]` and
  `#[derive(InitSpace)]`. Estimate account size for rent before adding new
  fields.

## 5. Local Validation

Before opening a PR, run all four CI steps locally:

```bash
cargo fmt --check
cargo test --locked
cargo clippy --all-targets -- -D warnings
cargo build-sbf
```

Any failure is a blocker. If `cargo build-sbf` fails with linker errors,
verify `cargo-build-sbf` is on `PATH` and that the Solana platform-tools
match the Solana BPF toolchain expected by `anchor-lang 1.1.2`.

## 6. Testing

- **Unit tests** live in `lib.rs` (inline `#[cfg(test)] mod tests`) and
  cover pure helpers: `quote_hash`, `split_gross`, route constant
  invariants. Add new tests here for any new pure function.
- **Integration tests** live in `programs/splitter/tests/` and use
  `litesvm`. They load `target/deploy/splitter.so` directly, so make sure
  `cargo build-sbf` has been run before invoking `cargo test`.
- **`splitter_light`** currently has inline unit tests only. Add a
  `programs/splitter_light/tests/` directory when the first litesvm test is
  created.
- **Coverage target**: 80% on new code. If you cannot hit that, document
  the gap in the PR description.
- **Digest regression vectors**: when changing the digest, add a regression
  test pinning the resulting 32-byte hash. The vector MUST be cross-checked
  against the EVM v1.4 deployment for the equivalent `Quote`; otherwise the
  bridge is broken. Note that the EVM and Solana digests are intentionally
  different algorithms (keccak256 EIP-712 vs. SHA-256 Borsh), so vectors are
  chain-specific.

## 7. Cross-Chain Parity Is Sacred

The following values are part of the cross-chain contract and changing them
is a coordinated upgrade:

- `EIP712_NAME`, `EIP712_VERSION`
- `DOMAIN_TYPEHASH`, `QUOTE_TYPEHASH`
- `ROUTE_AGENT_X402`, `ROUTE_MERCHANT_AIFP1`
- Fee caps (`MAX_TREASURY_BPS`, `MAX_IP_CREATOR_BPS`)
- The order of fields in `quote_hash()`

Any PR touching one of these MUST:

1. Reference the EVM v1.4 contract repo and the matching PR / commit.
2. Add a vector test that cross-checks both deployments.
3. Be tagged `cross-chain-affecting` and require two human approvals.

## 8. Pull Request Checklist

- [ ] Branch is up to date with the target.
- [ ] `cargo fmt --check` passes locally.
- [ ] `cargo test --locked` passes locally.
- [ ] `cargo clippy --all-targets -- -D warnings` passes locally.
- [ ] `cargo build-sbf` passes locally.
- [ ] New or modified instructions have at least one positive and one
  negative test case.
- [ ] New `ErrorCode` variants have a clear `#[msg]`.
- [ ] `ARCHITECTURE.md` updated if layout / roles / digest changed.
- [ ] `docs/IMPLEMENTATION.md` updated with status.
- [ ] `programs/splitter_light/README.md` / `AGENTS.md` updated if the light
  variant is touched.
- [ ] `docs/adr/` updated if a new architectural decision was made.
- [ ] No secrets, keypairs, or test wallets committed.

## 9. Commit Messages

Use the conventional style already present in the repo:

```
<scope>: <imperative summary>

<optional body explaining motivation and trade-offs>
```

Common scopes: `splitter`, `settle-native`, `settle-stable`, `rbac`,
`tests`, `ci`, `docs`, `adr`.

## 10. Issue Triage

- **P0** — funds at risk, paused-state bug, signer bypass. Page the on-call
  immediately.
- **P1** — fee miscalculation, missing error code, off-by-one.
- **P2** — UX, gas, documentation.
- **P3** — refactors, cleanup.

When in doubt, escalate to the EVM counterpart team before changing the
shared digest or route identifiers.