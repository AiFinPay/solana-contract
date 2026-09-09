#!/usr/bin/env node

// Rotate (or grant) the secp256k1 hot signer key of the canonical splitter.
//
// This is the "ONE Ledger signature instead of N" step: the cold admin key
// (ideally a Ledger-held multisig) authorizes a hot operational key in a
// single transaction; the hot key then signs quote digests in bulk through
// any SignerBackend (see signer.ts).
//
// Usage:
//   # 1. Generate a fresh hot key (writes 0600 file, never overwrites):
//   pnpm rotate:signer -- --generate ./hot-signer.hex
//   # 2. Rotate the on-chain signer to it (admin signs, confirmation required):
//   ADMIN_KEYPAIR_PATH=keypairs/admin.json pnpm rotate:signer -- \
//     --pubkey $(cat ./hot-signer.pub) --instruction rotate --yes
//   # Or grant for the first time:
//   ... --instruction grant
//
// Env: ADMIN_KEYPAIR_PATH (required), SOLANA_RPC_URL (default mainnet-beta).
//
// Ledger note: this script signs with a FILE admin keypair. If admin lives
// on a Ledger, do NOT export it — build the identical instruction payload
// (discriminator + 64-byte pubkey, accounts [config, admin]) with a
// Ledger-capable client instead. The payload construction below is kept
// explicit so it can be reproduced 1:1 elsewhere.

import {
  address,
  AccountRole,
  createSolanaRpc,
  createSolanaRpcSubscriptions,
  createKeyPairSignerFromBytes,
  getProgramDerivedAddress,
  appendTransactionMessageInstruction,
  createTransactionMessage,
  pipe,
  setTransactionMessageFeePayerSigner,
  setTransactionMessageLifetimeUsingBlockhash,
  signTransactionMessageWithSigners,
  sendAndConfirmTransactionFactory,
  getSignatureFromTransaction,
  type Instruction,
} from "@solana/kit";
import { PublicKey } from "@solana/web3.js";
import { randomBytes } from "crypto";
import * as fs from "fs";
import * as path from "path";
import * as readline from "readline";
import { secp256k1 } from "@noble/curves/secp256k1.js";

// ---------------------------------------------------------------------------
// Load cluster env file + .env.local override (no dotenv dependency)
// ---------------------------------------------------------------------------
function loadEnvFile(envPath: string, overwrite = false) {
  if (!fs.existsSync(envPath)) return;
  const content = fs.readFileSync(envPath, "utf-8");
  content.split("\n").forEach(line => {
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith("#")) {
      const [key, ...valueParts] = trimmed.split("=");
      if (key && valueParts.length > 0) {
        if (overwrite || process.env[key.trim()] === undefined) {
          process.env[key.trim()] = valueParts.join("=").trim();
        }
      }
    }
  });
}

const ROOT = path.join(__dirname, "..", "..");
loadEnvFile(path.join(ROOT, ".env.production"));
loadEnvFile(path.join(ROOT, ".env.local"), true);

// ---------------------------------------------------------------------------
// Constants (must match programs/splitter/src/{constants.rs,lib.rs})
// ---------------------------------------------------------------------------
const PROGRAM_ID = address("DPFAmcgGe7ZaLCRQAZ24Z9SJ8s5gaBWHHbKjLNWsWWBS");
const CONFIG_SEED = new TextEncoder().encode("config");

// Anchor discriminators: sha256("global:<name>")[0..8].
// grant:  sha256("global:grant_signer_role")[0..8]
// rotate: sha256("global:rotate_signer_role")[0..8]
const GRANT_SIGNER_ROLE_DISCRIMINATOR = new Uint8Array([228, 184, 14, 183, 65, 234, 104, 55]);
const ROTATE_SIGNER_ROLE_DISCRIMINATOR = new Uint8Array([221, 87, 44, 133, 28, 108, 195, 255]);

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------
function argValue(flag: string): string | undefined {
  const i = process.argv.indexOf(flag);
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : undefined;
}
const hasFlag = (flag: string) => process.argv.includes(flag);

function parsePubkeyHex(hex: string): Uint8Array {
  const clean = hex.trim().toLowerCase().replace(/^0x/, "");
  if (!/^[0-9a-f]{128}$/.test(clean)) {
    throw new Error("--pubkey must be 128 hex chars (64-byte uncompressed X || Y)");
  }
  const bytes = Uint8Array.from(Buffer.from(clean, "hex"));
  if (bytes.every(b => b === 0)) throw new Error("refusing zero signer pubkey");
  return bytes;
}

async function confirm(prompt: string): Promise<boolean> {
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  try {
    const answer = await new Promise<string>(resolve => rl.question(prompt, resolve));
    return answer.trim().toLowerCase() === "y";
  } finally {
    rl.close();
  }
}

