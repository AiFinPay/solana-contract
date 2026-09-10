# AiFinPay Solana v1.4 — Backend Integration Guide

## Overview

AiFinPay Splitter v1.4 is a Solana program for signed, multi-route gross settlement.
It splits a gross payment amount into merchant, treasury, and IP creator legs based
on configurable route profiles. The program supports both native SOL and SPL stablecoin
settlements with secp256k1 off-chain signature verification.

| Field | Value |
|---|---|
| Program ID | `DPFAmcgGe7ZaLCRQAZ24Z9SJ8s5gaBWHHbKjLNWsWWBS` |
| Version | `1.4.0` |
| Devnet | Deployed (2026-09-03) |
| Domain Tag | `AiFinPay-Solana-v1.4` |

---

## 1. Prerequisites

```bash
npm install @coral-xyz/anchor @solana/web3.js bs58 secp256k1 sha256 borsh
```

For SPL token operations you also need:

```bash
npm install @solana/spl-token
```

---

## 2. Constants

```typescript
import { PublicKey } from "@solana/web3.js";

// ---------------------------------------------------------------------------
// Program
// ---------------------------------------------------------------------------
export const PROGRAM_ID = new PublicKey("DPFAmcgGe7ZaLCRQAZ24Z9SJ8s5gaBWHHbKjLNWsWWBS");

// ---------------------------------------------------------------------------
// Domain tag (bytes, NOT a string — used as raw prefix in SHA-256)
// ---------------------------------------------------------------------------
export const MESSAGE_DOMAIN_TAG = Buffer.from("AiFinPay-Solana-v1.4");

// ---------------------------------------------------------------------------
// PDA seeds
// ---------------------------------------------------------------------------
export const CONFIG_SEED = Buffer.from("config");
export const TOKEN_LIST_SEED = Buffer.from("token-list");
export const PROFILES_INDEX_SEED = Buffer.from("profiles-index");
export const PAYER_NONCE_SEED = Buffer.from("payer-nonce");
export const CONSUMED_NONCE_SEED = Buffer.from("consumed-nonce");

// ---------------------------------------------------------------------------
// Route IDs (keccak256 of route name — cross-chain constants, DO NOT change)
// ---------------------------------------------------------------------------
export const ROUTE_AGENT_X402 = Buffer.from([
  0x8d, 0xc5, 0x05, 0xbe, 0x33, 0x5e, 0x56, 0x5d,
  0x2a, 0x5e, 0x2c, 0x96, 0x05, 0x7c, 0x7f, 0xb0,
  0xca, 0xff, 0x7c, 0x50, 0x09, 0xf6, 0x1b, 0x82,
  0xac, 0x3e, 0xf5, 0xe7, 0xa9, 0xec, 0x0f, 0x1e,
]);

export const ROUTE_MERCHANT_AIFP1 = Buffer.from([
  0xb9, 0xdb, 0xf5, 0x87, 0xb0, 0xdf, 0x69, 0x87,
  0x0d, 0xf1, 0xe6, 0x0b, 0x22, 0xfb, 0xa0, 0x31,
  0x7f, 0x53, 0xeb, 0x19, 0xd7, 0x8a, 0x57, 0x3a,
  0xbf, 0x94, 0xfc, 0x38, 0x4a, 0x33, 0x9a, 0x89,
]);

// ---------------------------------------------------------------------------
// Fee caps
// ---------------------------------------------------------------------------
export const BPS_DENOMINATOR = 10_000;
export const MAX_TREASURY_BPS = 500;   // 5.0%
export const MAX_IP_CREATOR_BPS = 100;  // 1.0%
```

---

## 3. PDA Derivation

