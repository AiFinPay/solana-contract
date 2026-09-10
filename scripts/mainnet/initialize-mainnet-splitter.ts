#!/usr/bin/env node

// Mainnet `initialize` for the canonical splitter program.
// Mirror of scripts/devnet/initialize-devnet-splitter.ts pointed at
// mainnet-beta and the canonical program ID.
//
// WARNING: sends a real mainnet transaction. The payer MUST be the on-chain
// DEPLOYER baked into the binary (see programs/splitter/src/constants.rs).
//
// Ledger note: this script signs with a file keypair from
// DEPLOYER_KEYPAIR_PATH. If DEPLOYER lives on a Ledger, this script CANNOT
// sign for it — build the same instruction with a Ledger-capable client
// instead (see scripts/mainnet/README.md) and do NOT export Ledger keys
// to a file.

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
import {
  struct,
  u8,
  u16,
  publicKey,
  vec,
  array,
  bool,
} from "@coral-xyz/borsh";
import * as fs from "fs";
import * as path from "path";

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
// Constants (must match programs/splitter/src/constants.rs; canonical mainnet ID)
// ---------------------------------------------------------------------------
const PROGRAM_ID = address("5QBJgMap7wuFsYfaU8Pmuu2i96GsJ3aBv6mMoUSaPoiS");
const CONFIG_SEED = new TextEncoder().encode("config");
const TOKEN_LIST_SEED = new TextEncoder().encode("token-list");
const PROFILES_INDEX_SEED = new TextEncoder().encode("profiles-index");

const ROUTE_AGENT_X402 = new Uint8Array([
  0x8d, 0xc5, 0x05, 0xbe, 0x33, 0x5e, 0x56, 0x5d, 0x2a, 0x5e, 0x2c, 0x96, 0x05, 0x7c, 0x7f, 0xb0,
  0xca, 0xff, 0x7c, 0x50, 0x09, 0xf6, 0x1b, 0x82, 0xac, 0x3e, 0xf5, 0xe7, 0xa9, 0xec, 0x0f, 0x1e,
]);

const ROUTE_MERCHANT_AIFP1 = new Uint8Array([
  0xb9, 0xdb, 0xf5, 0x87, 0xb0, 0xdf, 0x69, 0x87, 0x0d, 0xf1, 0xe6, 0x0b, 0x22, 0xfb, 0xa0, 0x31,
  0x7f, 0x53, 0xeb, 0x19, 0xd7, 0x8a, 0x57, 0x3a, 0xbf, 0x94, 0xfc, 0x38, 0x4a, 0x33, 0x9a, 0x89,
]);

// Anchor instruction discriminator for "global:initialize"
const INITIALIZE_DISCRIMINATOR = new Uint8Array([175, 175, 109, 31, 13, 152, 155, 237]);

// System program
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");

