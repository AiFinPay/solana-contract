#!/usr/bin/env bash
# Real MAINNET deploy simulation for an Anchor/Solana program.
# Mirror of scripts/devnet/simulate-devnet-deploy.sh pointed at mainnet-beta.
# Builds the program, deploys a THROWAWAY program ID to mainnet, reports the
# actual on-chain cost, and optionally closes the program to recover rent.
#
# WARNING: This script performs real transactions on MAINNET and spends real
# SOL (~2.5 SOL for a ~360 KB program; most is refundable on close, fees are
# not). It NEVER touches the canonical program ID. Requires typing
# SIMULATE-MAINNET to proceed (override with --yes).
#
# Usage:
#   ./simulate-mainnet-deploy.sh [--program <name>] [--keypair <path|ledger-url>]
#                                [--url <rpc_url>] [--yes]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
DEPLOY_DIR="$PROJECT_ROOT/target/deploy"

# Defaults.
PROGRAM_NAME="splitter"
KEYPAIR_DIR="$PROJECT_ROOT/keypairs"
DEPLOYER_KEYPAIR="usb://ledger?key=0"
SOLANA_URL="https://api.mainnet-beta.solana.com"
CONFIRM="ask"

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
        --yes)
            CONFIRM="yes"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--program <program_name>] [--keypair <path|ledger-url>] [--url <rpc_url>] [--yes]"
            echo "  --program   Program name under programs/ (default: splitter)"
            echo "  --keypair   Deployer signer: Ledger URL (default: usb://ledger?key=0) or file keypair path"
            echo "  --url       Solana cluster RPC URL (default: https://api.mainnet-beta.solana.com)"
            echo "  --yes       Skip the SIMULATE-MAINNET confirmation prompt"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--program <program_name>] [--keypair <path|ledger-url>] [--url <rpc_url>] [--yes]"
            exit 1
            ;;
    esac
done

PROGRAM_SO="$DEPLOY_DIR/${PROGRAM_NAME}.so"
PROGRAM_MANIFEST="$PROJECT_ROOT/programs/$PROGRAM_NAME/Cargo.toml"
LOG_DIR="$PROJECT_ROOT/deployments/splitter_v14"
TS="$(date +%Y%m%d-%H%M%S)"
DEPLOY_LOG="$LOG_DIR/${PROGRAM_NAME}-mainnet-simulate-${TS}.log"

log() {
    echo "[mainnet-simulate] $*"
    mkdir -p "$LOG_DIR"
    echo "$(date -Iseconds) $*" >> "$DEPLOY_LOG"
}

fmt_sol() {
    printf "%.9f" "$1"
}

