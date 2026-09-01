# AiFinPay Solana Splitter — Architecture

## 1. Purpose

The `splitter` program is the **Solana counterpart** of the AiFinPay B2B payment
splitter v1.4. It accepts signed payment quotes, splits the gross amount into a
merchant leg, an optional protocol-treasury leg, and an optional IP-creator
royalty leg, and atomically credits the three legs in a single transaction.

Cross-chain design constraint: **the on-chain payment semantics, route
identifiers, fee caps, and EIP-712-style digest MUST match the EVM v1.4
contract exactly.** Wallets, off-chain relayers, and payment APIs rely on a
single quote schema being valid on both chains.

## 2. High-Level Architecture

```
                 ┌──────────────────────────────────────────┐
                 │           Off-chain payer API            │
                 │  builds Quote + secp256k1 signature over │
                 │         EIP-712-style digest             │
                 └─────────────────────┬────────────────────┘
                                       │  (signed Quote)
              ┌────────────────────────┼────────────────────────┐
              │                        │                        │
              ▼                        ▼                        ▼
   ┌─────────────────────┐  ┌─────────────────────┐  ┌─────────────────────┐
   │ settle_native       │  │ settle_stable       │  │  quote_total        │
   │ (SOL legs)          │  │ (SPL legs)          │  │  (view: dry-run)    │
   └──────────┬──────────┘  └──────────┬──────────┘  └──────────┬──────────┘
              │                        │                        │
              ▼                        ▼                        ▼
   ┌──────────────────────────────────────────────────────────────────┐
   │                          Core verifier                            │
   │   digest() → recover_signer() → verify_quote_core() → split_gross()│
   └──────────────────────────────────────────────────────────────────┘
              │                        │
              ▼                        ▼
   ┌─────────────────────┐  ┌─────────────────────┐
   │ Native SOL transfers│  │ SPL CPI transfers   │
   │ (system_program)    │  │ (anchor_spl::token) │
   └─────────────────────┘  └─────────────────────┘
                                       │
                                       ▼
                         ┌──────────────────────────┐
                         │ Payment event (Anchor    │
                         │ emit!) + payment_id hash │
                         └──────────────────────────┘
```

## 3. Module Layout

```
programs/splitter/
├── AGENTS.md              # Program-level agent instructions
├── README.md              # Program-level quick reference
├── lib.rs                 # declare_id, #[program] entrypoints, inline unit tests
├── instructions.rs        # Re-exports every instruction module
├── constants.rs           # EIP-712 name/version, typehashes, route IDs, seeds, caps
├── state.rs               # Config, TokenList, RouteProfileEntry, ProfilesIndex,
│                          # PayerNonce, ConsumedNonce, Quote
├── error.rs               # ErrorCode variants (one per revert condition)
├── utils.rs               # digest(), domain_separator(), quote_hash(),
│                          # recover_signer(), verify_quote_core(),
│                          # split_gross(), emit_payment()
└── instructions/
    ├── initialize.rs              # one-shot bootstrap (admin, signer, pauser, …)
    ├── settle_native.rs           # SOL settlement
    ├── settle_stable.rs           # SPL stablecoin settlement
    ├── quote_total.rs             # view: returns split for (gross, route, ip_creator)
    ├── pause.rs / unpause.rs      # circuit breaker
    ├── set_treasury.rs            # admin rotates global treasury
    ├── configure_route.rs         # admin sets per-route fees + route_treasury
    ├── enable_route.rs / disable_route.rs
    ├── set_whitelisted_tokens.rs  # admin edits SPL whitelist
    └── grant_*_role / rotate_*_role  # admin rotates signer & pauser

programs/splitter_light/
├── AGENTS.md              # Program-level agent instructions
├── README.md              # Program-level quick reference
└── src/                   # Same module layout (minimal hardcoded variant)
    ├── lib.rs
    ├── constants.rs
    ├── state.rs
    ├── error.rs
    ├── utils.rs
    ├── instructions.rs
    └── instructions/
        ├── settle_native.rs
        ├── settle_stable.rs
        └── set_signer.rs
```