// ---------------------------------------------------------------------------
// --generate: fresh hot secp256k1 key, 0600 file, never overwrites
// ---------------------------------------------------------------------------
async function generateHotKey(outPath: string) {
  const resolved = path.resolve(outPath);
  if (fs.existsSync(resolved)) {
    throw new Error(`refusing to overwrite existing file: ${resolved}`);
  }
  let privKey = randomBytes(32);
  // Rejection-sample until noble accepts it as a scalar (negligible loop).
  // eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      secp256k1.getPublicKey(privKey, false);
      break;
    } catch {
      privKey = randomBytes(32);
    }
  }
  fs.writeFileSync(resolved, Buffer.from(privKey).toString("hex") + "\n", { mode: 0o600 });
  const pub64 = Buffer.from(secp256k1.getPublicKey(privKey, false).slice(1)).toString("hex");
  console.log(`hot private key (0600): ${resolved}`);
  console.log(`hot pubkey (X || Y hex): ${pub64}`);
  console.log("Store the key in your KMS/secrets manager; delete the file after import.");
}

// ---------------------------------------------------------------------------
// Main: build + send grant/rotate instruction, verify on-chain
// ---------------------------------------------------------------------------
async function main() {
  const generatePath = argValue("--generate");
  if (generatePath) {
    await generateHotKey(generatePath);
    return;
  }

  const pubkeyHex = argValue("--pubkey");
  if (!pubkeyHex) {
    throw new Error("Missing --pubkey <128-hex> (or use --generate <path> first)");
  }
  const newSigner = parsePubkeyHex(pubkeyHex);

  const instruction = argValue("--instruction") || "rotate";
  if (instruction !== "grant" && instruction !== "rotate") {
    throw new Error(`--instruction must be "grant" or "rotate", got "${instruction}"`);
  }
  const discriminator =
    instruction === "grant" ? GRANT_SIGNER_ROLE_DISCRIMINATOR : ROTATE_SIGNER_ROLE_DISCRIMINATOR;

  console.log(`=== AiFinPay Splitter v1.4 — ${instruction}_signer_role ===\n`);

  const adminKeypairPath = process.env.ADMIN_KEYPAIR_PATH;
  if (!adminKeypairPath) {
    throw new Error(
      "Missing env var: ADMIN_KEYPAIR_PATH. Point it at the file keypair of the " +
      "on-chain admin. If admin lives on a Ledger, do NOT export it — see header.",
    );
  }
  const resolvedKeypairPath = path.resolve(adminKeypairPath);
  if (!fs.existsSync(resolvedKeypairPath)) {
    throw new Error(`Keypair file not found: ${resolvedKeypairPath}`);
  }
  const secret = JSON.parse(fs.readFileSync(resolvedKeypairPath, "utf-8"));
  const admin = await createKeyPairSignerFromBytes(new Uint8Array(secret));
  console.log(`Admin pubkey:  ${admin.address}`);

  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  console.log(`RPC:           ${rpcUrl}`);
  const wsUrl = rpcUrl.replace("https://", "wss://").replace("http://", "ws://");
  const rpc = createSolanaRpc(rpcUrl);
  const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl);

  const [configPDA] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: [CONFIG_SEED],
  });
  console.log(`Config PDA:    ${configPDA}`);
  console.log(`New signer:    ${Buffer.from(newSigner).toString("hex")}\n`);

  if (!hasFlag("--yes")) {
    const ok = await confirm(`Send ${instruction}_signer_role? [y/N] `);
    if (!ok) {
      console.log("Aborted.");
      return;
    }
  }

  const instructionData = Buffer.concat([
    Buffer.from(discriminator),
    Buffer.from(newSigner),
  ]);

  const rotateInstruction: Instruction = {
    programAddress: PROGRAM_ID,
    accounts: [
      { address: configPDA, role: AccountRole.WRITABLE },
      { address: admin.address, role: AccountRole.WRITABLE_SIGNER },
    ],
    data: new Uint8Array(instructionData),
  };

  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();
  const transactionMessage = pipe(
    createTransactionMessage({ version: 0 }),
    (tx) => setTransactionMessageFeePayerSigner(admin, tx),
    (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    (tx) => appendTransactionMessageInstruction(rotateInstruction, tx),
  );

  const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
  const txSignature = getSignatureFromTransaction(signedTransaction);

  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTransaction as any, { commitment: "confirmed" });

  console.log(`Transaction: ${txSignature}`);

  // --- Verify: read config.signer back (admin 32B @8, signer 64B @40) ---
  const { value: configAccount } = await rpc
    .getAccountInfo(configPDA, { encoding: "base64" })
    .send();
  if (!configAccount) throw new Error("config account not found after rotation");
  const raw = Buffer.from((configAccount.data as unknown as [string, string])[0], "base64");
  const onChainSigner = raw.subarray(8 + 32, 8 + 32 + 64);
  const match = onChainSigner.equals(Buffer.from(newSigner));
  console.log(`On-chain signer matches: ${match ? "YES" : "NO **** MISMATCH ****"}`);
  if (!match) {
    console.log(`  on-chain: ${onChainSigner.toString("hex")}`);
    process.exit(1);
  }
  // Cross-check the admin printed above really is the on-chain admin.
  const onChainAdmin = new PublicKey(raw.subarray(8, 40)).toBase58();
  console.log(`On-chain admin: ${onChainAdmin} (you signed as ${admin.address})`);
}

main().catch(err => {
  console.error("Fatal error:", err);
  process.exit(1);
});
