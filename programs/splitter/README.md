# `splitter` — AiFinPay Solana Splitter v1.4

Full-featured Anchor program for signed, multi-route gross settlement.
This is the Solana counterpart of the AiFinPay EVM splitter v1.4.

## Program ID

```
5QBJgMap7wuFsYfaU8Pmuu2i96GsJ3aBv6mMoUSaPoiS
```

## What it does

A payer submits a signed quote. The program verifies the secp256k1
signature, checks the nonce, splits the gross amount into three legs,
and credits them atomically:

```
gross_amount
├── merchant_amount   = gross - treasury - ip_creator
├── treasury_amount   = gross × treasury_bps / 10_000
└── ip_creator_amount = gross × ip_creator_bps / 10_000   (if configured)
```

## Build

```bash
anchor build --package splitter
anchor keys sync                           # Sync program ID with keypair
```

Produces `target/deploy/splitter.so`.

## Test

```bash
cargo test --package splitter
```

Runs inline unit tests in `src/lib.rs` and the `litesvm` integration test
in `tests/test_initialize.rs`. Make sure `anchor build` has run first
so the `.so` is available.

## Lint

```bash
cargo fmt --check --package splitter
cargo clippy --package splitter --all-targets -- -D warnings
```

## Instructions

| Instruction | Purpose | Caller |
|---|---|---|
| `initialize` | One-shot bootstrap of admin, signer, pauser, treasury, token list, and route profiles | `DEPLOYER` constant |
| `settle_native` | SOL settlement | payer |
| `settle_stable` | SPL stablecoin settlement | payer |
| `quote_total` | Dry-run split for a (gross, route, ip_creator) tuple | anyone |
| `pause` / `unpause` | Circuit breaker | admin or pauser / admin only |
| `set_treasury` | Rotate global treasury | admin |
| `configure_route` | Add/edit route fee config and route treasury override | admin |
| `enable_route` / `disable_route` | Toggle a route | admin |
| `set_whitelisted_tokens` | Add/remove allowed SPL mints | admin |
| `grant_signer_role` / `rotate_signer_role` | Set/rotate the secp256k1 signer | admin |
| `rotate_admin_role` | Transfer admin to a new address | admin |
| `grant_pauser_role` / `rotate_pauser_role` | Set/rotate the pauser | admin |

## Quote schema

```rust
pub struct Quote {
    pub payer: Pubkey,
    pub merchant: Pubkey,
    pub token: Pubkey,         // default for native SOL, whitelisted mint for stable
    pub gross_amount: u64,
    pub ip_creator: Pubkey,    // default if no royalty
    pub valid_until: i64,
    pub order_id_hash: [u8; 32],
    pub nonce: u64,
    pub route_id: [u8; 32],
}
```

The digest is Solana-native and program-bound:

```
digest = SHA-256(b"AiFinPay-Solana-v1.4" || program_id || Borsh(Quote))
```

The secp256k1 signature layout is `r (32) || s (32) || v (1)`. The same
signer key pair is used on EVM, but the signed digest bytes differ by chain
because EVM uses EIP-712 / keccak256. See `docs/adr/0002-digest-binding.md`.

## Routes

| Route | ID (hex prefix) | Default treasury bps | Default IP bps |
|---|---|---|---|
| `ROUTE_AGENT_X402` | `0x8dc505be…` | 0 | 0 |
| `ROUTE_MERCHANT_AIFP1` | `0xb9dbf587…` | 100 | 0 |

Fee caps: treasury ≤ 500 bps (5%), IP creator ≤ 100 bps (1%).

## Program documentation

- [`AGENTS.md`](./AGENTS.md) — agent instructions for this program
- Repo-level [`ARCHITECTURE.md`](../../ARCHITECTURE.md) — full architecture
- Repo-level [`SECURITY.md`](../../SECURITY.md) — threat model and invariants
- Repo-level [`docs/BUSINESS_LOGIC.md`](../../docs/BUSINESS_LOGIC.md) — business rules
- Repo-level [`docs/IMPLEMENTATION.md`](../../docs/IMPLEMENTATION.md) — feature status

## Cross-chain parity

This program shares the quote schema, route IDs, fee caps, and settlement
semantics with the EVM v1.4 deployment. The digest and event `payment_id`
hash are chain-specific (SHA-256 on Solana, keccak256 on EVM). See
[`CONTRIBUTING.md`](../../CONTRIBUTING.md#7-cross-chain-parity-is-sacred) and
ADR-0002.