```typescript
import { PublicKey } from "@solana/web3.js";
import {
  PROGRAM_ID,
  CONFIG_SEED,
  TOKEN_LIST_SEED,
  PROFILES_INDEX_SEED,
  PAYER_NONCE_SEED,
  CONSUMED_NONCE_SEED,
} from "./constants";

// Singleton PDAs
export const [configPDA] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
export const [tokenListPDA] = PublicKey.findProgramAddressSync([TOKEN_LIST_SEED], PROGRAM_ID);
export const [profilesPDA] = PublicKey.findProgramAddressSync([PROFILES_INDEX_SEED], PROGRAM_ID);

// Per-payer PDAs (call with the payer's PublicKey)
export function derivePayerNonce(payer: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([PAYER_NONCE_SEED, payer.toBuffer()], PROGRAM_ID);
}

export function deriveConsumedNonce(payer: PublicKey, nonce: bigint): [PublicKey, number] {
  const nonceBytes = Buffer.alloc(8);
  nonceBytes.writeBigUInt64LE(nonce);
  return PublicKey.findProgramAddressSync(
    [CONSUMED_NONCE_SEED, payer.toBuffer(), nonceBytes],
    PROGRAM_ID,
  );
}
```

---

## 4. Quote Structure

The `Quote` is the signed payment intent. Field order matters for Borsh serialization.

```typescript
export interface Quote {
  payer: PublicKey;       // Payer's Solana wallet
  merchant: PublicKey;    // Merchant's Solana wallet
  token: PublicKey;       // Pubkey.default() for native SOL; SPL mint for stablecoins
  grossAmount: bigint;   // u64 — lamports or SPL base units
  ipCreator: PublicKey;   // IP creator wallet (use Pubkey.default() if no royalty)
  validUntil: bigint;    // i64 — Unix timestamp expiry
  orderIdHash: Buffer;   // [u8; 32] — opaque client correlation ID hash
  nonce: bigint;         // u64 — payer-monotonic nonce
  routeId: Buffer;       // [u8; 32] — keccak256 of route name
}
```

### Borsh Serialization

Use the Anchor-compatible Borsh layout. The field order MUST match the on-chain struct:

```typescript
import { BorshSchema } from "borsh";

// Borsh schema for Quote (field order is critical)
const quoteSchema = {
  struct: {
    payer: { array: { type: "u8", len: 32 } },
    merchant: { array: { type: "u8", len: 32 } },
    token: { array: { type: "u8", len: 32 } },
    grossAmount: "u64",
    ipCreator: { array: { type: "u8", len: 32 } },
    validUntil: "i64",
    orderIdHash: { array: { type: "u8", len: 32 } },
    nonce: "u64",
    routeId: { array: { type: "u8", len: 32 } },
  },
};

export function serializeQuote(quote: Quote): Buffer {
  const layout = BorshSchema(quoteSchema);
  return Buffer.from(
    layout.encode({
      payer: quote.payer.toBytes(),
      merchant: quote.merchant.toBytes(),
      token: quote.token.toBytes(),
      grossAmount: quote.grossAmount,
      ipCreator: quote.ipCreator.toBytes(),
      validUntil: quote.validUntil,
      orderIdHash: quote.orderIdHash,
      nonce: quote.nonce,
      routeId: quote.routeId,
    }),
  );
}
```

---

## 5. Digest & Signing

**Critical:** The digest is Solana-native, NOT EIP-712.

```
digest = SHA-256( domain_tag || program_id || Borsh(Quote) )
```

Where:
- `domain_tag` = `b"AiFinPay-Solana-v1.4"` (19 bytes raw)
- `program_id` = 32 bytes of the program address
- `Borsh(Quote)` = Borsh-serialized quote struct

### Compute the Digest

```typescript
import { createHash } from "crypto";
import { PublicKey } from "@solana/web3.js";
import { PROGRAM_ID, MESSAGE_DOMAIN_TAG } from "./constants";
import { serializeQuote, Quote } from "./quote";

export function computeDigest(quote: Quote): Buffer {
  const quoteBytes = serializeQuote(quote);

  // SHA-256(domain_tag || program_id || borsh(quote))
  const hash = createHash("sha256");
  hash.update(MESSAGE_DOMAIN_TAG);
  hash.update(PROGRAM_ID.toBuffer());
  hash.update(quoteBytes);
  return hash.digest(); // 32 bytes
}
```

### Sign with secp256k1

