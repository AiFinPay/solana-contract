# Security

## Reporting a Vulnerability

If you discover a security issue in the AiFinPay Solana Splitter, please
**do not open a public GitHub issue**.

Email **security@aifinpay.example** with:

- A clear description of the issue and impact.
- Reproduction steps (transaction hash, accounts, signatures — never send
  the **private key** of any signer).
- Your assessment of severity (Critical / High / Medium / Low).

We commit to:

- Acknowledgement within **72 hours**.
- A triage decision within **7 days**.
- A coordinated disclosure timeline agreed with the reporter.

We follow responsible disclosure and credit reporters in the post-mortem
unless anonymity is requested.

## Threat Model

### Trust assumptions

| Role       | Powers                                                          | Trust level |
|------------|------------------------------------------------------------------|-------------|
| `admin`    | Rotate roles, edit token list, edit routes, set treasury, pause  | Trusted     |
| `pauser`   | Pause the program                                                | Semi-trusted |
| `signer`   | Sign chain-specific digests over the canonical `Quote` (secp256k1) | Trusted     |
| `payer`    | Submit signed settlements                                        | Untrusted   |
| `merchant` | Receive the merchant leg                                          | Untrusted   |
| `ip_creator` | Receive the royalty leg                                       | Untrusted   |

The protocol assumes `admin`, `pauser`, and `signer` keys are held in
distinct HSMs / cold wallets operated by independent parties. Compromise
of two of the three is required for catastrophic damage; loss of any
single key is recoverable via rotation. Deployments may consolidate
`admin`, `pauser` and `treasury` on a single multisig vault — this is
permitted by the program but weakens the assumption above to the security
of that one vault.

### Out of scope

- Bugs in the Solana runtime.
- Bugs in `anchor-lang` / `anchor-spl` / `litesvm`.
- Bugs in the off-chain relayer / quote-builder.
- Loss of funds caused by user-side key mismanagement.

### In scope

- Signer bypass (e.g. malleable signature, replay, weak recovery).
- Lamport / SPL mis-accounting that benefits an attacker.
- Replay across payers, routes, or program upgrades.
- Reentrancy via CPI (we deliberately avoid CPIs except SPL transfers).
- Fee-cap bypass (e.g. via excessive bps values or negative splits).
- DoS via state bloat, oversized PDAs, or unbounded loops.

## Security-Critical Invariants

These properties are checked by `cargo test` and MUST remain green:

1. **Signer exclusivity** — every accepted settlement recovers to
   `config.signer` exactly. (`InvalidSigner`.)
2. **Replay safety** — a `(payer, nonce)` pair cannot settle twice.
   (`NonceAlreadyConsumed`.)
3. **No zero-value splits** — `merchant_amt > 0`, and any non-zero
   `treasury_bps` / `ip_creator_bps` must produce a non-zero fee.
4. **Fee-cap enforcement** — `treasury_bps <= 500`, `ip_creator_bps <= 100`.
5. **No role overlap (operational, not on-chain)** — admin, pauser and
   treasury SHOULD be distinct keys / vaults. The program permits them to
   coincide (single-multisig deployments) and enforces no distinctness.
6. **Pause enforcement** — `settle_native` and `settle_stable` reject
   while `is_paused`.
7. **Cross-chain quote parity** — the `Quote` schema, route IDs, and fee
   caps match EVM v1.4. The digest construction is chain-specific (SHA-256 /
   Borsh / `program_id` on Solana; EIP-712 / keccak256 on EVM) — see
   ADR-0002.

## Operational Security

### Key handling

- The `signer` secp256k1 private key MUST be stored in an HSM or a
  threshold-signing service. The `Config.signer` field stores only the
  uncompressed public key.
- The `admin` and `pauser` keys are Solana Ed25519 keypairs. Use hardware
  wallets (Ledger) for any non-localnet deployment.
- **Never** commit keypairs to the repository. `.gitignore` already
  excludes `keypairs/`, `*.json.bak`, and `.env`.

### Deployment checklist

#### `splitter`

1. `cargo build-sbf` produces `target/deploy/splitter.so`. Verify the
   file size matches the previous release within ±5%.
2. Verify the declared program ID matches `56cRuWVNt5KXRgvA4m6wroB4D45A3SjvowZVZXYBw3Mr`.
3. Use a multi-sig (Squads) for the `admin` on mainnet.
4. Run `initialize` once, immediately transferring ownership of the
   multi-sig.
5. Confirm `quote_total` returns the expected splits for a dry-run quote
   before opening the program to live traffic.

### Incident response

- If a bug is reported, **pause immediately** via `pause` (admin or
  pauser).
- The `pause` flag halts both `settle_native` and `settle_stable`. It
  does **not** freeze funds already credited.
- A `revoke_signer_role` zeroes the trusted signer key. After revocation
  no further settlements can be signed until `grant_signer_role` is
  called with a fresh key.

## Audits

This program is part of the v1.4 cross-chain release. Audit reports
covering the Solana program are committed alongside this repo under
`audits/`. Any deployment to a non-localnet cluster MUST be covered by a
recent audit report.

If you are running a fork of this code, you are responsible for your own
audit trail. See `CONTRIBUTING.md` for the review expectations that apply
to changes in this repo.

## Safe Integer Math

All fee math uses `u128` intermediates (`(gross as u128) * bps / 10_000`)
and `checked_*` for every subtraction and addition. A settlement that
would overflow (`NonceOverflow`, `MerchantTransferFailed`, …) reverts
the entire transaction — there are no partial successes.

## Disclaimer

This document does not constitute a warranty. Use at your own risk and
always perform an independent security review before deploying to a
cluster that handles real funds.