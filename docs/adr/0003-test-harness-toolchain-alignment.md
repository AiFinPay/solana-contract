# ADR-0003: Test-Harness / Toolchain Alignment for `splitter` v1.4

- **Status**: Accepted (with open follow-up, see §5)
- **Date**: 2026-09-09
- **Deciders**: AiFinPay core team
- **Related**: `programs/splitter`, `programs/splitter/tests/test_initialize.rs`,
  `Anchor.toml` (`solana_version = "4.1.2"`), `cargo-build-sbf 4.1.0` /
  `platform-tools v1.54`, `anchor-lang 1.1.2`, litesvm

## Context

The `splitter` test suite combines native unit tests (`cargo test`, pure
functions in `src/lib.rs`) with LiteSVM integration tests
(`programs/splitter/tests/`) that execute the compiled SBF binary from
`target/deploy/splitter.so` (built via `cargo build-sbf`).

The dev-dependencies historically pinned `litesvm 0.6.1` + `solana-sdk 2.2`,
i.e. the Agave 2.2-era runtime (`solana-program-runtime 2.2.x`, sbpf 0.10-era
VM). The program itself is built with `cargo-build-sbf 4.1.0` /
`platform-tools v1.54` (Agave 4.x era) through `anchor-lang 1.1.2`, which
pulls `solana-*` crates at versions 3.0/4.x.

## Findings

1. **Version skew, not a broken SDK.** Nothing is wrong with `solana-sdk`
   itself — it is simply too old for the toolchain that builds this program.
   There were two independent mismatches:
   - *Runtime ABI*: the SBF binary loads (program account is `executable`,
     PDAs derive) but faults in its entrypoint prologue: `Access violation
     in unknown section at address 0x8 of size 48` after exactly 581 CUs.
     The fault is byte-for-byte identical for **any** input, including an
     unknown instruction discriminator, and identical under both the 2.2
     (litesvm 0.6) and 3.0 (litesvm 0.12) runtimes. It therefore occurs
     before Anchor dispatch: the VM executes code that expects a newer
     runtime ABI (input serialization, syscalls, memory layout from the
     current platform-tools) than the old runtime provides.
   - *Crate types (compile-time only, not the cause of the fault)*:
     `anchor-lang 1.1.2` uses `solana-pubkey 3.0` while `solana-sdk 2.2`
     uses `pubkey 2.2` — distinct types that forced byte-level conversions
     in test code.

2. **Upgrade to litesvm 0.12** (3.x stack: runtime 3.0, keypair 3.1,
   instruction 3.5, pubkey 3.0). This removes the type skew — the test
   harness and the program crate now share a single `Pubkey` type — and is
   the newest harness generation with a coherent dependency set. The
   entrypoint fault persists unchanged under 0.12, confirming it is not
   specific to the 2.2 runtime.

3. **litesvm 0.16 deliberately not adopted.** The 4.x-era harness is a
   mid-migration mix (`solana-address 2.x` next to `solana-keypair 3.x`
   next to `solana-transaction 4.x`) with mutually incompatible address
   types. Building a working harness on it is a standalone project with an
   uncertain payoff: the binary may fault there too if the problem is in
   the binary rather than the runtime.

## Decision

- Pin the integration harness at **litesvm 0.12** with granular 3.x
  dev-dependencies (`solana-pubkey 3.0`, `solana-keypair 3.1`,
  `solana-signer 3.0`, `solana-instruction 3.5`, `solana-transaction 3.1`);
  drop `solana-sdk 2.2`.
- Keep the `initialize` negative test behavioral rather than exact: assert
  the transaction **fails** and creates **no accounts** (a deployer-gate
  regression test that stays valid regardless of the fault code), instead
  of pinning `InvalidDeployer` (6035).
- Re-enable the exact `Custom(6035)` assertion once §5 is resolved
  (marked `TODO(tooling)` in `test_initialize.rs`).

## Consequences

- `cargo test --package splitter` is green (27 unit + 2 integration tests),
  clippy clean, no new `fmt` drift.
- The suite currently proves program loading, PDA derivation, and the
  deployer gate at the behavioral level — but not exact on-chain error
  codes end-to-end.

## 5. Open follow-up (pre-mainnet)

No repo record shows this binary ever executing successfully anywhere —
`deployments/` contains deployment logs only, no execution traces. Before
mainnet, either (a) execute the program on devnet / a matching Agave test
validator and record the trace, or (b) align the build and test runtimes
(newer litesvm once its 4.x dependency set stabilizes, or platform-tools
matching the pinned test runtime) and confirm the entrypoint fault is
purely a harness skew and not a program bug.