```typescript
import * as secp256k1 from "secp256k1";

/**
 * Sign a quote with a secp256k1 private key.
 * Returns a 65-byte signature: r(32) || s(32) || v(1)
 * where v = 27 + recovery_id.
 *
 * Rejects high-s signatures (EIP-2 canonical).
 */
export function signQuote(digest: Buffer, privateKey: Buffer): Buffer {
  // Sign the digest as a prehash
  const { signature, recid } = secp256k1.ecdsaSign(
    new Uint8Array(digest),
    new Uint8Array(privateKey),
  );

  // Build 65-byte signature: r || s || v
  const sig65 = Buffer.alloc(65);
  signature.copy(sig65, 0); // r(32) + s(32)
  sig65[64] = 27 + recid;  // v

  // Reject high-s (EIP-2 canonical)
  const s = signature.subarray(32, 64);
  if (isHighS(s)) {
    throw new Error("Non-canonical signature (high-s). Use low-s form.");
  }

  return sig65;
}

function isHighS(s: Uint8Array): boolean {
  // Half secp256k1 curve order N, big-endian
  const HALF_N = Buffer.from(
    "7FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF5D576E7357A4501DDFE92F46681B20A0",
    "hex",
  );
  for (let i = 0; i < 32; i++) {
    if (s[i] !== HALF_N[i]) return s[i] > HALF_N[i];
  }
  return false; // s == HALF_N is allowed (low-s boundary)
}
```

### Recover the Signer (Verification)

```typescript
import * as secp256k1 from "secp256k1";

/**
 * Recover the 64-byte uncompressed public key (X || Y) from a signature.
 * The recovered key must match config.signer on-chain.
 */
export function recoverSigner(digest: Buffer, signature: Buffer): Buffer {
  if (signature.length !== 65) {
    throw new Error("Signature must be 65 bytes");
  }

  const v = signature[64];
  const recoveryId = v - 27;
  if (recoveryId !== 0 && recoveryId !== 1) {
    throw new Error(`Invalid recovery id: ${v}`);
  }

  const r = signature.subarray(0, 32);
  const s = signature.subarray(32, 64);

  // Reject high-s
  if (isHighS(s)) {
    throw new Error("Non-canonical signature (high-s)");
  }

  // secp256k1recover returns 65 bytes: 0x04 || X(32) || Y(32)
  const pubkeyBytes = secp256k1.ecdsaRecover(
    new Uint8Array(signature.subarray(0, 64)),
    recoveryId,
    new Uint8Array(digest),
  );

  // Return the 64-byte uncompressed key (skip the 0x04 prefix)
  return Buffer.from(pubkeyBytes.slice(1, 65));
}
```

---

## 6. Nonce Management

Each payer has a monotonically increasing nonce. The current nonce is stored in the
`PayerNonce` PDA on-chain.

```typescript
import { Connection, PublicKey } from "@solana/web3.js";
import { BN } from "@coral-xyz/anchor";
import { derivePayerNonce } from "./pdas";

/**
 * Fetch the current expected nonce for a payer.
 * Returns 0n if the PDA doesn't exist yet (first settlement).
 */
export async function getNextNonce(
  connection: Connection,
  payer: PublicKey,
): Promise<bigint> {
  const [noncePDA] = derivePayerNonce(payer);
  const accountInfo = await connection.getAccountInfo(noncePDA, "confirmed");

  if (!accountInfo) {
    return 0n; // First settlement
  }

  // Manual Borsh decode: 8-byte discriminator + 32 (payer) + 8 (nonce: u64) + 1 (bump)
  const data = accountInfo.data;
  const nonce = data.readBigUInt64LE(40); // 8 + 32 = 40
  return nonce;
}
```

**Flow:**
1. Query `getNextNonce(connection, payer)` before building the quote
2. Use the returned nonce in the quote and as the `nonce` instruction argument
3. After settlement, the on-chain nonce auto-increments to `nonce + 1`
4. Each nonce is single-use — replaying the same nonce fails with `NonceAlreadyConsumed`

---

## 7. Settlement

