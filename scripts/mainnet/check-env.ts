#!/usr/bin/env node

// Pre-flight environment check for mainnet deployment.
// Run before deploy, initialize, or configure to verify all required
// env vars are set and valid. Loads .env.production + .env.local
// (same order as the other mainnet scripts).

import * as fs from "fs";
import * as path from "path";

// ---------------------------------------------------------------------------
// Load env files (mirrors initialize-mainnet-splitter.ts loadEnvFile)
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
const envProd = path.join(ROOT, ".env.production");

loadEnvFile(envProd);

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
let errors = 0;
let warnings = 0;

function loadProgramIdFromAnchorToml(): string | null {
  const anchorToml = path.join(ROOT, "Anchor.toml");
  if (!fs.existsSync(anchorToml)) return null;
  const content = fs.readFileSync(anchorToml, "utf-8");
  const match = content.match(/\[programs\.mainnet\][\s\S]*?splitter\s*=\s*"([^"]+)"/);
  return match ? match[1] : null;
}

function loadProgramIdFromIdl(): string | null {
  const idlPath = path.join(ROOT, "target", "idl", "splitter.json");
  if (!fs.existsSync(idlPath)) return null;
  try {
    const idl = JSON.parse(fs.readFileSync(idlPath, "utf-8"));
    return idl.address || idl.metadata?.address || null;
  } catch {
    return null;
  }
}

function required(name: string, ...validators: ((v: string) => string | null)[]) {
  const val = process.env[name];
  if (!val || val.trim() === "") {
    console.log(`  ❌ ${name}: MISSING`);
    errors++;
    return;
  }
  for (const validate of validators) {
    const err = validate(val);
    if (err) {
      console.log(`  ❌ ${name}: ${err}`);
      errors++;
      return;
    }
  }
  const display = val.length > 60 ? val.slice(0, 57) + "..." : val;
  console.log(`  ✅ ${name}: ${display}`);
}

function optional(name: string, fallback?: string) {
  const val = process.env[name] || fallback;
  if (!val || val.trim() === "") {
    console.log(`  ⚠️  ${name}: not set (will use default)`);
    warnings++;
    return;
  }
  const display = val.length > 60 ? val.slice(0, 57) + "..." : val;
  console.log(`  ✅ ${name}: ${display}`);
}

function fileExists(name: string) {
  return (v: string): string | null => {
    const resolved = path.resolve(v);
    if (!fs.existsSync(resolved)) return `file not found: ${resolved}`;
    return null;
  };
}

function noDevnetKey(name: string) {
  return (v: string): string | null => {
    if (v.toLowerCase().includes("devnet")) return `devnet keypair — use a mainnet key`;
    return null;
  };
}

function isBase58Pubkey(name: string) {
  return (v: string): string | null => {
    // Basic base58 check: no 0OIl characters, reasonable length
    if (!/^[1-9A-HJ-NP-Za-km-z]{32,44}$/.test(v)) {
      return `not a valid base58 pubkey (got ${v.length} chars)`;
    }
    return null;
  };
}

function isHex128(name: string) {
  return (v: string): string | null => {
    const clean = v.replace(/^0x/, "");
    if (clean.length !== 128) return `must be 128 hex chars (64 bytes), got ${clean.length}`;
    if (!/^[0-9a-fA-F]+$/.test(clean)) return "contains non-hex characters";
    return null;
  };
}

function isU16Array(name: string) {
  return (v: string): string | null => {
    const parts = v.split(",").map(s => s.trim());
    for (const p of parts) {
      const n = parseInt(p, 10);
      if (isNaN(n) || n < 0 || n > 65535) return `value "${p}" is not a valid u16`;
    }
    return null;
  };
}

// ---------------------------------------------------------------------------
// Checks
// ---------------------------------------------------------------------------
const step = process.argv[2]; // deploy | initialize | configure | all

console.log("=== AiFinPay Splitter v1.4 — Pre-flight Env Check ===\n");

if (step) {
  console.log(`Step: ${step}\n`);
} else {
  console.log("Step: all (pass 'deploy', 'initialize', or 'configure' to check specific step)\n");
}

// --- Build / Deploy vars ---
if (!step || step === "deploy") {
  console.log("▸ Deploy (mainnet-deploy.sh)");
  required("SPLITTER_DEPLOYER", isBase58Pubkey("SPLITTER_DEPLOYER"));

  // Program ID source of truth
  const programId = loadProgramIdFromAnchorToml() || loadProgramIdFromIdl();
  if (programId) {
    console.log(`  ✅ PROGRAM_ID (Anchor.toml): ${programId}`);
  } else {
    console.log("  ❌ PROGRAM_ID: cannot read from Anchor.toml or target/idl/splitter.json");
    errors++;
  }
  console.log("");
}

