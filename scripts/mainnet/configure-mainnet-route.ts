#!/usr/bin/env node

// Mainnet `configure_route` for the canonical splitter program.
// Mirror of scripts/devnet/configure-route.ts pointed at mainnet-beta
// and the canonical program ID.
//
// WARNING: sends real mainnet transactions signed by the admin keypair.
// ADMIN_KEYPAIR_PATH is required (no defaults on purpose — double-check
// which admin key you point at before running).

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
  type TransactionSigner,
} from "@solana/kit";
import { PublicKey } from "@solana/web3.js";
import * as fs from "fs";
import * as path from "path";

// ---------------------------------------------------------------------------
// Load .env / .env.local (no dotenv dependency)
// ---------------------------------------------------------------------------
function loadEnvFile(envPath: string) {
  if (!fs.existsSync(envPath)) return;
  const content = fs.readFileSync(envPath, "utf-8");
  content.split("\n").forEach(line => {
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith("#")) {
      const [key, ...valueParts] = trimmed.split("=");
      if (key && valueParts.length > 0) {
        process.env[key.trim()] = valueParts.join("=").trim();
      }
    }
  });
}

const ROOT = path.join(__dirname, "..", "..");
loadEnvFile(path.join(ROOT, ".env"));
loadEnvFile(path.join(ROOT, ".env.local"));

// ---------------------------------------------------------------------------
// Constants (must match programs/splitter/src/constants.rs; canonical mainnet ID)
// ---------------------------------------------------------------------------
const PROGRAM_ID = address("DPFAmcgGe7ZaLCRQAZ24Z9SJ8s5gaBWHHbKjLNWsWWBS");
const CONFIG_SEED = new TextEncoder().encode("config");
const PROFILES_INDEX_SEED = new TextEncoder().encode("profiles-index");

const ROUTE_AGENT_X402 = new Uint8Array([
  0x8d, 0xc5, 0x05, 0xbe, 0x33, 0x5e, 0x56, 0x5d, 0x2a, 0x5e, 0x2c, 0x96, 0x05, 0x7c, 0x7f, 0xb0,
  0xca, 0xff, 0x7c, 0x50, 0x09, 0xf6, 0x1b, 0x82, 0xac, 0x3e, 0xf5, 0xe7, 0xa9, 0xec, 0x0f, 0x1e,
]);

const ROUTE_MERCHANT_AIFP1 = new Uint8Array([
  0xb9, 0xdb, 0xf5, 0x87, 0xb0, 0xdf, 0x69, 0x87, 0x0d, 0xf1, 0xe6, 0x0b, 0x22, 0xfb, 0xa0, 0x31,
  0x7f, 0x53, 0xeb, 0x19, 0xd7, 0x8a, 0x57, 0x3a, 0xbf, 0x94, 0xfc, 0x38, 0x4a, 0x33, 0x9a, 0x89,
]);

// Anchor instruction discriminator for "global:configure_route"
const CONFIGURE_ROUTE_DISCRIMINATOR = new Uint8Array([43, 191, 221, 37, 219, 56, 2, 255]);

