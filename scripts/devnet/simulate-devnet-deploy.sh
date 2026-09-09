#!/usr/bin/env bash
# Real devnet deploy simulation for an Anchor/Solana program.
# Builds the program, deploys it to devnet, reports the actual on-chain cost,
# and optionally closes the program to recover rent.
#
# WARNING: This script performs real transactions on devnet. It requires a
# funded devnet wallet and consumes a small amount of SOL for fees. The
# deployed program is a throwaway measurement artifact.
#
# Usage:
#   ./simulate-devnet-deploy.sh [--program <name>] [--keypair <path>] [--url <rpc_url>]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
DEPLOY_DIR="$PROJECT_ROOT/target/deploy"

# Defaults.
PROGRAM_NAME="splitter"
KEYPAIR_DIR="$PROJECT_ROOT/keypairs"
DEPLOYER_KEYPAIR="$KEYPAIR_DIR/devnet-deployer.json"
SOLANA_URL="https://api.devnet.solana.com"

# Parse arguments.
while [[ $# -gt 0 ]]; do
    case "$1" in
        --program)
            PROGRAM_NAME="$2"
            shift 2
            ;;
        --keypair)
            DEPLOYER_KEYPAIR="$2"
            shift 2
            ;;
        --url)
            SOLANA_URL="$2"
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [--program <program_name>] [--keypair <keypair_path>] [--url <rpc_url>]"
            echo "  --program   Program name under programs/ (default: splitter)"
            echo "  --keypair   Path to deployer keypair (default: keypairs/devnet-deployer.json)"
            echo "  --url       Solana cluster RPC URL (default: https://api.devnet.solana.com)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--program <program_name>] [--keypair <keypair_path>] [--url <rpc_url>]"
            exit 1
            ;;
    esac
done

PROGRAM_SO="$DEPLOY_DIR/${PROGRAM_NAME}.so"
PROGRAM_MANIFEST="$PROJECT_ROOT/programs/$PROGRAM_NAME/Cargo.toml"
LOG_DIR="$PROJECT_ROOT/target/deploy-logs"
DEPLOY_LOG="$LOG_DIR/${PROGRAM_NAME}-devnet-deploy-$(date +%Y%m%d-%H%M%S).log"

log() {
    echo "[devnet-deploy] $*"
    mkdir -p "$LOG_DIR"
    echo "$(date -Iseconds) $*" >> "$DEPLOY_LOG"
}

fmt_sol() {
    printf "%.9f" "$1"
}

# Ensure the local project keypair exists.
if [[ ! -f "$DEPLOYER_KEYPAIR" ]]; then
    log "Deployer keypair not found: $DEPLOYER_KEYPAIR"
    log "Generate one with: solana-keygen new --no-passphrase -s -o $DEPLOYER_KEYPAIR"
    exit 1
fi

DEPLOYER_PUBKEY=$(solana-keygen pubkey "$DEPLOYER_KEYPAIR")
log "Deployer wallet : $DEPLOYER_PUBKEY"
log "Keypair path    : $DEPLOYER_KEYPAIR"

BALANCE=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Devnet balance  : $BALANCE SOL"

REQUIRED_BALANCE="0.05"
if awk "BEGIN {exit !($BALANCE < $REQUIRED_BALANCE)}"; then
    log "Insufficient devnet balance. Need at least $REQUIRED_BALANCE SOL."
    log "Request airdrop: solana airdrop 2 --keypair $DEPLOYER_KEYPAIR --url devnet"
    exit 1
fi

# Build if needed.
if [[ ! -f "$PROGRAM_SO" ]]; then
    if [[ ! -f "$PROGRAM_MANIFEST" ]]; then
        log "Program manifest not found: $PROGRAM_MANIFEST"
        log "Usage: $0 [--program <program_name>] [--keypair <keypair_path>] [--url <rpc_url>]"
        exit 1
    fi
    log "Program binary not found. Building $PROGRAM_NAME..."
    cd "$PROJECT_ROOT"
    cargo build-sbf --manifest-path "$PROGRAM_MANIFEST"
fi

PROGRAM_SIZE=$(stat -f%z "$PROGRAM_SO" 2>/dev/null || stat -c%s "$PROGRAM_SO")
log "Program size   : $PROGRAM_SIZE bytes ($(echo "scale=2; $PROGRAM_SIZE / 1024" | bc) KB)"

