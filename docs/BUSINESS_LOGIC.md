# Business Logic

This document describes the **business rules** the AiFinPay Solana
Splitter enforces, regardless of the implementation detail. It is the
source of truth for "what does the program do", in language the operations
team, auditors, and integrators can read.

## 1. Actors

| Actor          | Identity                          | Holds                                   |
|----------------|-----------------------------------|-----------------------------------------|
| Payer          | A Solana wallet (Ed25519)         | SOL or whitelisted SPL stablecoin       |
| Merchant       | A Solana wallet                   | The merchant leg of every settlement    |
| IP Creator     | A Solana wallet (optional)        | The royalty leg                         |
| Protocol Treasury | A Solana wallet               | The treasury fee leg                    |
| Route Treasury | A Solana wallet (optional override) | Receives fees for a specific route    |
| Signer (off-chain) | A secp256k1 key              | Signs quotes on behalf of payers        |
| Admin          | A Solana wallet                   | Governance: roles, fees, token list     |
| Pauser         | A Solana wallet                   | Emergency pause                         |

## 2. Concepts

### Quote

A signed payment intent. It is **not** a transaction; it is a payload
that the on-chain program validates before moving funds.

A quote binds together:

- **who pays** (`payer`),
- **who receives what** (`merchant`, `ip_creator`),
- **how much** (`gross_amount`),
- **in what asset** (`token` — `Pubkey::default()` for SOL, otherwise the
  SPL mint),
- **through which route** (`route_id`),
- **until when** (`valid_until`),
- **once only** (`nonce`).

### Route

A named lane through the splitter. Routes exist to keep fee schedules
independent (e.g. an agent-to-agent lane can be fee-free while a
merchant-to-customer lane charges a treasury fee).

Two routes are defined at v1.4:

| Route name            | Route ID (hex prefix) | Default treasury bps | Default IP bps |
|-----------------------|-----------------------|----------------------|----------------|
| `ROUTE_AGENT_X402`    | `0x8dc505be…`         | 0                    | 0              |
| `ROUTE_MERCHANT_AIFP1`| `0xb9dbf587…`         | 100 (1 %)            | 0              |

### Settlement

A successful on-chain transaction that:

1. Verifies the quote against the configured `signer`.
2. Splits `gross_amount` into `(merchant, treasury, ip_creator)`.
3. Credits each leg atomically.
4. Marks the `(payer, nonce)` pair as consumed and bumps the payer's
   nonce counter.

A quote can be settled **at most once**.

## 3. Business Rules

### R-1: One signing authority

There is exactly one signing authority (`Config.signer`). The same key
signs every quote accepted by the program. Rotation is performed by the
admin.

### R-2: Quotes are signed EIP-712-style

The on-chain digest is `keccak256(0x19 || 0x01 || domainSeparator ||
quoteHash)`, matching the EVM v1.4 contract byte-for-byte. The signer
field of EIP-712 is replaced by the Solana `program_id` to bind the
signature to the deployment.

### R-3: Replay protection

A `(payer, nonce)` pair settles **at most once**. After success, the
payer's monotonic nonce counter advances by one.

### R-4: TTL

A quote with `valid_until` in the past is rejected (`SignatureExpired`).
The payer's nonce is **not** consumed on expiry.

### R-5: Fee caps

Treasury fee ≤ 500 bps (5 %). IP-creator royalty ≤ 100 bps (1 %). Both
are absolute ceilings, not defaults — `initialize` and `configure_route`
reject higher values.

### R-6: Splits must be positive

If a configured fee rounds down to zero on a payment, the payment
reverts (`PaymentTooSmallForTreasury` / `PaymentTooSmallForRoyalty`).
This prevents silent fee evasion on tiny amounts.

### R-7: Routes can be enabled / disabled

`disable_route` halts new settlements on a route without deleting its
configuration. `enable_route` re-enables it. Disabling does **not**
revert already-settled funds.

### R-8: Treasury override per route

Each route profile may set its own `route_treasury`, overriding the
global `Config.treasury`. If left at `Pubkey::default()` the global
treasury is used.

### R-9: SPL mint whitelist

Only mints present in `Config.token_list` can be settled via
`settle_stable`. Adding / removing a mint is an admin action; it does
not require a re-init.

### R-10: Emergency pause

`pause` halts both settlement instructions. `unpause` resumes them.
`pause` may be invoked by either admin or pauser; `unpause` is admin
only.

### R-11: Role separation

Admin, pauser, and the first 32 bytes of the secp256k1 signer must be
pairwise distinct. This prevents a single compromised key from being
able to simultaneously settle, pause, and recover the protocol.

### R-12: Native SOL is identified by `Pubkey::default()`

`settle_native` requires `quote.token == Pubkey::default()`. Any other
value fails with `InvalidTokenForNative`. Conversely `settle_stable`
requires `quote.token != Pubkey::default()` and a whitelisted mint.

## 4. Money Flow

```
                  gross_amount
        ┌──────────────┴───────────────┐
        │                              │
        ▼                              ▼
   treasury_amt                   merchant_amt
   = gross × treasury_bps / 10⁴    = gross - treasury - ip_creator
                                   (must be > 0)
        │                              │
        ▼                              ▼
  effective_treasury              quote.merchant
  (route_treasury or
   config.treasury)

                  ip_creator_amt
                  = gross × ip_creator_bps / 10⁴  (if bps > 0)
                  (must be > 0 if bps > 0)
        ┌──────────────┴───────────────┐
        ▼                              ▼
   (skipped if bps == 0)          quote.ip_creator
```

The arithmetic guarantee is:

```
merchant_amt + treasury_amt + ip_creator_amt == gross_amount
```

and each leg is strictly positive when its bps is non-zero.

## 5. Settlement Outcomes

| Outcome                | Funds moved? | State mutated?                          |
|------------------------|--------------|------------------------------------------|
| Signature invalid      | No           | No                                       |
| Quote expired          | No           | No                                       |
| Wrong payer            | No           | No                                       |
| Unknown route          | No           | No                                       |
| Route disabled         | No           | No                                       |
| Nonce mismatch         | No           | No                                       |
| Nonce already consumed | No           | No                                       |
| IP creator missing     | No           | No                                       |
| Amount too small       | No           | No                                       |
| Paused                 | No           | No                                       |
| Successful             | **Yes**      | `payer_nonce`++, `consumed_nonce` set, `Payment` event emitted |

The all-or-nothing property holds because every fund-moving step
(lamport mutation, SPL CPI) is inside a single Anchor instruction. If
anything throws, the entire transaction reverts.

## 6. Off-Chain Responsibilities

The protocol assumes an **honest payer API** that:

- builds quotes that satisfy R-5, R-6, R-12,
- signs them with the on-chain `signer`,
- sends them only once per `(payer, nonce)`,
- retries safely on transient RPC failures (idempotent: same quote, same
  nonce).

If a relayer batches quotes, it must keep the per-payer nonce
sequential — out-of-order submission will revert with `InvalidNonce`.

## 7. Versioning

- On-chain version: **1.4** (`EIP712_VERSION = "1"`, semver-ish: the
  "1.4" suffix is descriptive, not enforced on-chain).
- Off-chain clients MUST pin the EIP-712 `version` to `"1"` and the
  domain `name` to `"AiFinPayB2BSplitter"`. A bump to either is a
  coordinated breaking change.