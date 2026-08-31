# settlement-v1 build evidence

Reproduce with `scripts/verify-build.sh` (fails closed on mismatch).

sha256: 7115ff596ab067ed0f39d7e5d952f87330f4584ba0ebf19d0ca7dc3369109763

| field | value |
|---|---|
| artifact | `target/deploy/aifinpay_settlement_v1.so` |
| size (bytes) | 177976 |
| built | 2026-08-31T07:21:31Z |
| host | Darwin arm64 |
| rustc (host, pinned) | 1.97.1 |
| cargo-build-sbf | cargo-build-sbf 4.1.0 platform-tools v1.54 rustc 1.89.0  |
| solana-program (Cargo.lock) | 2.3.0 |
| anchor-lang | 0.32.1 |

Notes:
- `cargo-build-sbf` ships its own pinned rustc via platform-tools, so unlike
  the host-rustc NEAR build the artifact is expected to reproduce across hosts.
  Record the platform-tools version above; if it changes, the size/hash may
  shift by a few hundred bytes (1.53 → 1.54 moved it 176,376 → 175,808).
- This size (with the H-1 dust fix) supersedes both earlier figures.
- Determinism confirmed: two clean builds with the committed Cargo.lock produce
  the identical hash. The lock is load-bearing — `generate-lockfile` re-resolving
  a Solana patch version changed the artifact once (same size, different hash).
- NOT YET compared with `solana-verify` (Docker verifiable build) — do that at
  deploy time and append the on-chain program hash + deployed slot.