## 4. On-Chain State

All accounts are PDAs derived from a single program ID. They are laid out so
the runtime can verify ownership cheaply and so off-chain indexers can
discover them deterministically.

| PDA                    | Seeds                                      | Mutable? | Purpose                                  |
|------------------------|--------------------------------------------|----------|------------------------------------------|
| `config`               | `[CONFIG_SEED]`                            | mut      | RBAC, pause flag, treasury, references   |
| `token_list`           | `[TOKEN_LIST_SEED]`                        | mut      | Whitelisted SPL stablecoin mints         |
| `profiles`             | `[PROFILES_INDEX_SEED]`                    | mut      | `Vec<RouteProfileEntry>` (≤ MAX_ROUTES)  |
| `payer_nonce`          | `[PAYER_NONCE_SEED, payer]`                | mut      | Per-payer monotonic nonce counter        |
| `consumed_nonce`       | `[CONSUMED_NONCE_SEED, payer, nonce_le]`   | mut      | Marker that `(payer, nonce)` is consumed |

### 4.1 `Config` — RBAC registry

Single global account storing the three role holders plus references to the
auxiliary PDAs:

- `admin: Pubkey` — may pause, unpause, edit token list, configure routes,
  rotate treasury, rotate signer and pauser.
- `signer: [u8; 64]` — secp256k1 uncompressed public key. The on-chain
  ecrecover result MUST equal this value for a quote to be accepted.
- `pauser: Pubkey` — may pause (but not unpause) and may be revoked by admin.
- `treasury: Pubkey` — default treasury for all routes unless a route profile
  overrides it via `route_treasury`.
- `is_paused: bool` — emergency circuit breaker checked at the top of every
  settle instruction.

### 4.2 `RouteProfileEntry`

A `(route_id → fee config)` row.

```
route_id        : [u8; 32]  // keccak256 of route name
treasury_bps    : u16       // 0..= MAX_TREASURY_BPS  (500)
ip_creator_bps  : u16       // 0..= MAX_IP_CREATOR_BPS (100)
enabled         : bool
configured_at   : i64
route_treasury  : Pubkey    // override of global treasury, or default()
```

`split_gross` rejects configurations where `ip_creator_bps > 0` but the quote's
`ip_creator` is `Pubkey::default()`.

### 4.3 `PayerNonce` / `ConsumedNonce`

Two-account replay protection:

1. `payer_nonce` PDA is initialized on the first settlement and tracks the
   next expected nonce for that payer.
2. `consumed_nonce` PDA is a marker keyed by `(payer, nonce)` that flips to
   `true` once the pair has been used.

A settlement is accepted iff `quote.nonce == payer_nonce.nonce` and the
matching `consumed_nonce` is not yet set. After success, both PDAs are
updated atomically with the same instruction.

## 5. Quote Schema and Signing

`Quote` is the canonical EIP-712-style payment intent (see `state.rs:71`):

```
payer           Pubkey
merchant        Pubkey
token           Pubkey    // Pubkey::default() for native SOL
gross_amount    u64       // lamports or base-units of the SPL mint
ip_creator      Pubkey    // Pubkey::default() if no royalty
valid_until     i64       // unix timestamp
order_id_hash   [u8; 32]  // opaque, client-supplied correlation id
nonce           u64       // payer-monotonic
route_id        [u8; 32]  // keccak256 of route name
```

The digest is built exactly like the EVM v1.4 contract:

```
prefix  = 0x19 0x01
domain  = keccak256( DOMAIN_TYPEHASH || keccak256(EIP712_NAME)
                                  || keccak256(EIP712_VERSION)
                                  || program_id )
qhash   = keccak256( QUOTE_TYPEHASH || payer || merchant || token ||
                     gross_amount_le || ip_creator || valid_until_le ||
                     order_id_hash || nonce_le || route_id )
digest  = keccak256( prefix || domain || qhash )
```

The 65-byte signature is `r (32) || s (32) || v (1)`; `v - 27` is the
secp256k1 recovery id passed to `solana_secp256k1_recover`.

## 6. Settlement Flows

### 6.1 `settle_native` (SOL)