# --- Resolve the deployer signer (Ledger URL or file). ---
if [[ "$DEPLOYER_KEYPAIR" == usb://* ]]; then
    log "Deployer signer : Ledger ($DEPLOYER_KEYPAIR)"
    log "Confirm every address on the Ledger screen before approving."
    DEPLOYER_PUBKEY=$(solana address --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | tail -n 1)
else
    if [[ ! -f "$DEPLOYER_KEYPAIR" ]]; then
        log "Deployer keypair not found: $DEPLOYER_KEYPAIR"
        exit 1
    fi
    log "Deployer signer : file ($DEPLOYER_KEYPAIR)"
    DEPLOYER_PUBKEY=$(solana-keygen pubkey "$DEPLOYER_KEYPAIR")
fi
log "Deployer wallet : $DEPLOYER_PUBKEY"

BALANCE=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Mainnet balance : $BALANCE SOL"

REQUIRED_BALANCE="3.0"
if awk "BEGIN {exit !($BALANCE < $REQUIRED_BALANCE)}"; then
    log "Insufficient mainnet balance. Need at least $REQUIRED_BALANCE SOL."
    log "(~2.5 SOL program rent for ~360 KB plus a safety margin; most is refundable on close.)"
    exit 1
fi

# Build if needed.
if [[ ! -f "$PROGRAM_SO" ]]; then
    if [[ ! -f "$PROGRAM_MANIFEST" ]]; then
        log "Program manifest not found: $PROGRAM_MANIFEST"
        log "Usage: $0 [--program <program_name>] [--keypair <path|ledger-url>] [--url <rpc_url>] [--yes]"
        exit 1
    fi
    log "Program binary not found. Building $PROGRAM_NAME..."
    cd "$PROJECT_ROOT"
    cargo build-sbf --arch v3 --manifest-path "$PROGRAM_MANIFEST"
fi

PROGRAM_SIZE=$(stat -f%z "$PROGRAM_SO" 2>/dev/null || stat -c%s "$PROGRAM_SO")
log "Program size   : $PROGRAM_SIZE bytes ($(echo "scale=2; $PROGRAM_SIZE / 1024" | bc) KB)"

# Generate a throwaway program keypair so the simulation NEVER touches the
# canonical program ID.
PROGRAM_KEYPAIR="$LOG_DIR/throwaway-program-keypair-mainnet-${TS}.json"
solana-keygen new --no-passphrase -s -o "$PROGRAM_KEYPAIR" > /dev/null 2>&1
PROGRAM_ID=$(solana-keygen pubkey "$PROGRAM_KEYPAIR")
log "Throwaway program ID: $PROGRAM_ID"

# --- Explicit mainnet confirmation. ---
if [[ "$CONFIRM" != "yes" ]]; then
    echo
    echo "[mainnet-simulate] You are about to spend REAL mainnet SOL on a throwaway deploy."
    echo "[mainnet-simulate]   Throwaway program : $PROGRAM_ID"
    echo "[mainnet-simulate]   Deployer          : $DEPLOYER_PUBKEY"
    echo "[mainnet-simulate]   RPC               : $SOLANA_URL"
    read -r -p "[mainnet-simulate] Type SIMULATE-MAINNET to continue: " response
    if [[ "$response" != "SIMULATE-MAINNET" ]]; then
        log "Aborted by operator."
        exit 1
    fi
fi

# Record pre-deploy balance.
BALANCE_BEFORE=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance before deploy: $BALANCE_BEFORE SOL"

# Deploy and capture output.
log "Deploying $PROGRAM_NAME throwaway to mainnet (this may take a minute)..."
solana program deploy "$PROGRAM_SO" \
    --program-id "$PROGRAM_KEYPAIR" \
    --keypair "$DEPLOYER_KEYPAIR" \
    --url "$SOLANA_URL" 2>&1 | tee -a "$DEPLOY_LOG"

# Record post-deploy balance.
BALANCE_AFTER=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance after deploy : $BALANCE_AFTER SOL"

# Compute actual cost from balance delta.
ACTUAL_DEPLOY_COST=$(echo "$BALANCE_BEFORE - $BALANCE_AFTER" | bc)
log "Actual deploy cost   : $(fmt_sol "$ACTUAL_DEPLOY_COST") SOL"
log "(balance delta includes all deployment transaction fees and rent)"

# PDA rent from chain (same cluster).
log ""
log "=== PDA Rent Exemption (mainnet) ==="
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
read -r -p "Close the throwaway mainnet program and recover rent? [y/N] " response
if [[ "$response" =~ ^[Yy]$ ]]; then
    log "Closing program $PROGRAM_ID..."
    solana program close "$PROGRAM_ID" \
        --keypair "$DEPLOYER_KEYPAIR" \
        --url "$SOLANA_URL" \
        --bypass-warning 2>&1 | tee -a "$DEPLOY_LOG"
    BALANCE_CLOSED=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
    log "Balance after close: $BALANCE_CLOSED SOL"
    RECOVERED=$(echo "$BALANCE_CLOSED - $BALANCE_AFTER" | bc)
    log "Recovered rent     : $(fmt_sol "$RECOVERED") SOL"
else
    log "Program left deployed on mainnet: $PROGRAM_ID"
    log "To close later: solana program close $PROGRAM_ID --keypair $DEPLOYER_KEYPAIR --url $SOLANA_URL"
fi

log "Full log saved to: $DEPLOY_LOG"