### 7.1 Native SOL Settlement (`settle_native`)

```typescript
import { Connection, Keypair, PublicKey, Transaction, SystemProgram } from "@solana/web3.js";
import { Program, BN } from "@coral-xyz/anchor";
import { configPDA, profilesPDA } from "./pdas";
import { derivePayerNonce, deriveConsumedNonce } from "./pdas";
import { computeDigest, signQuote } from "./signing";
import { Quote } from "./quote";

async function settleNative(
  connection: Connection,
  program: Program,
  payer: Keypair,           // The payer's keypair (signs the tx)
  signerPrivateKey: Buffer, // secp256k1 private key for quote signing
  quote: Quote,
): Promise<string> {
  const [payerNoncePDA] = derivePayerNonce(quote.payer);
  const [consumedNoncePDA] = deriveConsumedNonce(quote.payer, quote.nonce);

  // Compute digest and sign
  const digest = computeDigest(quote);
  const signature = signQuote(digest, signerPrivateKey);

  // Resolve effective treasury (route_treasury or global treasury)
  const effectiveTreasury = await resolveTreasury(program, quote.routeId);

  const tx = await program.methods
    .settleNative(
      new BN(quote.nonce.toString()),    // nonce: u64
      {                                   // Quote struct
        payer: quote.payer,
        merchant: quote.merchant,
        token: quote.token,
        grossAmount: new BN(quote.grossAmount.toString()),
        ipCreator: quote.ipCreator,
        validUntil: new BN(quote.validUntil.toString()),
        orderIdHash: quote.orderIdHash,
        nonce: new BN(quote.nonce.toString()),
        routeId: quote.routeId,
      },
      Array.from(signature),             // [u8; 65]
    )
    .accounts({
      config: configPDA,
      payerNonce: payerNoncePDA,
      consumedNonce: consumedNoncePDA,
      payer: payer.publicKey,
      merchant: quote.merchant,
      treasury: effectiveTreasury,
      ipCreator: quote.ipCreator,
      profiles: profilesPDA,
      systemProgram: SystemProgram.programId,
    })
    .signers([payer])
    .rpc();

  return tx;
}
```

**Accounts (in order):**

| # | Account | Writable | Signer | Description |
|---|---------|----------|--------|-------------|
| 0 | `config` | Yes | No | Global config PDA `["config"]` |
| 1 | `payerNonce` | Yes | No | Payer nonce PDA `["payer-nonce", payer]` (init_if_needed) |
| 2 | `consumedNonce` | Yes | No | Nonce marker PDA `["consumed-nonce", payer, nonce]` (init_if_needed) |
| 3 | `payer` | Yes | Yes | Payer wallet |
| 4 | `merchant` | Yes | No | Merchant wallet (must match `quote.merchant`) |
| 5 | `treasury` | Yes | No | Effective treasury (global or route-specific) |
| 6 | `ip_creator` | Yes | No | IP creator wallet (must match `quote.ip_creator`) |
| 7 | `profiles` | Yes | No | Profiles index PDA `["profiles-index"]` |
| 8 | `system_program` | No | No | `11111111111111111111111111111111` |

---

### 7.2 SPL Stablecoin Settlement (`settle_stable`)

