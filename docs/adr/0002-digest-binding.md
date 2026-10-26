# ADR-0002: Solana Digest Binding for v1.4

- **Status**: Accepted
- **Date**: 2026-Q3
- **Deciders**: AiFinPay core team
- **Supersedes**: ADR-0001 §2 ("EIP-712-style digest on Solana") in wording only
- **Related**: EVM splitter v1.4, `programs/splitter`

## Context

ADR-0001 described the Solana splitter as using an "EIP-712-style digest". The
goal was to keep cross-chain quote semantics identical so a single secp256k1
signer can authorize payments on both EVM and Solana. During implementation the
team made a concrete choice that was only partially reflected in the existing
docs:

- The Solana program does **not** use EIP-712 or keccak256 for the quote digest.
- It uses a Solana-native construction: SHA-256 over a tagged, program-bound,
  Borsh-serialized `Quote`.
- The off-chain signer is still the same secp256k1 key, but it signs a
  different digest on each chain. Wallets/relayers that build for both chains
  must branch at signing time.

This ADR records that decision explicitly, explains the rationale, and fixes the
out-of-date language in ADR-0001, `ARCHITECTURE.md`, `SECURITY.md`, and the
`splitter/README.md`.

## Decision

### 1. Keep a Solana-specific digest for v1.4

The canonical digest on Solana is:

```
digest = SHA-256(MESSAGE_DOMAIN_TAG || program_id || Borsh(Quote))
```

Where:

- `MESSAGE_DOMAIN_TAG = b"AiFinPay-Solana-v1.4"`
- `program_id` is the 32-byte program ID (`crate::ID`).
- `Borsh(Quote)` is the Anchor/Borsh serialization of the `Quote` struct in the
  field order declared in `state.rs`.

The implementation is `quote_message_hash()` in `programs/splitter/src/utils.rs`.
It is also exposed as `digest()` for tests and off-chain compatibility.

### 2. Do not migrate to an EIP-712 / keccak256 digest for v1.4

We keep the Solana-specific digest for the following reasons:

- **Solana runtime affinity**: SHA-256 is the native hash in `solana_program::hash`.
  The program uses it for both the quote digest and the `payment_id` event hash.
- **Borsh is the natural serialization in Anchor**: re-using the existing
  `AnchorSerialize` derive removes hand-written encoding and reduces the risk
  of field-order bugs.
- **Program-bound by default**: including `program_id` in the digest binds every
  signature to a single deployment, preventing replay across Solana clusters and
  across program upgrades.
- **Cross-chain constants stay identical**: route IDs, fee caps, and the `Quote`
  field order are still shared with EVM v1.4. Only the final digest bytes differ.
- **Risk of change outweighs parity**: re-writing the digest to match EIP-712
  byte-for-byte would require adding a keccak hasher dependency and hand-coded
  ABI-like encoding. The security benefit of full byte parity is not worth the
  added complexity for the v1.4 release.

### 3. Off-chain signing convention

Wallets and relayers must implement two signing paths:

| Chain   | Hash function | Payload encoding                                | Domain binding                         |
|---------|---------------|-------------------------------------------------|----------------------------------------|
| EVM     | keccak256     | EIP-712 `encodeData` of `Quote` struct          | `EIP712_DOMAIN` (name, version, chain) |
| Solana  | SHA-256       | Borsh serialization of `Quote`                  | `program_id`                           |

Both paths use the same secp256k1 private key and the same 65-byte signature
layout (`r || s || v`, with `v = 27 + recovery_id`). The program rejects
high-s signatures exactly as EIP-2 does.

### 4. Cross-chain regression vectors

The Rust program and TypeScript signer share a pinned Solana digest fixture
(`digest_fixture_for_ts_parity` in `lib.rs` and
`scripts/signer/signer-parity.test.ts`). The EVM digest is chain-specific and
is not expected to match the Solana fixture byte-for-byte.

## Consequences

### Positive

- Solana code is simpler and uses only Solana-native primitives.
- Signatures are automatically program-bound and cluster-bound.
- Borsh serialization is generated, so field-order bugs are caught by the
  compiler.
- The cross-chain quote schema and business constants remain identical.

### Negative

- A signature valid on EVM is **not** valid on Solana, and vice versa. Relayers
  must be chain-aware when signing.
- Existing documentation (ADR-0001, `ARCHITECTURE.md`, `README.md`) used
  "EIP-712-style" loosely and is now misleading.
- The `payment_id` hash is also chain-specific (SHA-256), so event
  deduplication logic cannot be naively shared across chains.

### Neutral

- Future versions may still introduce a chain-agnostic digest if the protocol
  adds a dedicated cross-chain signing layer. Any such change is a coordinated
  upgrade and requires a new ADR.

## Alternatives considered

1. **Full EIP-712 byte parity on Solana** — rejected: would require keccak256
   everywhere and ABI-like encoding, increasing audit surface without a clear
   security gain.
2. **Chain-bound domain separator added to EIP-712** — rejected: still leaves
   the Solana program dependent on keccak256 and custom encoding; the simpler
   Solana-native construction is preferred for v1.4.
3. **Keep the ADR-0001 wording as-is** — rejected: the documentation drift
   would confuse auditors and integrators.
