#!/usr/bin/env node

import * as anchor from "@coral-xyz/anchor";
import { Connection, PublicKey, Keypair, LAMPORTS_PER_SOL, SystemProgram } from "@solana/web3.js";
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

const ROOT = path.join(__dirname, "..");
loadEnvFile(path.join(ROOT, ".env"));
loadEnvFile(path.join(ROOT, ".env.local"));

// ---------------------------------------------------------------------------
// Constants (must match programs/splitter/src/constants.rs)
// ---------------------------------------------------------------------------
const PROGRAM_ID = new PublicKey("56cRuWVNt5KXRgvA4m6wroB4D45A3SjvowZVZXYBw3Mr");
const CONFIG_SEED = Buffer.from("config");
const TOKEN_LIST_SEED = Buffer.from("token-list");
const PROFILES_INDEX_SEED = Buffer.from("profiles-index");

const ROUTE_AGENT_X402 = new Uint8Array([
  0x8d, 0xc5, 0x05, 0xbe, 0x33, 0x5e, 0x56, 0x5d, 0x2a, 0x5e, 0x2c, 0x96, 0x05, 0x7c, 0x7f, 0xb0,
  0xca, 0xff, 0x7c, 0x50, 0x09, 0xf6, 0x1b, 0x82, 0xac, 0x3e, 0xf5, 0xe7, 0xa9, 0xec, 0x0f, 0x1e,
]);

const ROUTE_MERCHANT_AIFP1 = new Uint8Array([
  0xb9, 0xdb, 0xf5, 0x87, 0xb0, 0xdf, 0x69, 0x87, 0x0d, 0xf1, 0xe6, 0x0b, 0x22, 0xfb, 0xa0, 0x31,
  0x7f, 0x53, 0xeb, 0x19, 0xd7, 0x8a, 0x57, 0x3a, 0xbf, 0x94, 0xfc, 0x38, 0x4a, 0x33, 0x9a, 0x89,
]);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
function parsePubkey(name: string, value: string | undefined): PublicKey {
  if (!value) throw new Error(`Missing env var: ${name}`);
  try {
    return new PublicKey(value);
  } catch {
    throw new Error(`Invalid pubkey for ${name}: ${value}`);
  }
}

