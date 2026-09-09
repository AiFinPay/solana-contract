#!/usr/bin/env bash
# Deploy an Anchor/Solana program to localnet from a local project keypair.
#
# Mirrors scripts/devnet-deploy.sh, but targets a local validator:
#   - keypairs/localnet-deployer.json (falls back to ~/.config/solana/id.json)
#   - target/deploy/<program>.so       as the program binary
#   - deployments/splitter_v14/         for deploy logs, program keypairs, and IDL snapshots
#
# Usage:
#   ./localnet-deploy.sh [--program <name>] [--keypair <path>] [--url <rpc_url>]
#
# Prerequisite: local validator running, e.g.:
#   solana-test-validator --reset
# Before running, fund the deployer (localnet airdrop is free):
#   solana airdrop 100 --keypair keypairs/localnet-deployer.json --url localhost

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
DEPLOY_DIR="$PROJECT_ROOT/target/deploy"

# Defaults.
PROGRAM_NAME="splitter"
KEYPAIR_DIR="$PROJECT_ROOT/keypairs"
DEPLOYER_KEYPAIR="$KEYPAIR_DIR/localnet-deployer.json"
SOLANA_URL="http://127.0.0.1:8899"

# Fall back to default Solana CLI keypair if no localnet deployer exists.
if [[ ! -f "$DEPLOYER_KEYPAIR" && -f "$HOME/.config/solana/id.json" ]]; then
    DEPLOYER_KEYPAIR="$HOME/.config/solana/id.json"
fi

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
            echo "  --keypair   Path to deployer keypair (default: keypairs/localnet-deployer.json)"
            echo "  --url       Solana cluster RPC URL (default: http://127.0.0.1:8899)"
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
DEPLOY_LOG="$DEPLOYMENTS_DIR/${PROGRAM_NAME}-localnet-deploy-${TS}.log"

log() {
    echo "[localnet-deploy] $*"
    mkdir -p "$DEPLOYMENTS_DIR"
    echo "$(date -Iseconds) $*" >> "$DEPLOY_LOG"
}

# Warn if no local validator is reachable.
if ! solana cluster-version --url "$SOLANA_URL" >/dev/null 2>&1; then
    log "No validator reachable at $SOLANA_URL."
    log "Start one with: solana-test-validator --reset"
    exit 1
fi

# Verify the deployer keypair exists.
if [[ ! -f "$DEPLOYER_KEYPAIR" ]]; then
    log "Deployer keypair not found: $DEPLOYER_KEYPAIR"
    log "Generate one with: solana-keygen new --no-passphrase -s -o $DEPLOYER_KEYPAIR"
    exit 1
fi

DEPLOYER_PUBKEY=$(solana-keygen pubkey "$DEPLOYER_KEYPAIR")
log "Deployer wallet : $DEPLOYER_PUBKEY"
log "Keypair path    : $DEPLOYER_KEYPAIR"

# Report deployer balance (auto-airdrop on localnet if low).
BALANCE=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Localnet balance: $BALANCE SOL"

REQUIRED_BALANCE="0.05"
if awk "BEGIN {exit !($BALANCE < $REQUIRED_BALANCE)}"; then
    log "Low balance, requesting localnet airdrop..."
    solana airdrop 10 --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | tee -a "$DEPLOY_LOG" || true
    BALANCE=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
    log "Balance after airdrop: $BALANCE SOL"
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
PROGRAM_KEYPAIR="$DEPLOYMENTS_DIR/program-keypair-localnet-${TS}.json"
solana-keygen new --no-passphrase -s -o "$PROGRAM_KEYPAIR" > /dev/null 2>&1
PROGRAM_ID=$(solana-keygen pubkey "$PROGRAM_KEYPAIR")
log "Program ID      : $PROGRAM_ID"
log "Program keypair : $PROGRAM_KEYPAIR"

# Record balances before deploy.
BALANCE_BEFORE=$(solana balance --keypair "$DEPLOYER_KEYPAIR" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance before  : $BALANCE_BEFORE SOL"

# Deploy.
log "Deploying $PROGRAM_NAME to localnet..."
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
# deployments/splitter_v14 convention (splitter.localnet-<ts>.json).
IDL_SNAPSHOT="$DEPLOYMENTS_DIR/${PROGRAM_NAME}.localnet.${TS}.json"
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
log "     solana program close $PROGRAM_ID --keypair $DEPLOYER_KEYPAIR --url localhost"
log ""
log "Full log saved to: $DEPLOY_LOG"
