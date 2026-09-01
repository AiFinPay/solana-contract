# `splitter_light` — AiFinPay Solana Splitter (Light)

Minimal hardcoded Anchor program for signed SOL + USDC/USDT settlement.
This is a lightweight variant of the full AiFinPay Solana splitter v1.4.

## Program ID

```
7vGTUXSmooih99MuzQELyaeFeZmuo4QcstS7T9Jv7yyR
```

## What it does

A payer submits a signed quote. The program verifies the secp256k1
signature, checks the nonce, splits the gross amount into merchant and
treasury legs, and credits them atomically.

```
gross_amount
├── merchant_amount   = gross - treasury
└── treasury_amount   = gross × treasury_bps / 10_000
```

IP-creator royalties are currently parsed from the quote schema but are
not charged by either hardcoded route profile.

## Build

```bash
cargo build-sbf --package splitter_light
```

Produces `target/deploy/splitter_light.so`.

## Test

```bash
cargo test --package splitter_light
```

Runs inline unit tests in `src/lib.rs`. No `litesvm` integration tests
exist yet.

## Lint

```bash
cargo fmt --check --package splitter_light
cargo clippy --package splitter_light --all-targets -- -D warnings
```

## Instructions

| Instruction | Purpose | Caller |
|---|---|---|
| `settle_native` | SOL settlement | payer |
| `settle_stable` | USDC/USDT settlement | payer |
| `set_signer` | Rotate the trusted secp256k1 signer | anyone with a valid signature from the current signer |

There is no on-chain admin, pauser, or treasury registry. The signer
role is the only privileged role.

## Hardcoded configuration

Everything is defined in `src/constants.rs`:

| Constant | Value | Notes |
|---|---|---|
| `USDC_MINT` | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` | Mainnet USDC |
| `USDT_MINT` | `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB` | Mainnet USDT |
| `PROTOCOL_TREASURY` | placeholder | **Must be replaced before mainnet** |
| `INITIAL_SIGNER` | placeholder | **Must be replaced before mainnet** |
| `ROUTE_AGENT_X402` | `0x8dc505be…` | Fee-free agent route |
| `ROUTE_MERCHANT_AIFP1` | `0xb9dbf587…` | 1% treasury merchant route |

## Signer rotation (`set_signer`)

The trusted signer is rotated by presenting a valid secp256k1 signature
from the *current* signer over a digest of `(current_signer, new_signer)`. On the
very first call, when the `Config` PDA has not yet been initialized, the
signature is verified against the hardcoded `INITIAL_SIGNER`.

There is **no on-chain admin fallback**. If the current signer private
key is compromised, the protocol cannot recover control through an admin
instruction.

## Quote schema

Same EIP-712-style `Quote` struct as `splitter`. Native SOL uses
`token == Pubkey::default()`; stable settlement uses `USDC_MINT` or
`USDT_MINT`.

## Program documentation

- [`AGENTS.md`](./AGENTS.md) — agent instructions for this program
- Repo-level [`ARCHITECTURE.md`](../../ARCHITECTURE.md) — full architecture
- Repo-level [`SECURITY.md`](../../SECURITY.md) — threat model and invariants
- Repo-level [`docs/BUSINESS_LOGIC.md`](../../docs/BUSINESS_LOGIC.md) — business rules
- Repo-level [`docs/IMPLEMENTATION.md`](../../docs/IMPLEMENTATION.md) — feature status

## Deployment checklist

Before mainnet deployment:

1. Replace `PROTOCOL_TREASURY` with a real protocol-owned wallet.
2. Replace `INITIAL_SIGNER` with a valid secp256k1 uncompressed public key.
3. Run `cargo fmt --check`, `cargo clippy`, `cargo test`, and
   `cargo build-sbf`.
4. Run a full security audit and store the report.
5. Verify the deployed program ID matches the one declared in
   `src/lib.rs`.
