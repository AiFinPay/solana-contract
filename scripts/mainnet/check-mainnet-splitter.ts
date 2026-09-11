#!/usr/bin/env node

// Mainnet readiness check for the canonical splitter program.
// Mirror of scripts/devnet/check-devnet-splitter.ts pointed at mainnet-beta
// and the canonical program ID. Read-only: sends no transactions.

import { Connection, PublicKey, LAMPORTS_PER_SOL } from "@solana/web3.js";

// Constants from the program (canonical mainnet program ID = declare_id!).
const PROGRAM_ID = new PublicKey("724Ut31i4ecY4dJ25z8HuZetu3A43xtNkPdk4JdbsfdD");
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

const MAINNET_RPC = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";

interface RouteProfileEntry {
  routeId: Uint8Array;
  treasuryBps: number;
  ipCreatorBps: number;
  enabled: boolean;
  configuredAt: bigint;
  routeTreasury: PublicKey;
}

interface ConfigAccount {
  admin: PublicKey;
  signer: Uint8Array;
  pauser: PublicKey;
  treasury: PublicKey;
  tokenList: PublicKey;
  profiles: PublicKey;
  bump: number;
  isPaused: boolean;
}

interface TokenListAccount {
  admin: PublicKey;
  tokens: PublicKey[];
  bump: number;
}

interface ProfilesIndexAccount {
  entries: RouteProfileEntry[];
  count: number;
  bump: number;
}

async function getConfigPDA(): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddressSync([CONFIG_SEED], PROGRAM_ID);
}

async function getTokenListPDA(): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddressSync([TOKEN_LIST_SEED], PROGRAM_ID);
}

async function getProfilesIndexPDA(): Promise<[PublicKey, number]> {
  return PublicKey.findProgramAddressSync([PROFILES_INDEX_SEED], PROGRAM_ID);
}

function decodeConfig(data: Buffer): ConfigAccount {
  let offset = 8; // discriminator
  const admin = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;
  const signer = new Uint8Array(data.slice(offset, offset + 64));
  offset += 64;
  const pauser = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;
  const treasury = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;
  const tokenList = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;
  const profiles = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;
  const bump = data[offset++];
  const isPaused = data[offset] === 1;
  return { admin, signer, pauser, treasury, tokenList, profiles, bump, isPaused };
}

function decodeTokenList(data: Buffer): TokenListAccount {
  let offset = 8;
  const admin = new PublicKey(data.slice(offset, offset + 32));
  offset += 32;
  const vecLen = data.readUInt32LE(offset);
  offset += 4;
  const tokens: PublicKey[] = [];
  for (let i = 0; i < vecLen; i++) {
    tokens.push(new PublicKey(data.slice(offset, offset + 32)));
    offset += 32;
  }
  const bump = data[offset];
  return { admin, tokens, bump };
}

function decodeProfilesIndex(data: Buffer): ProfilesIndexAccount {
  let offset = 8;
  const vecLen = data.readUInt32LE(offset);
  offset += 4;
  const entries: RouteProfileEntry[] = [];
  for (let i = 0; i < vecLen; i++) {
    const routeId = new Uint8Array(data.slice(offset, offset + 32));
    offset += 32;
    const treasuryBps = data.readUInt16LE(offset);
    offset += 2;
    const ipCreatorBps = data.readUInt16LE(offset);
    offset += 2;
    const enabled = data[offset++] === 1;
    const configuredAt = data.readBigInt64LE(offset);
    offset += 8;
    const routeTreasury = new PublicKey(data.slice(offset, offset + 32));
    offset += 32;
    entries.push({ routeId, treasuryBps, ipCreatorBps, enabled, configuredAt, routeTreasury });
  }
  const count = data[offset++];
  const bump = data[offset];
  return { entries, count, bump };
}

