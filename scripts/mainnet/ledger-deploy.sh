#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# AiFinPay Splitter v1.4 — Mainnet Ledger Deploy Pipeline
# ─────────────────────────────────────────────────────────────────────────────
# Combined deploy + initialize script for Ledger-held deployer.
#
# Flow:
#   1. Pre-flight checks (env, keypair, balance, CI gates)
#   2. Build SBF binary with SPLITTER_DEPLOYER baked in
#   3. Deploy program to mainnet (Ledger signs)
#   4. Initialize PDAs via Anchor CLI (Ledger signs)
#   5. Configure routes via admin keypair
#   6. Verify readiness
#
# Prerequisites:
#   - Ledger connected, unlocked, Solana app open
#   - .env.production configured
#   - Anchor CLI, Solana CLI, Rust toolchain installed
#
# Usage:
#   ./scripts/mainnet/ledger-deploy.sh [--yes] [--skip-build] [--skip-init]
#                                      [--skip-routes] [--skip-verify]
#                                      [--ledger-index <n>]
#                                      [--url <rpc_url>]
#
# WARNING: REAL mainnet SOL is spent. Verify every address on the Ledger screen.
# ─────────────────────────────────────────────────────────────────────────────
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

# ─── Colors ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

# ─── Defaults ────────────────────────────────────────────────────────────────
CANONICAL_PROGRAM_ID="8dty5bD738Z9TzEkDu8vLSnhpJNWtEGMUEcYaKCUTY6y"
LEDGER_INDEX=0
SOLANA_URL="https://api.mainnet-beta.solana.com"
CONFIRM="ask"
SKIP_BUILD="false"
SKIP_INIT="false"
SKIP_ROUTES="false"
SKIP_VERIFY="false"

# ─── Parse arguments ─────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        --yes)            CONFIRM="yes"; shift ;;
        --skip-build)     SKIP_BUILD="true"; shift ;;
        --skip-init)      SKIP_INIT="true"; shift ;;
        --skip-routes)    SKIP_ROUTES="true"; shift ;;
        --skip-verify)    SKIP_VERIFY="true"; shift ;;
        --ledger-index)   LEDGER_INDEX="$2"; shift 2 ;;
        --url)            SOLANA_URL="$2"; shift 2 ;;
        -h|--help)
            echo "Usage: $0 [--yes] [--skip-build] [--skip-init] [--skip-routes]"
            echo "           [--skip-verify] [--ledger-index <n>] [--url <rpc>]"
            echo ""
            echo "Flags:"
            echo "  --yes              Skip all confirmation prompts"
            echo "  --skip-build       Skip SBF build (use existing binary)"
            echo "  --skip-init        Skip initialize step"
            echo "  --skip-routes      Skip route configuration"
            echo "  --skip-verify      Skip final verification"
            echo "  --ledger-index <n>  Ledger key index (default: 0)"
            echo "  --url <rpc>        Solana RPC URL (default: mainnet-beta)"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

LEDGER_KEY="usb://ledger?key=${LEDGER_INDEX}"

# ─── Helpers ─────────────────────────────────────────────────────────────────
log()    { echo -e "${CYAN}[ledger-deploy]${NC} $*"; }
ok()     { echo -e "${GREEN}  ✅ $*${NC}"; }
warn()   { echo -e "${YELLOW}  ⚠️  $*${NC}"; }
fail()   { echo -e "${RED}  ❌ $*${NC}"; exit 1; }
section(){ echo -e "\n${CYAN}══════════════════════════════════════════════════════════════${NC}"; echo -e "${CYAN}  $*${NC}"; echo -e "${CYAN}══════════════════════════════════════════════════════════════${NC}"; }

# ─── Load env ────────────────────────────────────────────────────────────────
load_env() {
    local env_file="$PROJECT_ROOT/.env.production"
    local env_local="$PROJECT_ROOT/.env.local"
    if [[ -f "$env_file" ]]; then
        set -a; source "$env_file"; set +a
        log "Loaded .env.production"
    else
        fail ".env.production not found"
    fi
    if [[ -f "$env_local" ]]; then
        set -a; source "$env_local"; set +a
        log "Loaded .env.local override"
    fi
}

# ═════════════════════════════════════════════════════════════════════════════
# STEP 0: Pre-flight
# ═════════════════════════════════════════════════════════════════════════════
section "STEP 0: Pre-flight Checks"

load_env

