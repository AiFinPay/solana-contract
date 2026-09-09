#!/usr/bin/env bash
# Deploy an Anchor/Solana program to devnet from a local project keypair.
#
# This script uses:
#   - keypairs/devnet-deployer.json   as the default deployer wallet
#   - target/deploy/<program>.so       as the program binary
#   - deployments/splitter_v14/         for deploy logs, program keypairs, and IDL snapshots
#
# Usage:
#   ./devnet-deploy.sh [--program <name>] [--keypair <path>] [--url <rpc_url>]
#
# Before running, fund the deployer:
#   solana airdrop 2 --keypair keypairs/devnet-deployer.json --url devnet

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
PROGRAM_IDL="$PROJECT_ROOT/target/idl/${PROGRAM_NAME}.json"
DEPLOYMENTS_DIR="$PROJECT_ROOT/deployments/splitter_v14"
TS="$(date +%Y%m%d-%H%M%S)"
DEPLOY_LOG="$DEPLOYMENTS_DIR/${PROGRAM_NAME}-deploy-${TS}.log"

log() {
    echo "[devnet-deploy] $*"
    mkdir -p "$DEPLOYMENTS_DIR"
    echo "$(date -Iseconds) $*" >> "$DEPLOY_LOG"
}

# Verify the deployer keypair exists.
if [[ ! -f "$DEPLOYER_KEYPAIR" ]]; then
    log "Deployer keypair not found: $DEPLOYER_KEYPAIR"
    log "Generate one with: solana-keygen new --no-passphrase -s -o $DEPLOYER_KEYPAIR"
    exit 1
fi

DEPLOYER_PUBKEY=$(solana-keygen pubkey "$DEPLOYER_KEYPAIR")
log "Deployer wallet : $DEPLOYER_PUBKEY"
log "Keypair path    : $DEPLOYER_KEYPAIR"

# Report deployer balance.
BALANCE=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Devnet balance  : $BALANCE SOL"

REQUIRED_BALANCE="0.05"
if awk "BEGIN {exit !($BALANCE < $REQUIRED_BALANCE)}"; then
    log "Insufficient devnet balance. Need at least $REQUIRED_BALANCE SOL."
    log "Run: solana airdrop 2 --keypair $DEPLOYER_KEYPAIR --url devnet"
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
log "Program size    : $PROGRAM_SIZE bytes ($(echo "scale=2; $PROGRAM_SIZE / 1024" | bc) KB)"

# Generate a fresh throwaway program keypair in the deployments directory.
PROGRAM_KEYPAIR="$DEPLOYMENTS_DIR/program-keypair-${TS}.json"
solana-keygen new --no-passphrase -s -o "$PROGRAM_KEYPAIR" > /dev/null 2>&1
PROGRAM_ID=$(solana-keygen pubkey "$PROGRAM_KEYPAIR")
log "Program ID      : $PROGRAM_ID"
log "Program keypair : $PROGRAM_KEYPAIR"

# Record balances before deploy.
BALANCE_BEFORE=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance before  : $BALANCE_BEFORE SOL"

# Deploy.
log "Deploying $PROGRAM_NAME to devnet..."
solana program deploy "$PROGRAM_SO" \
    --program-id "$PROGRAM_KEYPAIR" \
    --keypair "$DEPLOYER_KEYPAIR" \
    --url "$SOLANA_URL" 2>&1 | tee -a "$DEPLOY_LOG"

# Report balance after deploy.
BALANCE_AFTER=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance after   : $BALANCE_AFTER SOL"

ACTUAL_COST=$(echo "$BALANCE_BEFORE - $BALANCE_AFTER" | bc)
log "Deploy cost     : $(printf "%.9f" "$ACTUAL_COST") SOL"

# Snapshot the IDL with the deployed program address, following the
# deployments/splitter_v14 convention (splitter.devnet-<ts>.json).
IDL_SNAPSHOT="$DEPLOYMENTS_DIR/${PROGRAM_NAME}.devnet.${TS}.json"
if [[ -f "$PROGRAM_IDL" ]]; then
    if command -v python3 >/dev/null 2>&1; then
        python3 -c "import json,sys; p=sys.argv[1]; d=json.load(open(p)); d['address']='$PROGRAM_ID'; json.dump(d, open(sys.argv[2],'w'), indent=2)" \
            "$PROGRAM_IDL" "$IDL_SNAPSHOT"
        log "IDL snapshot    : $IDL_SNAPSHOT"
    else
        cp "$PROGRAM_IDL" "$IDL_SNAPSHOT"
        log "IDL snapshot    : $IDL_SNAPSHOT (address NOT patched: python3 missing)"
    fi
else
    log "IDL not found at $PROGRAM_IDL; skipping IDL snapshot."
fi

log ""
log "Next steps:"
log "  1. Initialize the program via Anchor/CLI with the deployer keypair."
log "  2. To close and recover rent later:"
log "     solana program close $PROGRAM_ID --keypair $DEPLOYER_KEYPAIR --url devnet"
log ""
log "Full log saved to: $DEPLOY_LOG"