```typescript
import { getAssociatedTokenAddressSync } from "@solana/spl-token";

async function settleStable(
  connection: Connection,
  program: Program,
  payer: Keypair,
  signerPrivateKey: Buffer,
  quote: Quote,
  mint: PublicKey, // SPL token mint (must be whitelisted)
): Promise<string> {
  const [payerNoncePDA] = derivePayerNonce(quote.payer);
  const [consumedNoncePDA] = deriveConsumedNonce(quote.payer, quote.nonce);

  const digest = computeDigest(quote);
  const signature = signQuote(digest, signerPrivateKey);

  const effectiveTreasury = await resolveTreasury(program, quote.routeId);

  // Derive ATAs (must be passed as remaining_accounts)
  const payerAta = getAssociatedTokenAddressSync(mint, quote.payer);
  const merchantAta = getAssociatedTokenAddressSync(mint, quote.merchant);
  const treasuryAta = getAssociatedTokenAddressSync(mint, effectiveTreasury);

  // Remaining accounts: [payer_ata, merchant_ata, treasury_ata, (ip_creator_ata)]
  const remainingAccounts = [payerAta, merchantAta, treasuryAta];

  if (!quote.ip_creator.equals(PublicKey.default)) {
    const ipCreatorAta = getAssociatedTokenAddressSync(mint, quote.ipCreator);
    remainingAccounts.push(ipCreatorAta);
  }

  const tx = await program.methods
    .settleStable(
      new BN(quote.nonce.toString()),
      {
        payer: quote.payer,
        merchant: quote.merchant,
        token: quote.token,
        grossAmount: new BN(quote.grossAmount.toString()),
        ipCreator: quote.ipCreator,
        validUntil: new BN(quote.validUntil.toString()),
        orderIdHash: quote.orderIdHash,
        nonce: new BN(quote.nonce.toString()),
        routeId: quote.routeId,
      },
      Array.from(signature),
    )
    .accounts({
      config: configPDA,
      payerNonce: payerNoncePDA,
      consumedNonce: consumedNoncePDA,
      payer: payer.publicKey,
      tokenList: tokenListPDA,
      mint: mint,
      profiles: profilesPDA,
      tokenProgram: new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
      systemProgram: SystemProgram.programId,
    })
    .remainingAccounts(remainingAccounts)
    .signers([payer])
    .rpc();

  return tx;
}
```

**Named Accounts:**

| # | Account | Writable | Signer | Description |
|---|---------|----------|--------|-------------|
| 0 | `config` | No | No | Global config PDA |
| 1 | `payerNonce` | Yes | No | Payer nonce PDA (init_if_needed) |
| 2 | `consumedNonce` | Yes | No | Nonce marker PDA (init_if_needed) |
| 3 | `payer` | Yes | Yes | Payer wallet |
| 4 | `tokenList` | No | No | Token list PDA (validates mint is whitelisted) |
| 5 | `mint` | No | No | SPL token mint address |
| 6 | `profiles` | Yes | No | Profiles index PDA |
| 7 | `token_program` | No | No | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` |
| 8 | `system_program` | No | No | System program |

**Remaining Accounts (order matters):**

| Index | Account | Description |
|-------|---------|-------------|
| 0 | payer_ata | Payer's ATA for the mint |
| 1 | merchant_ata | Merchant's ATA for the mint |
| 2 | treasury_ata | Effective treasury's ATA for the mint |
| 3 | ip_creator_ata | *(optional)* IP creator's ATA — required when `ip_creator_bps > 0` |

---

## 8. Reading On-Chain State

### 8.1 Config Account

```typescript
// Borsh layout for Config (234 bytes total, 8-byte discriminator)
// Fields: admin(32) + signer(64) + pauser(32) + treasury(32) + token_list(32) + profiles(32) + bump(1) + is_paused(1)

interface OnChainConfig {
  admin: PublicKey;
  signer: Buffer;         // 64 bytes — secp256k1 uncompressed pubkey (X || Y)
  pauser: PublicKey;
  treasury: PublicKey;
  tokenList: PublicKey;
  profiles: PublicKey;
  bump: number;
  isPaused: boolean;
}

function decodeConfig(data: Buffer): OnChainConfig {
  let offset = 8; // skip 8-byte Anchor discriminator
  const admin = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const signer = Buffer.from(data.subarray(offset, offset + 64)); offset += 64;
  const pauser = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const treasury = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const tokenList = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const profiles = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;
  const bump = data[offset]; offset += 1;
  const isPaused = data[offset] !== 0;

  return { admin, signer, pauser, treasury, tokenList, profiles, bump, isPaused };
}
```

### 8.2 TokenList Account

```typescript
interface OnChainTokenList {
  admin: PublicKey;
  tokens: PublicKey[];
  bump: number;
}

