// Quote encoding + digest for the AiFinPay Splitter v1.4.
//
// TypeScript mirror of programs/splitter/src/utils.rs (`encode_quote`,
// `quote_message_hash`). The field order below is a CROSS-CHAIN SACRED
// CONSTANT (must match EVM v1.4 abi.encode(quote) and the Rust layout
// byte-for-byte). DO NOT REORDER. Any change is a coordinated upgrade
// (see .opencode/AGENTS.md) and must update the pinned fixture in
// programs/splitter/src/lib.rs (`digest_fixture_for_ts_parity`) and
// signer-parity.test.ts in lockstep.
//
// Layout (216 bytes, all integers little-endian):
//   payer(32) + merchant(32) + token(32) + gross_amount u64(8)
//   + ip_creator(32) + valid_until i64(8) + order_id_hash(32)
//   + nonce u64(8) + route_id(32)
//
// digest = SHA-256( domain_tag || program_id || encoded_quote )
// This is NOT EIP-712 and NOT personal_sign: the 32-byte digest is signed
// raw (see signer.ts).

import { createHash } from "crypto";

export const MESSAGE_DOMAIN_TAG = Buffer.from("AiFinPay-Solana-v1.4");
export const QUOTE_ENCODED_LEN = 216;

export interface Quote {
  payer: Uint8Array; // 32 bytes
  merchant: Uint8Array; // 32 bytes
  token: Uint8Array; // 32 bytes (zeros = native SOL)
  grossAmount: bigint; // u64
  ipCreator: Uint8Array; // 32 bytes
  validUntil: bigint; // i64 unix timestamp
  orderIdHash: Uint8Array; // 32 bytes
  nonce: bigint; // u64
  routeId: Uint8Array; // 32 bytes
}

function requireLen(name: string, v: Uint8Array, len: number): void {
  if (v.length !== len) throw new Error(`${name} must be ${len} bytes, got ${v.length}`);
}

export function encodeQuote(q: Quote): Buffer {
  requireLen("payer", q.payer, 32);
  requireLen("merchant", q.merchant, 32);
  requireLen("token", q.token, 32);
  requireLen("ipCreator", q.ipCreator, 32);
  requireLen("orderIdHash", q.orderIdHash, 32);
  requireLen("routeId", q.routeId, 32);

  const out = Buffer.alloc(QUOTE_ENCODED_LEN);
  let o = 0;
  Buffer.from(q.payer).copy(out, o); o += 32;
  Buffer.from(q.merchant).copy(out, o); o += 32;
  Buffer.from(q.token).copy(out, o); o += 32;
  out.writeBigUInt64LE(q.grossAmount, o); o += 8;
  Buffer.from(q.ipCreator).copy(out, o); o += 32;
  out.writeBigInt64LE(q.validUntil, o); o += 8;
  Buffer.from(q.orderIdHash).copy(out, o); o += 32;
  out.writeBigUInt64LE(q.nonce, o); o += 8;
  Buffer.from(q.routeId).copy(out, o); o += 32;
  if (o !== QUOTE_ENCODED_LEN) throw new Error("encodeQuote: internal length mismatch");

  return out;
}

export function computeDigest(programId: Uint8Array, q: Quote): Buffer {
  requireLen("programId", programId, 32);
  const encoded = encodeQuote(q);
  const h = createHash("sha256");
  h.update(MESSAGE_DOMAIN_TAG);
  h.update(Buffer.from(programId));
  h.update(encoded);
  return h.digest(); // 32 bytes
}