# Required env vars
for var in SPLITTER_DEPLOYER ADMIN_PUBKEY SIGNER_PUBKEY PAUSER_PUBKEY TREASURY_PUBKEY; do
    if [[ -z "${!var:-}" ]]; then
        fail "Missing env var: $var"
    fi
done
ok "Required env vars present"

# Validate SIGNER_PUBKEY is 128 hex chars (64 bytes)
SIGNER_CLEAN="${SIGNER_PUBKEY//0x/}"
if [[ ${#SIGNER_CLEAN} -ne 128 ]]; then
    fail "SIGNER_PUBKEY must be 128 hex chars (64 bytes), got ${#SIGNER_CLEAN}"
fi
ok "SIGNER_PUBKEY format valid (128 hex)"

# Validate SPLITTER_DEPLOYER is not placeholder
if [[ "$SPLITTER_DEPLOYER" == "11111111111111111111111111111111" ]]; then
    fail "SPLITTER_DEPLOYER is the default all-zero pubkey"
fi
ok "SPLITTER_DEPLOYER: $SPLITTER_DEPLOYER"

# Check program keypair exists and matches canonical ID
PROGRAM_KEYPAIR="$PROJECT_ROOT/keypairs/splitter-keypair.json"
if [[ ! -f "$PROGRAM_KEYPAIR" ]]; then
    fail "Canonical program keypair not found: $PROGRAM_KEYPAIR"
fi
KEYPAIR_ID=$(solana-keygen pubkey "$PROGRAM_KEYPAIR")
if [[ "$KEYPAIR_ID" != "$CANONICAL_PROGRAM_ID" ]]; then
    fail "Program keypair ($KEYPAIR_ID) != canonical ID ($CANONICAL_PROGRAM_ID)"
fi
ok "Program keypair matches canonical ID: $CANONICAL_PROGRAM_ID"

# Check declare_id! in source
LIB_RS="$PROJECT_ROOT/programs/splitter/src/lib.rs"
if ! grep -q "declare_id!(\"$CANONICAL_PROGRAM_ID\")" "$LIB_RS"; then
    fail "declare_id! in lib.rs does not match $CANONICAL_PROGRAM_ID"
fi
ok "declare_id! matches"

# Check Ledger is reachable
log "Checking Ledger at $LEDGER_KEY ..."
DEPLOYER_PUBKEY=$(solana address --keypair "$LEDGER_KEY" --url "$SOLANA_URL" 2>&1 | tail -n 1)
if [[ $? -ne 0 ]]; then
    fail "Cannot reach Ledger. Is it connected, unlocked, and Solana app open?"
fi
ok "Ledger deployer: $DEPLOYER_PUBKEY"

# Check baked DEPLOYER matches Ledger
if [[ "$SPLITTER_DEPLOYER" != "$DEPLOYER_PUBKEY" ]]; then
    warn "Baked DEPLOYER ($SPLITTER_DEPLOYER) != Ledger wallet ($DEPLOYER_PUBKEY)"
    warn "initialize must be signed by the baked DEPLOYER address."
    if [[ "$CONFIRM" != "yes" ]]; then
        read -r -p "Continue anyway? (y/N): " yn
        [[ "$yn" =~ ^[Yy]$ ]] || exit 1
    fi
else
    ok "Baked DEPLOYER matches Ledger wallet"
fi

# Balance check
BALANCE=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Mainnet balance: $BALANCE SOL"
if awk "BEGIN {exit !($BALANCE < 3.0)}"; then
    fail "Insufficient balance. Need ≥ 3 SOL, have $BALANCE SOL"
fi
ok "Balance sufficient"

# ═════════════════════════════════════════════════════════════════════════════
# STEP 1: Build
# ═════════════════════════════════════════════════════════════════════════════
section "STEP 1: Build SBF Binary"

if [[ "$SKIP_BUILD" == "true" ]]; then
    warn "Skipping build (--skip-build)"
    PROGRAM_SO="$PROJECT_ROOT/target/deploy/splitter.so"
    if [[ ! -f "$PROGRAM_SO" ]]; then
        fail "Program binary not found: $PROGRAM_SO"
    fi
else
    log "Building with SPLITTER_DEPLOYER=$SPLITTER_DEPLOYER ..."
    cd "$PROJECT_ROOT"
    SPLITTER_DEPLOYER="$SPLITTER_DEPLOYER" cargo build-sbf --manifest-path programs/splitter/Cargo.toml
    ok "Build complete"
fi

PROGRAM_SO="$PROJECT_ROOT/target/deploy/splitter.so"
PROGRAM_SIZE=$(stat -c%s "$PROGRAM_SO" 2>/dev/null || stat -f%z "$PROGRAM_SO")
log "Program size: $PROGRAM_SIZE bytes ($(( PROGRAM_SIZE / 1024 )) KB)"

# ═════════════════════════════════════════════════════════════════════════════
# STEP 2: Deploy
# ═════════════════════════════════════════════════════════════════════════════
section "STEP 2: Deploy to Mainnet"

# Check if program already exists (upgrade vs fresh)
if solana program show "$CANONICAL_PROGRAM_ID" --url "$SOLANA_URL" >/dev/null 2>&1; then
    UPGRADE_AUTH=$(solana program show "$CANONICAL_PROGRAM_ID" --url "$SOLANA_URL" 2>&1 | grep -i "upgrade authority" | awk '{print $NF}' || true)
    log "Program exists — this is an UPGRADE"
    log "On-chain upgrade authority: ${UPGRADE_AUTH:-<unknown>}"
    if [[ -n "$UPGRADE_AUTH" && "$UPGRADE_AUTH" != "$DEPLOYER_PUBKEY" ]]; then
        fail "Deployer ($DEPLOYER_PUBKEY) is not the on-chain upgrade authority ($UPGRADE_AUTH)"
    fi
else
    log "No existing program — this is a FRESH deploy"
fi

BALANCE_BEFORE=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')

if [[ "$CONFIRM" != "yes" ]]; then
    echo
    echo -e "${YELLOW}╔══════════════════════════════════════════════════════════════╗${NC}"
    echo -e "${YELLOW}║  DEPLOYING TO MAINNET — VERIFY ON LEDGER SCREEN            ║${NC}"
    echo -e "${YELLOW}║                                                            ║${NC}"
    echo -e "${YELLOW}║  Program : $CANONICAL_PROGRAM_ID  ║${NC}"
    echo -e "${YELLOW}║  Deployer: $DEPLOYER_PUBKEY                              ║${NC}"
    echo -e "${YELLOW}║  RPC     : $SOLANA_URL                                    ║${NC}"
    echo -e "${YELLOW}║  Balance : $BALANCE_BEFORE SOL                                        ║${NC}"
    echo -e "${YELLOW}╚══════════════════════════════════════════════════════════════╝${NC}"
    echo
    read -r -p "Type DEPLOY-MAINNET to continue: " response
    if [[ "$response" != "DEPLOY-MAINNET" ]]; then
        log "Aborted by operator."
        exit 1
    fi
fi

log "Deploying (confirm on Ledger)..."
solana program deploy "$PROGRAM_SO" \
    --program-id "$PROGRAM_KEYPAIR" \
    --keypair "$LEDGER_KEY" \
    --url "$SOLANA_URL"

BALANCE_AFTER=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
DEPLOY_COST=$(echo "$BALANCE_BEFORE - $BALANCE_AFTER" | bc)
ok "Deploy complete. Cost: $(printf "%.6f" "$DEPLOY_COST") SOL. Balance: $BALANCE_AFTER SOL"

# ═════════════════════════════════════════════════════════════════════════════
# STEP 3: Initialize PDAs
# ═════════════════════════════════════════════════════════════════════════════
section "STEP 3: Initialize PDAs"

if [[ "$SKIP_INIT" == "true" ]]; then
    warn "Skipping initialize (--skip-init)"
else
    log "Building initialize instruction..."

    # Derive PDAs for display
    CONFIG_PDA=$(solana address -k "$PROGRAM_KEYPAIR" --seed "config" --program-id "$CANONICAL_PROGRAM_ID" 2>/dev/null || echo "<derive manually>")
    TOKEN_LIST_PDA=$(solana address -k "$PROGRAM_KEYPAIR" --seed "token-list" --program-id "$CANONICAL_PROGRAM_ID" 2>/dev/null || echo "<derive manually>")
    PROFILES_PDA=$(solana address -k "$PROGRAM_KEYPAIR" --seed "profiles-index" --program-id "$CANONICAL_PROGRAM_ID" 2>/dev/null || echo "<derive manually>")

    log "PDAs:"
    log "  Config:        $CONFIG_PDA"
    log "  TokenList:     $TOKEN_LIST_PDA"
    log "  ProfilesIndex: $PROFILES_PDA"

    if [[ "$CONFIRM" != "yes" ]]; then
        echo
        echo -e "${YELLOW}Initialize will be signed via Anchor CLI with Ledger.${NC}"
        echo -e "${YELLOW}Verify ALL addresses on the Ledger screen before approving.${NC}"
        echo
        read -r -p "Press Enter when Ledger is ready..."
    fi

    # Use Anchor CLI with Ledger wallet for initialize
    # Anchor supports --provider.wallet for Ledger signing
    log "Running initialize via Anchor CLI (confirm on Ledger)..."

    cd "$PROJECT_ROOT"
    ANCHOR_WALLET="$LEDGER_KEY" anchor run initialize:mainnet 2>&1 || {
        warn "Anchor run failed. Falling back to manual instruction..."
        echo
        echo "═══════════════════════════════════════════════════════════════"
        echo "  MANUAL INITIALIZE — run this command:"
        echo "═══════════════════════════════════════════════════════════════"
        echo
        echo "  cd $PROJECT_ROOT"
        echo "  DEPLOYER_KEYPAIR_PATH='$LEDGER_KEY' \\"
        echo "    ADMIN_PUBKEY=$ADMIN_PUBKEY \\"
        echo "    SIGNER_PUBKEY=$SIGNER_PUBKEY \\"
        echo "    PAUSER_PUBKEY=$PAUSER_PUBKEY \\"
        echo "    TREASURY_PUBKEY=$TREASURY_PUBKEY \\"
        echo "    STABLECOINS=${STABLECOINS:-} \\"
        echo "    ROUTE_IDS=${ROUTE_IDS:-AGENT_X402,MERCHANT_AIFP1} \\"
        echo "    TREASURY_BPS=${TREASURY_BPS:-0,100} \\"
        echo "    IP_CREATOR_BPS=${IP_CREATOR_BPS:-0,0} \\"
        echo "    pnpm initialize:mainnet"
        echo
        echo "  OR use a Ledger-capable client with:"
        echo "    Program: $CANONICAL_PROGRAM_ID"
        echo "    Instruction: initialize"
        echo "    Accounts: [deployer(signer), config, token_list, profiles, system_program]"
        echo "═══════════════════════════════════════════════════════════════"
        echo
        read -r -p "Type YES when initialize is complete: " yn
        [[ "$yn" == "YES" ]] || exit 1
    }
    ok "Initialize complete"
fi

# ═════════════════════════════════════════════════════════════════════════════
# STEP 4: Configure Routes
# ═════════════════════════════════════════════════════════════════════════════
section "STEP 4: Configure Routes"

if [[ "$SKIP_ROUTES" == "true" ]]; then
    warn "Skipping route configuration (--skip-routes)"
else
    if [[ -z "${ADMIN_KEYPAIR_PATH:-}" ]]; then
        warn "ADMIN_KEYPAIR_PATH not set — skipping route configuration"
        warn "Run manually: ADMIN_KEYPAIR_PATH=<path> pnpm configure:mainnet"
    else
        log "Configuring routes (admin signs)..."
        cd "$PROJECT_ROOT"
        ADMIN_KEYPAIR_PATH="$ADMIN_KEYPAIR_PATH" pnpm configure:mainnet
        ok "Routes configured"
    fi
fi

# ═════════════════════════════════════════════════════════════════════════════
# STEP 5: Verify
# ═════════════════════════════════════════════════════════════════════════════
section "STEP 5: Verify Readiness"

if [[ "$SKIP_VERIFY" == "true" ]]; then
    warn "Skipping verification (--skip-verify)"
else
    cd "$PROJECT_ROOT"
    pnpm check:mainnet || warn "Verification had issues — check output above"
fi

# ═════════════════════════════════════════════════════════════════════════════
# Summary
# ═════════════════════════════════════════════════════════════════════════════
section "DEPLOYMENT COMPLETE"

log "Program  : $CANONICAL_PROGRAM_ID"
log "Deployer : $DEPLOYER_PUBKEY"
log "Explorer : https://explorer.solana.com/address/$CANONICAL_PROGRAM_ID"
echo
log "Post-deploy checklist:"
log "  1. Verify program is executable on explorer"
log "  2. Verify Config/TokenList/ProfilesIndex PDAs exist"
log "  3. Test a small settle_native / settle_stable transaction"
log "  4. Consider rotating deployer role if using multisig"
echo
log "To rotate admin (if needed):"
log "  ADMIN_KEYPAIR_PATH=<path> pnpm rotate:admin -- --new-admin <PUBKEY>"
echo
log "To rotate signer (if needed):"
log "  pnpm rotate:signer -- --pubkey <NEW_SIGNER_HEX> --instruction rotate --yes"