function decodeTokenList(data: Buffer): OnChainTokenList {
  let offset = 8; // skip discriminator
  const admin = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;

  // Vec<Pubkey>: 4-byte length prefix + N * 32 bytes
  const count = data.readUInt32LE(offset); offset += 4;
  const tokens: PublicKey[] = [];
  for (let i = 0; i < count; i++) {
    tokens.push(new PublicKey(data.subarray(offset, offset + 32)));
    offset += 32;
  }

  const bump = data[offset];
  return { admin, tokens, bump };
}
```

### 8.3 ProfilesIndex Account

```typescript
interface RouteProfileEntry {
  routeId: Buffer;          // [u8; 32]
  treasuryBps: number;      // u16
  ipCreatorBps: number;     // u16
  enabled: boolean;
  configuredAt: bigint;     // i64
  routeTreasury: PublicKey; // 32 bytes
}

interface OnChainProfilesIndex {
  entries: RouteProfileEntry[];
  count: number;
  bump: number;
}

function decodeProfilesIndex(data: Buffer): OnChainProfilesIndex {
  let offset = 8; // skip discriminator

  // Vec<RouteProfileEntry>: 4-byte length prefix
  const count = data.readUInt32LE(offset); offset += 4;
  const entries: RouteProfileEntry[] = [];

  for (let i = 0; i < count; i++) {
    const routeId = Buffer.from(data.subarray(offset, offset + 32)); offset += 32;
    const treasuryBps = data.readUInt16LE(offset); offset += 2;
    const ipCreatorBps = data.readUInt16LE(offset); offset += 2;
    const enabled = data[offset] !== 0; offset += 1;
    const configuredAt = data.readBigInt64LE(offset); offset += 8;
    const routeTreasury = new PublicKey(data.subarray(offset, offset + 32)); offset += 32;

    entries.push({ routeId, treasuryBps, ipCreatorBps, enabled, configuredAt, routeTreasury });
  }

  const countByte = data[offset]; offset += 1;
  const bump = data[offset];

  return { entries, count: countByte, bump };
}
```

---

## 9. Admin Operations

All admin operations require the `admin` keypair as signer.

### 9.1 Configure Route

```typescript
import { PublicKey } from "@solana/web3.js";

async function configureRoute(
  program: Program,
  admin: Keypair,
  routeId: Buffer,            // 32 bytes
  treasuryBps: number,        // 0..=500
  ipCreatorBps: number,       // 0..=100
  routeTreasury: PublicKey,   // or PublicKey.default() for global treasury
): Promise<string> {
  const tx = await program.methods
    .configureRoute(
      Array.from(routeId),
      treasuryBps,
      ipCreatorBps,
      routeTreasury,
    )
    .accounts({
      config: configPDA,
      profiles: profilesPDA,
      admin: admin.publicKey,
    })
    .signers([admin])
    .rpc();

  return tx;
}
```

### 9.2 Enable / Disable Route

```typescript
async function enableRoute(program: Program, admin: Keypair, routeId: Buffer) {
  return program.methods
    .enableRoute(Array.from(routeId))
    .accounts({ config: configPDA, profiles: profilesPDA, admin: admin.publicKey })
    .signers([admin])
    .rpc();
}

async function disableRoute(program: Program, admin: Keypair, routeId: Buffer) {
  return program.methods
    .disableRoute(Array.from(routeId))
    .accounts({ config: configPDA, profiles: profilesPDA, admin: admin.publicKey })
    .signers([admin])
    .rpc();
}
```

### 9.3 Set Whitelisted Tokens

```typescript
async function setWhitelistedTokens(
  program: Program,
  admin: Keypair,
  tokens: PublicKey[],   // mints to add/remove
  allowed: boolean[],    // true = add, false = remove
): Promise<string> {
  return program.methods
    .setWhitelistedTokens(
      tokens,
      allowed,
    )
    .accounts({
      config: configPDA,
      tokenList: tokenListPDA,
      admin: admin.publicKey,
    })
    .signers([admin])
    .rpc();
}
```

### 9.4 Pause / Unpause

```typescript
async function pause(program: Program, pauserOrAdmin: Keypair) {
  return program.methods
    .pause()
    .accounts({ config: configPDA, authority: pauserOrAdmin.publicKey })
    .signers([pauserOrAdmin])
    .rpc();
}

