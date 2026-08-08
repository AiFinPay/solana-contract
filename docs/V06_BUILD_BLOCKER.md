# v0.6 build blocker — `pyth-solana-receiver-sdk` is internally inconsistent

**`cargo check` fails on this branch, and failed before the v0.6 work landed.
Every error is inside Pyth; none are in this program's source.** CI is red for
this reason, not because of the v0.6 instruction.

```
error[E0277]: the trait bound `pythnet_sdk::messages::PriceFeedMessage: BorshSerialize` is not satisfied
error[E0277]: the trait bound `pythnet_sdk::messages::PriceFeedMessage: BorshDeserialize` is not satisfied
error: could not compile `pyth-solana-receiver-sdk` (lib) due to 4 previous errors
```

## Root cause

`pyth-solana-receiver-sdk` 1.1.0 declares two dependencies that disagree with
each other:

| Its dependency | anchor-lang it pulls |
|---|---|
| `anchor-lang` (direct) | **0.32.1** |
| `pythnet-sdk` 2.3.1 → `anchor-lang` | **1.1.2** |

Inside that crate, `#[account]` on `PriceUpdateV2` derives anchor **0.32.1**'s
`AnchorSerialize`, while the `PriceFeedMessage` field it contains derives
anchor **1.1.2**'s (via pythnet-sdk's `solana-program` feature, which
`pyth-solana-receiver-sdk` enables itself). Those are two distinct traits from
two borsh generations, so the bound cannot be satisfied.

Nothing a consumer sets fixes this, because both sides are pinned inside the
dependency. Confirmed by trying every available combination:

| Attempt | Result |
|---|---|
| `pyth` 1.2.0 (what the caret range floats to) | same error |
| `pyth` pinned `=1.1.0` | same error |
| `pyth` 0.6.1 / 0.3.0 | proc-macro derive panics |
| `pyth` 0.5.0 | unresolvable `zeroize` |
| our `anchor-lang` → 1.1.2 | same error |
| `pythnet-sdk` feature `solana-program` forced on | same error |
| `pythnet-sdk` pinned < 2.3.1 | rejected: `pyth` requires `^2.3.1` |
| `pythnet-sdk` 2.3.2 / 2.3.3 / 2.3.4 | do not exist |

## Why the tree used to build

`contract/Cargo.lock` was never committed. It is not in `.gitignore` — it was
simply untracked, so every clone re-resolved transitive dependencies from
scratch. A tree that built on one machine stopped building on the next without
a single source change. **Committing the lockfile is the fix for that**, and it
is a prerequisite for the reproducible build the release gate asks for.

## Scope

Pyth is used only by the seat-donation pricing path (`reserve_seat_sol`,
`top_up_sol`). Neither B2B settlement instruction touches it. The blocker is
orthogonal to the v0.6 work — but it gates the whole artifact, because the
program deploys as one binary.

Verified by building a throwaway copy with `get_feed_id_from_hex` and
`PriceUpdateV2` stubbed out: that tree compiles clean. So the rest of the
program, v0.6 settlement included, is sound. The probe was not committed.

## Options

1. **Drop Pyth from this program.** The price feed serves the donation path
   only and is currently holding the entire program hostage. The SOL/USD rate
   could come from the client, as the EVM side already does with its
   `nativeUsdEnv` policy. Smallest change, removes the dependency permanently.
2. **Wait for a `pyth-solana-receiver-sdk` release** whose anchor-lang and
   pythnet-sdk agree. Out of our control.
3. **Vendor a patched `pyth-solana-receiver-sdk`** via `[patch.crates-io]`,
   pinning both sides to one anchor. Works, but we then own a fork of a
   dependency in the settlement path — worth avoiding.

Option 1 is the recommendation.

## Until it is resolved

- `anchor build` does not succeed, so the v0.6 release gate is **not met**.
- The IDL cannot be regenerated from a compiled program; `scripts/sync-idl.mjs`
  cannot be trusted as a drift gate while the source does not compile.
- `cargo test --lib` cannot run, so the required negative cases — replay,
  wrong mint, wrong treasury, wrong merchant, wrong amount, malformed id,
  unauthorized payer — remain unproven.
- **The Solana route stays disabled.** No deployment, no upgrade, no E2E. The
  support matrix must keep saying settlement disabled / pending upgrade, and
  no document may claim Solana fee-on-top settlement is live.