```
payer  ─┬──► merchant      (gross - treasury - ip_royalty)
        ├──► treasury      (treasury_bps × gross / 10_000)
        └──► ip_creator    (ip_creator_bps × gross / 10_000)
```

- The transaction must include enough lamports to cover `gross_amount` plus
  rent for the two nonce PDAs.
- Lamport movements are performed by direct lamport mutation under account
  info (no CPI to `system_program::transfer`); this avoids the system
  program rejecting transfers that would zero out an account.
- `quote.token` MUST be `Pubkey::default()`.

### 6.2 `settle_stable` (SPL)

- `quote.token` MUST be a whitelisted mint.
- Remaining accounts must be:
  `[payer_ata, merchant_ata, treasury_ata, (ip_creator_ata)?]`.
- The handler deserializes each token account and asserts ownership + mint
  before issuing SPL transfers.

### 6.3 `quote_total` (view)

Returns a `QuoteTotalResult { merchant_amount, treasury_amount,
ip_creator_amount, total_amount }` without consuming a nonce. Used by
clients to display a price breakdown before the user signs.

## 7. Role-Based Access Control

| Action                            | Admin | Pauser | Signer |
|-----------------------------------|:-----:|:------:|:------:|
| `pause`                           |   ✓   |   ✓    |        |
| `unpause`                         |   ✓   |        |        |
| `set_treasury`                    |   ✓   |        |        |
| `set_whitelisted_tokens`          |   ✓   |        |        |
| `configure_route`                 |   ✓   |        |        |
| `enable_route` / `disable_route`  |   ✓   |        |        |
| `grant_signer_role`               |   ✓   |        |        |
| `revoke_signer_role`              |   ✓   |        |        |
| `grant_pauser_role`               |   ✓   |        |        |
| `revoke_pauser_role`              |   ✓   |        |        |
| Sign quotes                       |       |        |   ✓    |

Distinctness rules enforced on every rotation:
- `admin != pauser`
- `pauser != first-32-bytes-of(signer)`
- `admin != first-32-bytes-of(signer)`

## 8. Fee Caps

| Field           | Cap (bps) | Percent |
|-----------------|-----------|---------|
| `treasury_bps`  | 500       | 5.0 %   |
| `ip_creator_bps`| 100       | 1.0 %   |

Both are enforced in `initialize` and `configure_route`. A payment is
rejected (`PaymentTooSmallForTreasury` / `PaymentTooSmallForRoyalty`) when
the resulting fee rounds down to zero.

## 9. Cross-Chain Parity (EVM v1.4)

Items that must remain byte-identical across the EVM and Solana deployments:

- EIP-712 `name`, `version`, `DOMAIN_TYPEHASH`, `QUOTE_TYPEHASH`.
- `ROUTE_AGENT_X402` and `ROUTE_MERCHANT_AIFP1` (keccak256 of route names).
- Fee caps.
- Settlement semantics (gross → merchant + treasury + royalty).

Changes to any of these require a coordinated upgrade of the EVM contract.

## 10. Observability

Every successful settlement emits an Anchor `Payment` event with:

```
payment_id        : keccak256 of canonical tuple
payer, merchant, token, gross_amount
merchant_amount, treasury_amount, ip_creator_amount
valid_until, route_id, order_id_hash
```

`payment_id` is unique per `(payer, merchant, token, gross_amount,
ip_creator, valid_until, order_id_hash, nonce, route_id)` tuple, so
indexers can deduplicate retries.

`set_treasury` additionally emits a `TreasuryUpdated` event.

## 11. Build, Test, Deploy

- Rust toolchain is pinned via `rust-toolchain.toml` to `1.89.0`.
- `cargo test` runs the inline unit tests in `lib.rs` and the
  `litesvm`-based integration test in `tests/test_initialize.rs`.
- `cargo build-sbf` is the deploy gate. It produces
  `target/deploy/splitter.so`, which the test harness loads directly.
- The CI pipeline in `.github/workflows/ci.yml` runs formatting, tests,
  clippy, and SBF build on every PR and push to `main` / `dev`.