// ---------------------------------------------------------------------------
// Borsh encoding helpers (manual, no borsh lib dependency)
// ---------------------------------------------------------------------------
function encodeConfigureRoute(
  routeId: Uint8Array,
  treasuryBps: number,
  ipCreatorBps: number,
  routeTreasury: Uint8Array,
): Buffer {
  const buf = Buffer.alloc(8 + 32 + 2 + 2 + 32);
  let offset = 0;

  // discriminator
  Buffer.from(CONFIGURE_ROUTE_DISCRIMINATOR).copy(buf, offset);
  offset += 8;

  // route_id: [u8; 32]
  Buffer.from(routeId).copy(buf, offset);
  offset += 32;

  // treasury_bps: u16 LE
  buf.writeUInt16LE(treasuryBps, offset);
  offset += 2;

  // ip_creator_bps: u16 LE
  buf.writeUInt16LE(ipCreatorBps, offset);
  offset += 2;

  // route_treasury: Pubkey (32 bytes)
  Buffer.from(routeTreasury).copy(buf, offset);
  offset += 32;

  return buf.subarray(0, offset);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  console.log("=== AiFinPay Splitter v1.4 — Configure Route (MAINNET) ===\n");
  console.log("WARNING: this sends REAL mainnet transactions signed by admin.\n");

  // --- Load admin keypair (configure_route requires admin signer) ---
  const adminKeypairPath = process.env.ADMIN_KEYPAIR_PATH;
  if (!adminKeypairPath) {
    throw new Error(
      "Missing env var: ADMIN_KEYPAIR_PATH. Point it at the file keypair of the " +
      "on-chain admin set during initialize."
    );
  }
  const resolvedKeypairPath = path.resolve(adminKeypairPath);
  console.log(`Admin keypair: ${resolvedKeypairPath}`);
  if (!fs.existsSync(resolvedKeypairPath)) {
    throw new Error(`Keypair file not found: ${resolvedKeypairPath}`);
  }
  const secret = JSON.parse(fs.readFileSync(resolvedKeypairPath, "utf-8"));
  const signer = await createKeyPairSignerFromBytes(new Uint8Array(secret));
  console.log(`Admin pubkey:  ${signer.address}\n`);

  // --- RPC ---
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const wsUrl = rpcUrl.replace("https://", "wss://").replace("http://", "ws://");
  const rpc = createSolanaRpc(rpcUrl);
  const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl);

  // --- Balance ---
  const { value: balance } = await rpc.getBalance(signer.address).send();
  const solBalance = Number(balance) / 1_000_000_000;
  console.log(`Balance: ${solBalance} SOL\n`);

  // --- Derive PDAs ---
  const [configPDA] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: [CONFIG_SEED],
  });
  const [profilesIndexPDA] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: [PROFILES_INDEX_SEED],
  });

  console.log("PDAs:");
  console.log(`  Config:        ${configPDA}`);
  console.log(`  ProfilesIndex: ${profilesIndexPDA}\n`);

  // --- Define route updates ---
  const TREASURY_PUBKEY = process.env.TREASURY_PUBKEY;
  if (!TREASURY_PUBKEY) throw new Error("Missing env var: TREASURY_PUBKEY");
  const routeTreasuryBytes = new Uint8Array(new PublicKey(TREASURY_PUBKEY).toBuffer());

  const routes = [
    { name: "AGENT_X402", id: ROUTE_AGENT_X402, treasuryBps: 0, ipCreatorBps: 0 },
    { name: "MERCHANT_AIFP1", id: ROUTE_MERCHANT_AIFP1, treasuryBps: 100, ipCreatorBps: 0 },
  ];

  for (const route of routes) {
    console.log(`Configuring ${route.name}: treasury=${route.treasuryBps} ip_creator=${route.ipCreatorBps}`);

    const instructionData = encodeConfigureRoute(
      route.id,
      route.treasuryBps,
      route.ipCreatorBps,
      routeTreasuryBytes,
    );

    const configureInstruction: Instruction = {
      programAddress: PROGRAM_ID,
      accounts: [
        { address: configPDA, role: AccountRole.WRITABLE },
        { address: profilesIndexPDA, role: AccountRole.WRITABLE },
        { address: signer.address, role: AccountRole.WRITABLE_SIGNER },
      ],
      data: new Uint8Array(instructionData),
    };

    const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

    const transactionMessage = pipe(
      createTransactionMessage({ version: 0 }),
      (tx) => setTransactionMessageFeePayerSigner(signer, tx),
      (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
      (tx) => appendTransactionMessageInstruction(configureInstruction, tx),
    );

    const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
    const txSignature = getSignatureFromTransaction(signedTransaction);

    const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
    await sendAndConfirm(signedTransaction as any, { commitment: "confirmed" });

    console.log(`  ✅ ${txSignature}`);
    console.log(`  Explorer: https://explorer.solana.com/tx/${txSignature}\n`);
  }

  console.log("Done! Run 'npm run check:mainnet' to verify.");
}

main().catch(err => {
  console.error("Fatal error:", err);
  process.exit(1);
});