async function unpause(program: Program, admin: Keypair) {
  return program.methods
    .unpause()
    .accounts({ config: configPDA, admin: admin.publicKey })
    .signers([admin])
    .rpc();
}
```

### 9.5 Set Treasury

```typescript
async function setTreasury(
  program: Program,
  admin: Keypair,
  newTreasury: PublicKey,
): Promise<string> {
  return program.methods
    .setTreasury(newTreasury)
    .accounts({ config: configPDA, admin: admin.publicKey })
    .signers([admin])
    .rpc();
}
```

### 9.6 Rotate Signer

```typescript
async function rotateSigner(
  program: Program,
  admin: Keypair,
  newSigner: Buffer, // 64-byte secp256k1 uncompressed pubkey
): Promise<string> {
  return program.methods
    .grantSignerRole(Array.from(newSigner))
    .accounts({ config: configPDA, admin: admin.publicKey })
    .signers([admin])
    .rpc();
}
```

### 9.7 Rotate Admin

```typescript
async function rotateAdmin(
  program: Program,
  admin: Keypair,
  newAdmin: PublicKey,
): Promise<string> {
  return program.methods
    .rotateAdminRole(newAdmin)
    .accounts({ config: configPDA, admin: admin.publicKey })
    .signers([admin])
    .rpc();
}
```

---

## 10. Full Example: End-to-End Flow

```typescript
import { Connection, Keypair, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";
import * as anchor from "@coral-xyz/anchor";
import { Program, BN } from "@coral-xyz/anchor";
import { createHash } from "crypto";
import * as secp256k1 from "secp256k1";
import {
  PROGRAM_ID,
  MESSAGE_DOMAIN_TAG,
  ROUTE_MERCHANT_AIFP1,
  CONFIG_SEED,
  PROFILES_INDEX_SEED,
  PAYER_NONCE_SEED,
  CONSUMED_NONCE_SEED,
} from "./constants";

// ── Setup ──────────────────────────────────────────────────────────────
const connection = new Connection("https://api.devnet.solana.com", "confirmed");
const payer = Keypair.fromSecretKey(/* ... */);
const signerKey = secp256k1.utils.randomPrivateKey(); // secp256k1 key
const signerPubkey = secp256k1.publicKeyCreate(signerKey, false).slice(1); // 64 bytes X||Y

const idl = JSON.parse(readFileSync("target/idl/splitter.json", "utf-8"));
const provider = new anchor.AnchorProvider(connection, new anchor.Wallet(payer), {
  commitment: "confirmed",
});
const program = new Program(idl, provider);

// ── 1. Get current nonce ───────────────────────────────────────────────
const [noncePDA] = PublicKey.findProgramAddressSync(
  [PAYER_NONCE_SEED, payer.publicKey.toBuffer()],
  PROGRAM_ID,
);
const nonceAccount = await connection.getAccountInfo(noncePDA, "confirmed");
const currentNonce = nonceAccount
  ? nonceAccount.data.readBigUInt64LE(40) // 8 (discriminator) + 32 (payer) = 40
  : 0n;

// ── 2. Build quote ─────────────────────────────────────────────────────
const merchant = new PublicKey("MerchantWalletPubkey...");
const ipCreator = PublicKey.default(); // No IP royalty
const grossAmount = BigInt(0.1 * LAMPORTS_PER_SOL); // 0.1 SOL
const validUntil = BigInt(Math.floor(Date.now() / 1000) + 3600); // 1 hour

const orderIdHash = createHash("sha256")
  .update("order-12345") // Your internal order ID
  .digest();

const quote = {
  payer: payer.publicKey,
  merchant,
  token: PublicKey.default(), // Native SOL
  grossAmount,
  ipCreator,
  validUntil,
  orderIdHash,
  nonce: currentNonce,
  routeId: ROUTE_MERCHANT_AIFP1,
};

// ── 3. Compute digest & sign ──────────────────────────────────────────
// Borsh-serialize the quote (field order matters)
const quoteBytes = borshEncode(quote); // See Section 4

const digest = createHash("sha256")
  .update(MESSAGE_DOMAIN_TAG)
  .update(PROGRAM_ID.toBuffer())
  .update(quoteBytes)
  .digest();

const { signature: rawSig, recid } = secp256k1.ecdsaSign(
  new Uint8Array(digest),
  signerKey,
);
const signature = Buffer.alloc(65);
Buffer.from(rawSig).copy(signature);
signature[64] = 27 + recid;

// ── 4. Send settlement transaction ────────────────────────────────────
const [configPDA] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
const [profilesPDA] = PublicKey.findProgramAddressSync([PROFILES_INDEX_SEED], PROGRAM_ID);
const [consumedNoncePDA] = PublicKey.findProgramAddressSync(
  [CONSUMED_NONCE_SEED, payer.publicKey.toBuffer(), Buffer.alloc(8).writeBigUInt64LE(currentNonce)],
  PROGRAM_ID,
);

// Resolve treasury from on-chain profiles (or use global)
const treasury = /* ... */;

const tx = await program.methods
  .settleNative(
    new BN(currentNonce.toString()),
    {
      payer: payer.publicKey,
      merchant,
      token: PublicKey.default(),
      grossAmount: new BN(grossAmount.toString()),
      ipCreator,
      validUntil: new BN(validUntil.toString()),
      orderIdHash,
      nonce: new BN(currentNonce.toString()),
      routeId: Array.from(ROUTE_MERCHANT_AIFP1),
    },
    Array.from(signature),
  )
  .accounts({
    config: configPDA,
    payerNonce: noncePDA,
    consumedNonce: consumedNoncePDA,
    payer: payer.publicKey,
    merchant,
    treasury,
    ipCreator,
    profiles: profilesPDA,
    systemProgram: SystemProgram.programId,
  })
  .signers([payer])
  .rpc();

console.log(`Settlement TX: ${tx}`);
```

---

## 11. Error Handling

| Error Code | Name | Cause | Recovery |
|------------|------|-------|----------|
| 6000 | `Unauthorized` | Wrong signer for admin operation | Check keypair |
| 6004 | `UnsupportedToken` | Mint not in whitelist | Call `set_whitelisted_tokens` |
| 6013 | `InvalidSigner` | Recovered key doesn't match `config.signer` | Verify signing key |
| 6014 | `InvalidSignature` | Malformed or high-s signature | Use canonical low-s sig |
| 6016 | `SignatureExpired` | `quote.valid_until < clock.unix_timestamp` | Extend expiry |
| 6017 | `InvalidPayer` | `quote.payer != tx.signer` | payer must sign tx |
| 6019 | `InvalidNonce` | Nonce mismatch | Re-query `PayerNonce` PDA |
| 6020 | `NonceAlreadyConsumed` | Replayed nonce | Get fresh nonce |
| 6023 | `RouteDisabled` | Route is disabled | Admin must `enable_route` |
| 6024 | `UnknownRoute` | Route ID not in profiles | Admin must `configure_route` |
| 6038 | `ProtocolPaused` | Program is paused | Admin must `unpause` |
| 6042 | `DuplicateSettlementAccount` | merchant/treasury/ip_creator overlap | Use distinct wallets |

---

## 12. Cross-Chain Compatibility Notes

- Route IDs (`ROUTE_AGENT_X402`, `ROUTE_MERCHANT_AIFP1`) are keccak256 hashes of route names and MUST match EVM v1.4 exactly
- The Quote field order is identical to EVM v1.4
- The **digest construction differs**: Solana uses `SHA-256(domain_tag + program_id + Borsh)` while EVM uses `EIP-712 + keccak256`
- The secp256k1 key can be the same across chains, but the signature will differ due to different digests
- Fee caps are identical: `MAX_TREASURY_BPS=500`, `MAX_IP_CREATOR_BPS=100`, `MAX_AGGREGATE_BPS=1000`