# Generate a throwaway program keypair so the deployment does not collide with
# the canonical program ID.
PROGRAM_KEYPAIR="$LOG_DIR/throwaway-program-keypair.json"
if [[ ! -f "$PROGRAM_KEYPAIR" ]]; then
    solana-keygen new --no-passphrase -s -o "$PROGRAM_KEYPAIR" > /dev/null 2>&1
fi
PROGRAM_ID=$(solana-keygen pubkey "$PROGRAM_KEYPAIR")
log "Throwaway program ID: $PROGRAM_ID"

# Record pre-deploy balance.
BALANCE_BEFORE=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance before deploy: $BALANCE_BEFORE SOL"

# Deploy and capture output.
log "Deploying $PROGRAM_NAME to devnet (this may take a minute)..."
solana program deploy "$PROGRAM_SO" \
    --program-id "$PROGRAM_KEYPAIR" \
    --keypair "$DEPLOYER_KEYPAIR" \
    --url "$SOLANA_URL" 2>&1 | tee -a "$DEPLOY_LOG"

# Record post-deploy balance.
BALANCE_AFTER=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance after deploy : $BALANCE_AFTER SOL"

# Compute actual cost from balance delta.
ACTUAL_DEPLOY_COST=$(echo "$BALANCE_BEFORE - $BALANCE_AFTER" | bc)
log "Actual deploy cost   : $(fmt_sol "$ACTUAL_DEPLOY_COST") SOL"
log "(balance delta includes all deployment transaction fees and rent)"

# PDA rent from chain (same cluster).
log ""
log "=== PDA Rent Exemption (devnet) ==="
CONFIG_SIZE=234
TOKEN_LIST_SIZE=557
PROFILES_SIZE=2478

CONFIG_RENT=$(solana rent "$CONFIG_SIZE" --url "$SOLANA_URL" 2>&1 | grep "Rent-exempt minimum" | awk '{print $3}')
TOKEN_LIST_RENT=$(solana rent "$TOKEN_LIST_SIZE" --url "$SOLANA_URL" 2>&1 | grep "Rent-exempt minimum" | awk '{print $3}')
PROFILES_RENT=$(solana rent "$PROFILES_SIZE" --url "$SOLANA_URL" 2>&1 | grep "Rent-exempt minimum" | awk '{print $3}')

log "Config        ($CONFIG_SIZE bytes) : $(fmt_sol "$CONFIG_RENT") SOL"
log "TokenList     ($TOKEN_LIST_SIZE bytes) : $(fmt_sol "$TOKEN_LIST_RENT") SOL"
log "ProfilesIndex ($PROFILES_SIZE bytes) : $(fmt_sol "$PROFILES_RENT") SOL"

TOTAL_RENT=$(echo "$CONFIG_RENT + $TOKEN_LIST_RENT + $PROFILES_RENT" | bc)
log "Total PDA rent                    : $(fmt_sol "$TOTAL_RENT") SOL"

log ""
log "=== Summary ==="
log "Actual program deploy cost : $(fmt_sol "$ACTUAL_DEPLOY_COST") SOL"
log "Estimated PDA rent         : $(fmt_sol "$TOTAL_RENT") SOL"
TOTAL_ESTIMATED=$(echo "$ACTUAL_DEPLOY_COST + $TOTAL_RENT" | bc)
log "Total estimated launch cost: $(fmt_sol "$TOTAL_ESTIMATED") SOL"

# Offer to close the throwaway program and recover rent.
echo
read -r -p "Close the throwaway devnet program and recover rent? [y/N] " response
if [[ "$response" =~ ^[Yy]$ ]]; then
    log "Closing program $PROGRAM_ID..."
    solana program close "$PROGRAM_ID" \
        --keypair "$DEPLOYER_KEYPAIR" \
        --url "$SOLANA_URL" \
        --bypass-warning 2>&1 | tee -a "$DEPLOY_LOG"
    BALANCE_CLOSED=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
    log "Balance after close: $BALANCE_CLOSED SOL"
    RECOVERED=$(echo "$BALANCE_CLOSED - $BALANCE_AFTER" | bc)
    log "Recovered rent     : $(fmt_sol "$RECOVERED") SOL"
else
    log "Program left deployed on devnet: $PROGRAM_ID"
    log "To close later: solana program close $PROGRAM_ID --keypair $DEPLOYER_KEYPAIR --url $SOLANA_URL"
fi

log "Full log saved to: $DEPLOY_LOG"
