// Parity tests: TypeScript signer vs the on-chain Rust implementation.
//
// 1. `computeDigest` must equal the PINNED fixture from
//    programs/splitter/src/lib.rs (`digest_fixture_for_ts_parity`).
//    If either side changes, both must be updated in lockstep.
// 2. `LocalSignerBackend` signatures must round-trip through
//    `recoverSigner` (the off-chain mirror of on-chain `recover_signer`),
//    with v in {27, 28} and low-s enforced.
// 3. Tampered digests and high-s signatures must be rejected.
//
// Run: pnpm test:signer
// (mocha over the compiled output; no network, no key files.)

import assert from "assert";
import { PublicKey } from "@solana/web3.js";
import { computeDigest, encodeQuote, QUOTE_ENCODED_LEN, type Quote } from "./quote";
import { LocalSignerBackend, recoverSigner, isHighS } from "./signer";

const PROGRAM_ID = new PublicKey("5QBJgMap7wuFsYfaU8Pmuu2i96GsJ3aBv6mMoUSaPoiS");

// Byte-identical to the Rust fixture in digest_fixture_for_ts_parity.
function fixtureQuote(): Quote {
  return {
    payer: new Uint8Array(32).fill(1),
    merchant: new Uint8Array(32).fill(2),
    token: new Uint8Array(32),
    grossAmount: 1_000_000n,
    ipCreator: new Uint8Array(32),
    validUntil: 1_700_003_600n,
    orderIdHash: new Uint8Array(32).fill(7),
    nonce: 3n,
    routeId: Uint8Array.from([
      0xb9, 0xdb, 0xf5, 0x87, 0xb0, 0xdf, 0x69, 0x87, 0x0d, 0xf1, 0xe6, 0x0b,
      0x22, 0xfb, 0xa0, 0x31, 0x7f, 0x53, 0xeb, 0x19, 0xd7, 0x8a, 0x57, 0x3a,
      0xbf, 0x94, 0xfc, 0x38, 0x4a, 0x33, 0x9a, 0x89,
    ]),
  };
}

const PINNED_DIGEST_HEX =
  "611e97c5fa10b9adeb9f717837a715b9c281d6341d34a0a34facc9a33b71ff6b";

describe("signer parity (TS vs Rust)", () => {
  it("encodes the quote to 216 bytes", () => {
    assert.strictEqual(encodeQuote(fixtureQuote()).length, QUOTE_ENCODED_LEN);
  });

  it("computeDigest equals the pinned Rust fixture", () => {
    const digest = computeDigest(
      new Uint8Array(new PublicKey(PROGRAM_ID).toBuffer()),
      fixtureQuote(),
    );
    assert.strictEqual(Buffer.from(digest).toString("hex"), PINNED_DIGEST_HEX);
  });

  it("sign -> recover round-trips with v in {27,28} and low-s", () => {
    const backend = new LocalSignerBackend(new Uint8Array(32).fill(7));
    const digest = computeDigest(
      new Uint8Array(new PublicKey(PROGRAM_ID).toBuffer()),
      fixtureQuote(),
    );
    const sig = backend.signDigest(new Uint8Array(digest));
    assert.strictEqual(sig.length, 65);
    assert.ok(sig[64] === 27 || sig[64] === 28, `v=${sig[64]}`);
    assert.ok(!isHighS(sig.subarray(0, 64)), "signature must be low-s");

    const recovered = recoverSigner(
      new Uint8Array(digest),
      sig,
      backend.getPublicKey(),
    );
    assert.ok(Buffer.from(recovered).equals(Buffer.from(backend.getPublicKey())));
  });

  it("rejects a tampered digest", () => {
    const backend = new LocalSignerBackend(new Uint8Array(32).fill(7));
    const digest = computeDigest(
      new Uint8Array(new PublicKey(PROGRAM_ID).toBuffer()),
      fixtureQuote(),
    );
    const sig = backend.signDigest(new Uint8Array(digest));
    const tampered = Uint8Array.from(digest);
    tampered[0] ^= 0xff;
    assert.throws(() => recoverSigner(tampered, sig, backend.getPublicKey()));
  });

  it("rejects high-s signatures", () => {
    const backend = new LocalSignerBackend(new Uint8Array(32).fill(7));
    const digest = computeDigest(
      new Uint8Array(new PublicKey(PROGRAM_ID).toBuffer()),
      fixtureQuote(),
    );
    const sig = Uint8Array.from(backend.signDigest(new Uint8Array(digest)));
    // Flip s -> N - s: still a valid ECDSA sig, but non-canonical (high-s).
    const N = BigInt(
      "0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141",
    );
    const s = BigInt("0x" + Buffer.from(sig.subarray(32, 64)).toString("hex"));
    const sHigh = (N - s).toString(16).padStart(64, "0");
    sig.set(Uint8Array.from(Buffer.from(sHigh, "hex")), 32);
    assert.ok(isHighS(sig.subarray(0, 64)));
    assert.throws(() => recoverSigner(new Uint8Array(digest), sig, backend.getPublicKey()));
  });

  it("rejects bad recovery ids", () => {
    const backend = new LocalSignerBackend(new Uint8Array(32).fill(7));
    const digest = computeDigest(
      new Uint8Array(new PublicKey(PROGRAM_ID).toBuffer()),
      fixtureQuote(),
    );
    const sig = Uint8Array.from(backend.signDigest(new Uint8Array(digest)));
    sig[64] = 30;
    assert.throws(() => recoverSigner(new Uint8Array(digest), sig, backend.getPublicKey()));
  });
});