function arraysEqual(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

async function main() {
  console.log("=== AiFinPay Splitter v1.4 Mainnet Check ===\n");

  const connection = new Connection(MAINNET_RPC, "confirmed");

  // 1. Check program exists
  console.log("1. Checking program deployment...");
  const programAccount = await connection.getAccountInfo(PROGRAM_ID, "confirmed");
  if (!programAccount) {
    console.error("❌ Program not found at:", PROGRAM_ID.toBase58());
    process.exit(1);
  }
  if (!programAccount.executable) {
    console.error("❌ Account exists but is not executable");
    process.exit(1);
  }
  console.log(`✅ Program deployed: ${PROGRAM_ID.toBase58()}`);
  console.log(`   Owner: ${programAccount.owner.toBase58()}`);
  console.log(`   Lamports: ${programAccount.lamports / LAMPORTS_PER_SOL} SOL`);
  console.log(`   Data size: ${programAccount.data.length} bytes\n`);

  // 2. Check Config PDA
  console.log("2. Checking Config PDA...");
  const [configPDA, configBump] = await getConfigPDA();
  console.log(`   Config PDA: ${configPDA.toBase58()} (bump: ${configBump})`);
  const configAccount = await connection.getAccountInfo(configPDA, "confirmed");
  if (!configAccount) {
    console.error("❌ Config PDA not found - program not initialized");
    process.exit(1);
  }
  const config = decodeConfig(configAccount.data);
  console.log("✅ Config initialized:");
  console.log(`   Admin: ${config.admin.toBase58()}`);
  console.log(`   Pauser: ${config.pauser.toBase58()}`);
  console.log(`   Treasury: ${config.treasury.toBase58()}`);
  console.log(`   Token List: ${config.tokenList.toBase58()}`);
  console.log(`   Profiles: ${config.profiles.toBase58()}`);
  console.log(`   Bump: ${config.bump}`);
  console.log(`   Paused: ${config.isPaused ? "YES" : "NO"}\n`);

  // 3. Check TokenList PDA
  console.log("3. Checking TokenList PDA...");
  const [tokenListPDA, tokenListBump] = await getTokenListPDA();
  console.log(`   TokenList PDA: ${tokenListPDA.toBase58()} (bump: ${tokenListBump})`);
  const tokenListAccount = await connection.getAccountInfo(tokenListPDA, "confirmed");
  if (!tokenListAccount) {
    console.error("❌ TokenList PDA not found");
    process.exit(1);
  }
  const tokenList = decodeTokenList(tokenListAccount.data);
  console.log("✅ TokenList initialized:");
  console.log(`   Admin: ${tokenList.admin.toBase58()}`);
  console.log(`   Whitelisted tokens (${tokenList.tokens.length}):`);
  if (tokenList.tokens.length === 0) {
    console.log("     (none)");
  } else {
    tokenList.tokens.forEach((t, i) => console.log(`     ${i + 1}. ${t.toBase58()}`));
  }
  console.log(`   Bump: ${tokenList.bump}\n`);

  // 4. Check ProfilesIndex PDA
  console.log("4. Checking ProfilesIndex PDA...");
  const [profilesIndexPDA, profilesIndexBump] = await getProfilesIndexPDA();
  console.log(`   ProfilesIndex PDA: ${profilesIndexPDA.toBase58()} (bump: ${profilesIndexBump})`);
  const profilesIndexAccount = await connection.getAccountInfo(profilesIndexPDA, "confirmed");
  if (!profilesIndexAccount) {
    console.error("❌ ProfilesIndex PDA not found");
    process.exit(1);
  }
  const profilesIndex = decodeProfilesIndex(profilesIndexAccount.data);
  console.log("✅ ProfilesIndex initialized:");
  console.log(`   Route count: ${profilesIndex.count}`);
  console.log(`   Bump: ${profilesIndex.bump}`);
  console.log("   Routes:");
  if (profilesIndex.entries.length === 0) {
    console.log("     (none configured)");
  } else {
    profilesIndex.entries.forEach((entry, i) => {
      const isAgentX402 = arraysEqual(entry.routeId, ROUTE_AGENT_X402);
      const isMerchantAIFP1 = arraysEqual(entry.routeId, ROUTE_MERCHANT_AIFP1);
      let label = "";
      if (isAgentX402) label = " [ROUTE_AGENT_X402]";
      else if (isMerchantAIFP1) label = " [ROUTE_MERCHANT_AIFP1]";
      console.log(`     ${i + 1}. Route ID: ${Buffer.from(entry.routeId).toString("hex")}${label}`);
      console.log(`        Treasury BPS: ${entry.treasuryBps} (${entry.treasuryBps / 100}%)`);
      console.log(`        IP Creator BPS: ${entry.ipCreatorBps} (${entry.ipCreatorBps / 100}%)`);
      console.log(`        Enabled: ${entry.enabled ? "YES" : "NO"}`);
      console.log(`        Configured at: ${new Date(Number(entry.configuredAt) * 1000).toISOString()}`);
      console.log(`        Route Treasury: ${entry.routeTreasury.toBase58()}`);
    });
  }
  console.log("");

  // 5. Verify required routes
  console.log("5. Verifying required routes...");
  const hasAgentX402 = profilesIndex.entries.some(e => arraysEqual(e.routeId, ROUTE_AGENT_X402));
  const hasMerchantAIFP1 = profilesIndex.entries.some(e => arraysEqual(e.routeId, ROUTE_MERCHANT_AIFP1));

  if (hasAgentX402) {
    const entry = profilesIndex.entries.find(e => arraysEqual(e.routeId, ROUTE_AGENT_X402))!;
    console.log(`✅ ROUTE_AGENT_X402 configured (enabled: ${entry.enabled})`);
  } else {
    console.log("❌ ROUTE_AGENT_X402 NOT configured");
  }

  if (hasMerchantAIFP1) {
    const entry = profilesIndex.entries.find(e => arraysEqual(e.routeId, ROUTE_MERCHANT_AIFP1))!;
    console.log(`✅ ROUTE_MERCHANT_AIFP1 configured (enabled: ${entry.enabled})`);
  } else {
    console.log("❌ ROUTE_MERCHANT_AIFP1 NOT configured");
  }

  // 6. Overall readiness check
  console.log("\n=== READINESS SUMMARY ===");
  const checks = {
    programDeployed: true,
    configInitialized: true,
    tokenListInitialized: true,
    profilesIndexInitialized: true,
    agentX402Configured: hasAgentX402,
    merchantAIFP1Configured: hasMerchantAIFP1,
    notPaused: !config.isPaused,
  };

  Object.entries(checks).forEach(([check, pass]) => {
    console.log(`  ${pass ? "✅" : "❌"} ${check}`);
  });

  const allPassed = Object.values(checks).every(Boolean);
  console.log(`\n${allPassed ? "✅ READY FOR PRODUCTION" : "❌ NOT READY - Fix issues above"}`);
  process.exit(allPassed ? 0 : 1);
}

main().catch(err => {
  console.error("Fatal error:", err);
  process.exit(1);
});
