# ADR-0001: Splitter Architecture for v1.4

- **Status**: Accepted
- **Date**: 2024-Q4
- **Deciders**: AiFinPay core team
- **Supersedes**: —
- **Related**: EVM splitter v1.4 (Solana counterpart)

## Context

AiFinPay operates a cross-chain B2B payment protocol. The v1.4 release
introduces a Solana deployment that MUST mirror the on-chain semantics
of the existing EVM contract:

- Same quote schema (EIP-712-style typed data).
- Same route identifiers (keccak256 of route names).
- Same fee caps and split math.
- Same trust model (one secp256k1 signer off-chain, on-chain RBAC for
  admin/pauser).

The Solana program must run as a single Anchor v1.1.2 program (cdylib
+ lib) and integrate with SPL for stablecoin settlement. Native SOL
settlement is required as well — SPL token-2022 is not appropriate for
the "agent pays merchant in SOL" use case.

## Decision

We adopt the following architecture:

### 1. Single Anchor program, single program ID

One Anchor program named `splitter` with program ID
`G6neYBZe8AzMvNPcewBhmzLao4CtZqc2uPVzPQBbAdrT`. All settlement logic
lives in `programs/splitter/`.

### 2. EIP-712-style digest on Solana

Even though EIP-712 is an EVM standard, we deliberately re-use the
*digest construction* on Solana because:

- It lets the off-chain `signer` be a single secp256k1 key signing for
  both chains.
- It keeps the cross-chain payment semantics identical.
- `solana-secp256k1-recover` is a stable syscall; we don't roll our own
  crypto.

The on-chain digest uses `solana-keccak-hasher` to match the EVM
keccak256 byte-for-byte.

### 3. PDAs over program-owned accounts

All state accounts are PDAs derived from the program ID:

- `Config` — single global.
- `TokenList` — single global.
- `ProfilesIndex` — single global, holds `Vec<RouteProfileEntry>`.
- `PayerNonce` — one per payer, `init_if_needed`.
- `ConsumedNonce` — one per `(payer, nonce)`, `init_if_needed`.

This gives us deterministic addresses, free rent-on-init for replay
protection, and no off-chain registry to maintain.

### 4. Direct lamport mutation for SOL settlement

`settle_native` mutates lamports directly via `try_borrow_mut_lamports`
rather than CPI to `system_program::transfer`. Rationale:

- Avoids the system-program check that prevents a transfer that would
  zero out the source account — relevant when the payer pays exactly
  its full balance.
- Keeps the operation inside a single instruction for atomicity.
- We do not invoke any other program from `settle_native`, so the
  reentrancy surface is empty.

### 5. SPL CPI for stable settlement

`settle_stable` uses `anchor_spl::token::transfer` with the payer as
the authority. The token list constraint (`token_list.is_allowed(mint)`)
ensures the mint is whitelisted before any CPI is issued.

### 6. RBAC: three distinct keys

`admin`, `pauser`, and the first 32 bytes of `signer` must be pairwise
distinct. This is enforced at `initialize` and at every rotation
instruction. Rationale: a single compromised key must not be able to
pause, rotate, and settle simultaneously.

### 7. Native and stable as separate instructions

`settle_native` and `settle_stable` are separate entrypoints (rather
than a single instruction with a discriminator). Rationale:

- Different validation paths (token-list vs. `Pubkey::default()`).
- Different remaining-accounts layouts.
- Cleaner error reporting: an invalid token for native produces
  `InvalidTokenForNative`, not a generic `UnsupportedToken`.

### 8. `litesvm` for unit tests, not Anchor's JS harness

The CI runs `cargo test` which compiles to native and exercises the
inline `#[cfg(test)]` unit tests in `lib.rs` plus the `litesvm`
integration test in `tests/test_initialize.rs`. This avoids the JS
toolchain and makes CI deterministic. `Anchor.toml` sets
`skip_local_validator = true` to signal that Anchor's local validator
is not part of the workflow.

## Consequences

### Positive

- A single off-chain `signer` works for both EVM and Solana.
- All settlement invariants live in one place; cross-chain auditors can
  read both codebases side-by-side.
- Fee math, route IDs, and digest are byte-identical across chains.
- No off-chain registry is needed; addresses are PDAs.

### Negative

- Anchor's "no entrypoint" / "no-idl" features have to be opted into
  per build target.
- The `increment.rs` template scaffolding is left in the tree and
  requires a cleanup pass.
- `litesvm` is younger than Anchor's JS harness; we accept the risk in
  exchange for a lighter CI.

### Neutral

- The CI workflow file currently points at a `splitter/` subdirectory
  that does not exist. We will either restructure the repo or fix the
  workflow in a follow-up ADR.

## Alternatives considered

1. **One instruction with a `token` discriminator** — rejected: blurs
   the error surface and forces both branches to pay for `remaining
   accounts`.
2. **CPI to a system program wrapper instead of direct lamport
   mutation** — rejected: makes it harder to fully drain the payer
   account and adds a CPI we don't need.
3. **Use `spl-token` directly (no Anchor SPL wrapper)** — rejected:
   `anchor_spl::token` is the idiomatic path and gives us CPI helpers
   for free.
4. **Token-2022 with transfer hooks** — rejected: would require every
   stablecoin issuer to opt in; we don't control the mints.

## Follow-ups

- [ ] ADR-0002: clarify whether future versions keep the v1.4 digest
      unchanged or migrate to a chain-bound domain separator.
- [ ] Remove `instructions/increment.rs` (template leftover).
- [ ] Pin a cross-chain digest vector for regression testing.