// ---------------------------------------------------------------------------
// Borsh schema — must match InitializeParams in initialize.rs exactly
// ---------------------------------------------------------------------------
const InitializeParamsLayout = struct([
  publicKey("admin"),
  array(u8(), 64, "signer"),
  publicKey("pauser"),
  publicKey("treasury"),
  vec(publicKey(), "stablecoins"),
  vec(array(u8(), 32), "route_ids"),
  vec(u16(), "treasury_bps"),
  vec(u16(), "ip_creator_bps"),
]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function parsePubkey(name: string, value: string | undefined): string {
  if (!value) throw new Error(`Missing env var: ${name}`);
  try {
    address(value);
    return value;
  } catch {
    throw new Error(`Invalid pubkey for ${name}: ${value}`);
  }
}

function parseHexBytes64(name: string, value: string | undefined): number[] {
  if (!value) throw new Error(`Missing env var: ${name}`);
  const clean = value.replace(/^0x/, "");
  if (clean.length !== 128) {
    throw new Error(`${name} must be 64 bytes (128 hex chars), got ${clean.length}`);
  }
  const bytes: number[] = [];
  for (let i = 0; i < 64; i++) {
    bytes.push(parseInt(clean.slice(i * 2, i * 2 + 2), 16));
  }
  return bytes;
}

function parseU16(name: string, value: string | undefined): number {
  if (!value) throw new Error(`Missing env var: ${name}`);
  const parsed = parseInt(value, 10);
  if (isNaN(parsed) || parsed < 0 || parsed > 65535) {
    throw new Error(`${name} must be a u16 (0-65535), got ${value}`);
  }
  return parsed;
}

function parsePubkeyArray(name: string, value: string | undefined): string[] {
  if (!value || value.trim() === "") return [];
  return value.split(",").map(s => parsePubkey(name, s.trim()));
}

function parseU16Array(name: string, value: string | undefined): number[] {
  if (!value || value.trim() === "") return [];
  return value.split(",").map(s => parseU16(name, s.trim()));
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------
async function main() {
  console.log("=== AiFinPay Splitter v1.4 — Initialize MAINNET (borsh) ===\n");
  console.log("WARNING: this sends a REAL mainnet transaction paid by DEPLOYER.\n");

  // --- Parse env vars (no devnet defaults on purpose) ---
  const deployerKeypairPath = process.env.DEPLOYER_KEYPAIR_PATH;
  if (!deployerKeypairPath) {
    throw new Error(
      "Missing env var: DEPLOYER_KEYPAIR_PATH. Point it at the file keypair of the " +
      "on-chain DEPLOYER. If DEPLOYER lives on a Ledger, stop here — this script " +
      "cannot sign with hardware wallets; see scripts/mainnet/README.md."
    );
  }
  const admin = parsePubkey("ADMIN_PUBKEY", process.env.ADMIN_PUBKEY);
  const signerBytes = parseHexBytes64("SIGNER_PUBKEY", process.env.SIGNER_PUBKEY);
  const pauser = parsePubkey("PAUSER_PUBKEY", process.env.PAUSER_PUBKEY);
  const treasury = parsePubkey("TREASURY_PUBKEY", process.env.TREASURY_PUBKEY);
  const stablecoins = parsePubkeyArray("STABLECOINS", process.env.STABLECOINS);

  const routeIdsStr = process.env.ROUTE_IDS || "AGENT_X402,MERCHANT_AIFP1";
  const routeIds = routeIdsStr.split(",").map(s => {
    const trimmed = s.trim().toUpperCase();
    if (trimmed === "AGENT_X402") return ROUTE_AGENT_X402;
    if (trimmed === "MERCHANT_AIFP1") return ROUTE_MERCHANT_AIFP1;
    throw new Error(`Unknown route ID: ${s}. Valid: AGENT_X402, MERCHANT_AIFP1`);
  });

  const treasuryBps = parseU16Array("TREASURY_BPS", process.env.TREASURY_BPS);
  const ipCreatorBps = parseU16Array("IP_CREATOR_BPS", process.env.IP_CREATOR_BPS);

  if (routeIds.length === 0) throw new Error("At least one route ID is required");
  if (routeIds.length !== treasuryBps.length || routeIds.length !== ipCreatorBps.length) {
    throw new Error("ROUTE_IDS, TREASURY_BPS, and IP_CREATOR_BPS must have the same length");
  }

  // --- Load deployer keypair ---
  const resolvedKeypairPath = path.resolve(deployerKeypairPath);
  console.log(`Deployer keypair: ${resolvedKeypairPath}`);
  if (!fs.existsSync(resolvedKeypairPath)) {
    throw new Error(`Keypair file not found: ${resolvedKeypairPath}`);
  }
  const secret = JSON.parse(fs.readFileSync(resolvedKeypairPath, "utf-8"));
  const signer = await createKeyPairSignerFromBytes(new Uint8Array(secret));
  console.log(`Deployer pubkey:  ${signer.address}\n`);

  // --- RPC ---
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
  const wsUrl = rpcUrl.replace("https://", "wss://").replace("http://", "ws://");
  const rpc = createSolanaRpc(rpcUrl);
  const rpcSubscriptions = createSolanaRpcSubscriptions(wsUrl);

  // --- Balance ---
  const { value: balance } = await rpc.getBalance(signer.address).send();
  const solBalance = Number(balance) / 1_000_000_000;
  console.log(`Balance: ${solBalance} SOL`);
  if (solBalance < 0.5) {
    console.warn("Low balance — you may need more SOL for rent exemption.\n");
  }

  // --- Derive PDAs ---
  const [configPDA] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: [CONFIG_SEED],
  });
  const [tokenListPDA] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: [TOKEN_LIST_SEED],
  });
  const [profilesIndexPDA] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: [PROFILES_INDEX_SEED],
  });

  console.log("PDAs:");
  console.log(`  Config:        ${configPDA}`);
  console.log(`  TokenList:     ${tokenListPDA}`);
  console.log(`  ProfilesIndex: ${profilesIndexPDA}\n`);

  // --- Check if already initialized ---
  const { value: existingConfig } = await rpc.getAccountInfo(configPDA, { encoding: "base64" }).send();
  if (existingConfig) {
    console.log("Config PDA already exists — program may be initialized.");
    const readline = require("readline").createInterface({ input: process.stdin, output: process.stdout });
    const answer = await new Promise<string>(resolve =>
      readline.question("Continue anyway? (y/N): ", (ans: string) => { readline.close(); resolve(ans); })
    );
    if (answer.toLowerCase() !== "y") {
      console.log("Aborted.");
      process.exit(0);
    }
  }

  // --- Borsh-serialize InitializeParams ---
  const params = {
    admin: new PublicKey(admin),
    signer: signerBytes,
    pauser: new PublicKey(pauser),
    treasury: new PublicKey(treasury),
    stablecoins: stablecoins.map(s => new PublicKey(s)),
    route_ids: routeIds.map(r => Array.from(r)),
    treasury_bps: treasuryBps,
    ip_creator_bps: ipCreatorBps,
  };

  const maxParamsSize = 4 + 64 + 4 + 32 * 3 + 4 + 32 * 2 + 4 + 2 * 2 + 4 + 32 + 200;
  const paramsBuffer = Buffer.alloc(maxParamsSize);
  const written = InitializeParamsLayout.encode(params, paramsBuffer);
  const paramsSlice = paramsBuffer.subarray(0, written);

  const instructionData = Buffer.concat([Buffer.from(INITIALIZE_DISCRIMINATOR), paramsSlice]);
  console.log(`Instruction data length: ${instructionData.length} bytes`);
  console.log(`Instruction data hex: ${instructionData.toString("hex").slice(0, 80)}...`);

  // --- Build instruction ---
  const initInstruction: Instruction = {
    programAddress: PROGRAM_ID,
    accounts: [
      { address: signer.address, role: AccountRole.WRITABLE_SIGNER },
      { address: configPDA, role: AccountRole.WRITABLE },
      { address: tokenListPDA, role: AccountRole.WRITABLE },
      { address: profilesIndexPDA, role: AccountRole.WRITABLE },
      { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
    ],
    data: new Uint8Array(instructionData),
  };

  // --- Build and send transaction ---
  console.log("Sending initialize transaction...");

  const { value: latestBlockhash } = await rpc.getLatestBlockhash().send();

  const transactionMessage = pipe(
    createTransactionMessage({ version: 0 }),
    (tx) => setTransactionMessageFeePayerSigner(signer, tx),
    (tx) => setTransactionMessageLifetimeUsingBlockhash(latestBlockhash, tx),
    (tx) => appendTransactionMessageInstruction(initInstruction, tx),
  );

  const signedTransaction = await signTransactionMessageWithSigners(transactionMessage);
  const txSignature = getSignatureFromTransaction(signedTransaction);

  const sendAndConfirm = sendAndConfirmTransactionFactory({ rpc, rpcSubscriptions });
  await sendAndConfirm(signedTransaction as any, { commitment: "confirmed" });

  console.log(`Transaction sent: ${txSignature}`);
  console.log(`Explorer: https://explorer.solana.com/tx/${txSignature}\n`);

  // --- Verify ---
  console.log("Verifying initialization...");
  const { value: configAccount } = await rpc.getAccountInfo(configPDA, { encoding: "base64" }).send();
  const { value: tokenListAccount } = await rpc.getAccountInfo(tokenListPDA, { encoding: "base64" }).send();
  const { value: profilesAccount } = await rpc.getAccountInfo(profilesIndexPDA, { encoding: "base64" }).send();

  console.log(`  ${configAccount ? "OK" : "WARN"} Config PDA ${configAccount ? "created" : "not yet visible"}`);
  console.log(`  ${tokenListAccount ? "OK" : "WARN"} TokenList PDA ${tokenListAccount ? "created" : "not yet visible"}`);
  console.log(`  ${profilesAccount ? "OK" : "WARN"} ProfilesIndex PDA ${profilesAccount ? "created" : "not yet visible"}`);

  console.log("\nInitialization complete!");
  console.log("Run 'npm run check:mainnet' to verify full setup.");
}

main().catch(err => {
  console.error("Fatal error:", err);
  process.exit(1);
});
