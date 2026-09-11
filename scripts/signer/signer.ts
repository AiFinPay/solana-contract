// secp256k1 quote-signing backend (ports & adapters).
//
// The on-chain verifier (programs/splitter/src/utils.rs: `recover_signer`)
// accepts a 65-byte Ethereum-style signature r(32) || s(32) || v(1) with
// v = 27 + recovery_id and REJECTS high-s (EIP-2 non-canonical) signatures.
// This module is the off-chain counterpart: it produces exactly such
// signatures over the raw 32-byte quote digest (see quote.ts).
//
// Architecture: business logic depends only on the `SignerBackend` port.
// Adapters:
//   - LocalSignerBackend — raw private key in memory. DEV/TEST ONLY.
//     Never use for mainnet volume: prefer a KMS adapter.
//   - KMS (production) — implement `SignerBackend` on top of your KMS:
//     AWS KMS `Sign` with an ECC_SECG_P256K1 key, MessageType=DIGEST,
//     SigningAlgorithm=ECDSA_SHA_256 returns DER; parse it into r || s,
//     derive the recovery id by trial (0/1) against `GetPublicKey`
//     (uncompressed X || Y), and return r || s || (27 + recid).
//     The bulk-signing loop below works unchanged with any adapter,
//     which is how you sign N quotes with ONE Ledger-held admin key
//     authorizing the hot key via rotate-signer-role.ts.
//
// noble (@noble/curves v2) interop notes — read before touching:
//   1. `prehash: false` is MANDATORY. The v2 default re-hashes the input
//      (sign(SHA256(msg))), which would NOT recover on-chain. The digest
//      from quote.ts is signed raw, like k256 `sign_digest_prehash` and
//      BACKEND_INTEGRATION.md `ecdsaSign(digest, key)`.
//   2. v2 `sign()` returns 64-byte compact r || s WITHOUT a recovery bit.
//      The recovery id is found by trial (0, then 1) against the known
//      public key via `recoverPublicKey`.
//   3. noble v2 `recovered` format is recovery || r || s (recovery byte
//      FIRST — opposite of Ethereum r || s || v). The top-level
//      `recoverPublicKey` returns a COMPRESSED (33-byte) key; uncompress
//      via `Point.fromBytes(...).toBytes(false)` and slice off the 0x04
//      prefix to get the 64-byte X || Y on-chain `config.signer` format.

import { secp256k1 } from "@noble/curves/secp256k1.js";

// Half secp256k1 curve order N (big-endian). s > HALF_N is high-s.
// Byte-identical to utils.rs `HALF_N` (boundary s == HALF_N is allowed).
// Written as explicit bytes — not a hex string — so a dropped/duplicated
// nibble is impossible to miss in review.
const HALF_N = BigInt(
  "0x" +
    Buffer.from([
      0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
      0xff, 0xff, 0xff, 0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d,
      0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
    ]).toString("hex"),
);

function bytesToBigIntBE(b: Uint8Array): bigint {
  return BigInt("0x" + Buffer.from(b).toString("hex"));
}

export function isHighS(sig64: Uint8Array): boolean {
  if (sig64.length !== 64) throw new Error("signature must be 64 bytes (r || s)");
  return bytesToBigIntBE(sig64.subarray(32, 64)) > HALF_N;
}

/// Hot-key port. Implementations MUST return low-s signatures and MUST
/// never log or persist key material.
export interface SignerBackend {
  /** 64-byte uncompressed secp256k1 public key (X || Y), the on-chain format. */
  getPublicKey(): Uint8Array;
  /** Sign the raw 32-byte quote digest. Returns 65-byte r || s || v. */
  signDigest(digest: Uint8Array): Uint8Array;
}

/**
 * File/memory-backed secp256k1 key. DEV AND TEST ONLY — for mainnet
 * volume use a KMS adapter (see module header). Constructor warns loudly.
 */
export class LocalSignerBackend implements SignerBackend {
  private readonly privKey: Uint8Array;
  private readonly pubKey64: Uint8Array;

  constructor(privKey32: Uint8Array) {
    if (privKey32.length !== 32) throw new Error("private key must be 32 bytes");
    console.warn(
      "[signer] WARNING: LocalSignerBackend holds a raw private key in memory. " +
      "Dev/test only — use a KMS adapter for mainnet volume.",
    );
    this.privKey = Uint8Array.from(privKey32);
    this.pubKey64 = secp256k1.getPublicKey(this.privKey, false).slice(1);
  }

  getPublicKey(): Uint8Array {
    return Uint8Array.from(this.pubKey64);
  }

  signDigest(digest: Uint8Array): Uint8Array {
    if (digest.length !== 32) throw new Error("digest must be 32 bytes");
    // prehash:false — sign the digest RAW (see module header note 1).
    const compact = secp256k1.sign(digest, this.privKey, { lowS: true, prehash: false });
    return this.attachRecoveryId(digest, compact);
  }

  private attachRecoveryId(digest: Uint8Array, compact: Uint8Array): Uint8Array {
    for (const rec of [0, 1]) {
      // noble v2 `recovered` format: recovery || r || s (note 3).
      const recoveredInput = Buffer.concat([Buffer.from([rec]), Buffer.from(compact)]);
      const r33 = secp256k1.recoverPublicKey(recoveredInput, digest, { prehash: false });
      const r65 = secp256k1.Point.fromBytes(r33).toBytes(false);
      if (Buffer.from(r65.slice(1)).equals(Buffer.from(this.pubKey64))) {
        return Buffer.concat([Buffer.from(compact), Buffer.from([27 + rec])]);
      }
    }
    throw new Error("signDigest: recovery id not found (unreachable for a valid key)");
  }
}

/**
 * Parse/verify a (digest, signature) pair against the claimed 64-byte
 * signer pubkey. Returns the signer on success, throws otherwise.
 * Mirrors on-chain `recover_signer` + `recovered == config.signer`:
 * rejects bad v and high-s BEFORE recovery.
 */
export function recoverSigner(
  digest: Uint8Array,
  signature65: Uint8Array,
  claimedSigner64: Uint8Array,
): Uint8Array {
  if (digest.length !== 32) throw new Error("digest must be 32 bytes");
  if (signature65.length !== 65) throw new Error("signature must be 65 bytes (r || s || v)");
  if (claimedSigner64.length !== 64) throw new Error("signer pubkey must be 64 bytes (X || Y)");

  const v = signature65[64];
  const rec = v - 27;
  if (rec !== 0 && rec !== 1) throw new Error(`invalid recovery id v=${v} (want 27 or 28)`);

  const compact = signature65.subarray(0, 64);
  if (isHighS(compact)) throw new Error("non-canonical signature (high-s)");

  const recoveredInput = Buffer.concat([Buffer.from([rec]), Buffer.from(compact)]);
  const r33 = secp256k1.recoverPublicKey(recoveredInput, digest, { prehash: false });
  const r65 = secp256k1.Point.fromBytes(r33).toBytes(false);
  const recovered64 = Uint8Array.from(r65.slice(1));

  if (!Buffer.from(recovered64).equals(Buffer.from(claimedSigner64))) {
    throw new Error("signature does not recover to the claimed signer");
  }
  return recovered64;
}

/** Sign one quote digest through any backend (the bulk loop calls this N times). */
export function signQuoteDigest(backend: SignerBackend, digest: Uint8Array): Uint8Array {
  return backend.signDigest(digest);
}