function parseHexBytes32(name: string, value: string | undefined): Uint8Array {
  if (!value) throw new Error(`Missing env var: ${name}`);
  const clean = value.replace(/^0x/, "");
  if (clean.length !== 64) {
    throw new Error(`${name} must be 32 bytes (64 hex chars), got ${clean.length}`);
  }
  const bytes = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function parseHexBytes64(name: string, value: string | undefined): Uint8Array {
  if (!value) throw new Error(`Missing env var: ${name}`);
  const clean = value.replace(/^0x/, "");
  if (clean.length !== 128) {
    throw new Error(`${name} must be 64 bytes (128 hex chars), got ${clean.length}`);
  }
  const bytes = new Uint8Array(64);
  for (let i = 0; i < 64; i++) {
    bytes[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
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

function parsePubkeyArray(name: string, value: string | undefined): PublicKey[] {
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
  console.log("=== AiFinPay Splitter v1.4 — Initialize ===\n");

  // --- Parse env vars ---
  const deployerKeypairPath = process.env.DEPLOYER_KEYPAIR_PATH || "keypairs/devnet-deployer.json";
  const admin = parsePubkey("ADMIN_PUBKEY", process.env.ADMIN_PUBKEY);
  const signer = parseHexBytes64("SIGNER_PUBKEY", process.env.SIGNER_PUBKEY);
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
    throw new Error(`Keypair file not found: ${resolvedKeypairPath}\nGenerate with: solana-keygen new --no-passphrase -s -o ${resolvedKeypairPath}`);
  }
  const secret = JSON.parse(fs.readFileSync(resolvedKeypairPath, "utf-8"));
  const deployer = Keypair.fromSecretKey(new Uint8Array(secret));
  console.log(`Deployer pubkey:  ${deployer.publicKey.toBase58()}\n`);

  // --- Connection & balance ---
  const rpcUrl = process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com";
  const connection = new Connection(rpcUrl, "confirmed");

  const balance = await connection.getBalance(deployer.publicKey);
  console.log(`Balance: ${balance / LAMPORTS_PER_SOL} SOL`);
  if (balance < 0.5 * LAMPORTS_PER_SOL) {
    console.warn("⚠  Low balance — you may need more SOL for rent exemption.\n");
  }

  // --- Derive PDAs ---
  const [configPDA] = PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
  const [tokenListPDA] = PublicKey.findProgramAddressSync([TOKEN_LIST_SEED], PROGRAM_ID);
  const [profilesIndexPDA] = PublicKey.findProgramAddressSync([PROFILES_INDEX_SEED], PROGRAM_ID);

  console.log("PDAs:");
  console.log(`  Config:       ${configPDA.toBase58()}`);
  console.log(`  TokenList:    ${tokenListPDA.toBase58()}`);
  console.log(`  ProfilesIndex: ${profilesIndexPDA.toBase58()}\n`);

  // --- Check if already initialized ---
  const existingConfig = await connection.getAccountInfo(configPDA, "confirmed");
  if (existingConfig) {
    console.log("⚠  Config PDA already exists — program may be initialized.");
    const readline = require("readline").createInterface({ input: process.stdin, output: process.stdout });
    const answer = await new Promise<string>(resolve =>
      readline.question("Continue anyway? (y/N): ", (ans: string) => { readline.close(); resolve(ans); })
    );
    if (answer.toLowerCase() !== "y") {
      console.log("Aborted.");
      process.exit(0);
    }
  }

  // --- Build Anchor provider & program ---
  const wallet = new anchor.Wallet(deployer);
  const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
  anchor.setProvider(provider);

  const idlPath = path.join(ROOT, "target", "idl", "splitter.json");
  if (!fs.existsSync(idlPath)) {
    throw new Error(`IDL not found at ${idlPath}\nRun 'anchor build' first.`);
  }
  const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
  const program = new anchor.Program(idl, provider);

  // --- Send initialize transaction ---
  console.log("Sending initialize transaction...");

  try {
    const tx = await program.methods
      .initialize({
        admin,
        signer: Array.from(signer),
        pauser,
        treasury,
        stablecoins,
        routeIds: routeIds.map(arr => Array.from(arr)),
        treasuryBps,
        ipCreatorBps,
      })
      .accounts({
        payer: deployer.publicKey,
        config: configPDA,
        tokenList: tokenListPDA,
        profiles: profilesIndexPDA,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    console.log(`✅ Transaction sent: ${tx}`);
    console.log(`   Explorer: https://explorer.solana.com/tx/${tx}?cluster=devnet\n`);

    // --- Verify ---
    console.log("Verifying initialization...");
    await new Promise(r => setTimeout(r, 3000));

    const configAccount = await connection.getAccountInfo(configPDA, "confirmed");
    const tokenListAccount = await connection.getAccountInfo(tokenListPDA, "confirmed");
    const profilesAccount = await connection.getAccountInfo(profilesIndexPDA, "confirmed");

    const results = [
      ["Config", configAccount],
      ["TokenList", tokenListAccount],
      ["ProfilesIndex", profilesAccount],
    ] as const;

    for (const [name, acct] of results) {
      console.log(`  ${acct ? "✅" : "⚠"} ${name} PDA ${acct ? "created" : "not yet visible"}`);
    }

    console.log("\n🎉 Initialization complete!");
    console.log("Run 'npm run check:devnet' to verify full setup.");

  } catch (error: any) {
    console.error(`❌ Initialize failed: ${error.message}`);
    if (error.logs) {
      console.error("Transaction logs:");
      error.logs.forEach((log: string) => console.error(`  ${log}`));
    }
    process.exit(1);
  }
}

main().catch(err => {
  console.error("Fatal error:", err);
  process.exit(1);
});