// --- Initialize vars ---
if (!step || step === "initialize") {
  console.log("▸ Initialize (initialize-mainnet-splitter.ts)");
  required("DEPLOYER_KEYPAIR_PATH", fileExists("DEPLOYER_KEYPAIR_PATH"), noDevnetKey("DEPLOYER_KEYPAIR_PATH"));
  required("ADMIN_PUBKEY", isBase58Pubkey("ADMIN_PUBKEY"));
  required("SIGNER_PUBKEY", isHex128("SIGNER_PUBKEY"));
  required("PAUSER_PUBKEY", isBase58Pubkey("PAUSER_PUBKEY"));
  required("TREASURY_PUBKEY", isBase58Pubkey("TREASURY_PUBKEY"));
  optional("STABLECOINS");
  optional("ROUTE_IDS", "AGENT_X402,MERCHANT_AIFP1");
  required("TREASURY_BPS", isU16Array("TREASURY_BPS"));
  required("IP_CREATOR_BPS", isU16Array("IP_CREATOR_BPS"));
  optional("SOLANA_RPC_URL", "https://api.mainnet-beta.solana.com");
  console.log("");
}

// --- Configure vars ---
if (!step || step === "configure") {
  console.log("▸ Configure (configure-mainnet-route.ts)");
  required("ADMIN_KEYPAIR_PATH", fileExists("ADMIN_KEYPAIR_PATH"), noDevnetKey("ADMIN_KEYPAIR_PATH"));
  required("TREASURY_PUBKEY", isBase58Pubkey("TREASURY_PUBKEY"));
  optional("SOLANA_RPC_URL", "https://api.mainnet-beta.solana.com");
  console.log("");
}

// --- Cross-step consistency ---
if (!step || step === "all") {
  console.log("▸ Consistency checks");
  const deployer = process.env.SPLITTER_DEPLOYER;
  const admin = process.env.ADMIN_PUBKEY;
  const pauser = process.env.PAUSER_PUBKEY;
  const treasury = process.env.TREASURY_PUBKEY;

  if (deployer && admin && deployer === admin) {
    console.log("  ⚠️  SPLITTER_DEPLOYER == ADMIN_PUBKEY (same wallet — OK but less separation)");
    warnings++;
  } else {
    console.log("  ✅ SPLITTER_DEPLOYER != ADMIN_PUBKEY (good role separation)");
  }

  if (admin && pauser && admin === pauser) {
    console.log("  ⚠️  ADMIN_PUBKEY == PAUSER_PUBKEY (same wallet — OK but less separation)");
    warnings++;
  } else {
    console.log("  ✅ ADMIN_PUBKEY != PAUSER_PUBKEY (good role separation)");
  }

  // Route BPS length match
  const routeIds = (process.env.ROUTE_IDS || "AGENT_X402,MERCHANT_AIFP1").split(",").length;
  const treasuryBps = (process.env.TREASURY_BPS || "").split(",").filter(s => s.trim()).length;
  const ipCreatorBps = (process.env.IP_CREATOR_BPS || "").split(",").filter(s => s.trim()).length;

  if (treasuryBps > 0 && ipCreatorBps > 0) {
    if (routeIds === treasuryBps && routeIds === ipCreatorBps) {
      console.log(`  ✅ ROUTE_IDS (${routeIds}) == TREASURY_BPS (${treasuryBps}) == IP_CREATOR_BPS (${ipCreatorBps})`);
    } else {
      console.log(`  ❌ Array length mismatch: ROUTE_IDS=${routeIds}, TREASURY_BPS=${treasuryBps}, IP_CREATOR_BPS=${ipCreatorBps}`);
      errors++;
    }
  }
  console.log("");
}

// --- Loaded env files ---
console.log("▸ Env files loaded");
console.log(`  .env.production: ${fs.existsSync(envProd) ? "found" : "NOT FOUND"}`);
console.log("");

// --- Summary ---
console.log("=== SUMMARY ===");
if (errors === 0 && warnings === 0) {
  console.log("✅ All checks passed. Ready to proceed.");
} else if (errors === 0) {
  console.log(`⚠️  ${warnings} warning(s), 0 errors. Proceed with caution.`);
} else {
  console.log(`❌ ${errors} error(s), ${warnings} warning(s). Fix errors before proceeding.`);
}

process.exit(errors > 0 ? 1 : 0);
