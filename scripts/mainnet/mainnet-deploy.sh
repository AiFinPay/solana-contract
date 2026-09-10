#!/usr/bin/env bash
# Deploy the canonical splitter program to MAINNET from a Ledger wallet.
#
# Mainnet differences vs scripts/devnet/devnet-deploy.sh:
#   - deployer defaults to a Ledger URL (`usb://ledger`), NOT a file keypair;
#     a file path may still be passed explicitly via --keypair;
#   - the program keypair is NEVER generated: it must be the canonical
#     keypairs/splitter-keypair.json matching declare_id! in lib.rs;
#   - aborts if the placeholder DEPLOYER constant is still present in
#     programs/splitter/src/constants.rs (initialize is gated to DEPLOYER,
#     so the Ledger address must be baked in BEFORE `cargo build-sbf`);
#   - rebuilds the SBF binary by default so the on-chain DEPLOYER matches
#     the source tree (override with --skip-build);
#   - requires typing DEPLOY-MAINNET to proceed (override with --yes).
#
# WARNING: real mainnet SOL is spent. Verify every address on the Ledger screen.
#
# Usage:
#   ./mainnet-deploy.sh [--program <name>] [--keypair <path|ledger-url>]
#                       [--deployer <base58-pubkey>] [--program-keypair <path>]
#                       [--url <rpc_url>] [--yes] [--skip-build]
#
# Before running:
#   1. Provide DEPLOYER via --deployer or SPLITTER_DEPLOYER env (preferred:
#      no code change — build.rs bakes it into the binary). Legacy manual
#      edit of programs/splitter/src/constants.rs still works.
#   2. Fund the Ledger wallet with at least ~3 SOL (program rent + margin).
#   3. Connect the Ledger, unlock it, and open the Solana app.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"
DEPLOY_DIR="$PROJECT_ROOT/target/deploy"

# Canonical program ID (must match declare_id! in programs/splitter/src/lib.rs
# and keypairs/splitter-keypair.json). NEVER deploy mainnet to any other ID.
CANONICAL_PROGRAM_ID="5QBJgMap7wuFsYfaU8Pmuu2i96GsJ3aBv6mMoUSaPoiS"

# Fingerprint of the placeholder DEPLOYER bytes in constants.rs. If these are
# still present, the binary would gate `initialize` to a throwaway address.
PLACEHOLDER_DEPLOYER_FP="0xDE, 0xA0, 0xD0, 0xBE"

# Defaults.
PROGRAM_NAME="splitter"
KEYPAIR_DIR="$PROJECT_ROOT/keypairs"
DEPLOYER_KEYPAIR="usb://ledger"
DEPLOYER_OVERRIDE="${SPLITTER_DEPLOYER:-}"
PROGRAM_KEYPAIR="$KEYPAIR_DIR/splitter-keypair.json"
SOLANA_URL="https://api.mainnet-beta.solana.com"
CONFIRM="ask"
SKIP_BUILD="false"

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
        --deployer)
            DEPLOYER_OVERRIDE="$2"
            shift 2
            ;;
        --program-keypair)
            PROGRAM_KEYPAIR="$2"
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
        --skip-build)
            SKIP_BUILD="true"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--program <program_name>] [--keypair <path|ledger-url>]"
            echo "           [--deployer <base58-pubkey>] [--program-keypair <path>]"
            echo "           [--url <rpc_url>] [--yes] [--skip-build]"
            echo "  --program         Program name under programs/ (default: splitter)"
            echo "  --keypair         Deployer signer: Ledger URL (default: usb://ledger)"
            echo "                    or a file keypair path, e.g. usb://ledger?key=0"
            echo "  --deployer        On-chain DEPLOYER baked into the binary (base58)."
            echo "                    Same as SPLITTER_DEPLOYER env. No code change needed."
            echo "                    Defaults to \$SPLITTER_DEPLOYER when the flag is absent."
            echo "  --program-keypair Canonical program keypair (default: keypairs/splitter-keypair.json)"
            echo "  --url             Solana cluster RPC URL (default: https://api.mainnet-beta.solana.com)"
            echo "  --yes             Skip the DEPLOY-MAINNET confirmation prompt"
            echo "  --skip-build      Do not rebuild the SBF binary before deploying"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--program <program_name>] [--keypair <path|ledger-url>] [--deployer <base58-pubkey>] [--url <rpc_url>] [--yes] [--skip-build]"
            exit 1
            ;;
    esac
done

PROGRAM_SO="$DEPLOY_DIR/${PROGRAM_NAME}.so"
PROGRAM_MANIFEST="$PROJECT_ROOT/programs/$PROGRAM_NAME/Cargo.toml"
PROGRAM_IDL="$PROJECT_ROOT/target/idl/${PROGRAM_NAME}.json"
LIB_RS="$PROJECT_ROOT/programs/$PROGRAM_NAME/src/lib.rs"
CONSTANTS_RS="$PROJECT_ROOT/programs/$PROGRAM_NAME/src/constants.rs"
DEPLOYMENTS_DIR="$PROJECT_ROOT/deployments/splitter_v14"
TS="$(date +%Y%m%d-%H%M%S)"
DEPLOY_LOG="$DEPLOYMENTS_DIR/${PROGRAM_NAME}-mainnet-deploy-${TS}.log"

log() {
    echo "[mainnet-deploy] $*"
    mkdir -p "$DEPLOYMENTS_DIR"
    echo "$(date -Iseconds) $*" >> "$DEPLOY_LOG"
}

# --- Guard 1: canonical program keypair must exist and match declare_id. ---
if [[ ! -f "$PROGRAM_KEYPAIR" ]]; then
    log "Canonical program keypair not found: $PROGRAM_KEYPAIR"
    log "Mainnet MUST deploy to $CANONICAL_PROGRAM_ID; generate or restore"
    log "the canonical keypair first (never commit it to git)."
    exit 1
fi

PROGRAM_ID=$(solana-keygen pubkey "$PROGRAM_KEYPAIR")
log "Program ID (keypair file): $PROGRAM_ID"
if [[ "$PROGRAM_ID" != "$CANONICAL_PROGRAM_ID" ]]; then
    log "ABORT: program keypair does not match the canonical program ID."
    log "  keypair : $PROGRAM_ID"
    log "  expected: $CANONICAL_PROGRAM_ID"
    exit 1
fi

if ! grep -q "declare_id!(\"$CANONICAL_PROGRAM_ID\")" "$LIB_RS"; then
    log "ABORT: declare_id! in $LIB_RS does not match $CANONICAL_PROGRAM_ID."
    log "The binary would derive different PDAs. Fix declare_id first."
    exit 1
fi
log "Canonical program ID confirmed: $CANONICAL_PROGRAM_ID"

# --- Guard 2: DEPLOYER must be real (flag/env override or manual edit). ---
# Preferred path (no code change): --deployer <base58> or SPLITTER_DEPLOYER
# env. build.rs bakes it into the binary at build time. Legacy path: a
# manual constants.rs edit that removed the placeholder bytes.
if [[ -n "$DEPLOYER_OVERRIDE" ]]; then
    export SPLITTER_DEPLOYER="$DEPLOYER_OVERRIDE"
    if [[ "$SPLITTER_DEPLOYER" == "11111111111111111111111111111111" ]]; then
        log "ABORT: SPLITTER_DEPLOYER is the default (all-zero) pubkey."
        exit 1
    fi
    log "DEPLOYER override : $SPLITTER_DEPLOYER (baked via build.rs, no code change)"
elif grep -q "$PLACEHOLDER_DEPLOYER_FP" "$CONSTANTS_RS"; then
    log "ABORT: placeholder DEPLOYER bytes still present in $CONSTANTS_RS."
    log "Pass the real Ledger (or multisig) address without editing code:"
    log "  $0 --deployer <BASE58_PUBKEY> --keypair \"$DEPLOYER_KEYPAIR\""
    log "or:"
    log "  SPLITTER_DEPLOYER=<BASE58_PUBKEY> $0 --keypair \"$DEPLOYER_KEYPAIR\""
    log "Legacy alternative: edit DEPLOYER in $CONSTANTS_RS, then rebuild:"
    log "  cargo build-sbf --manifest-path $PROGRAM_MANIFEST"
    exit 1
else
    log "DEPLOYER placeholder absent from constants.rs (verify the address manually!)."
fi

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
if [[ -n "${SPLITTER_DEPLOYER:-}" && "$SPLITTER_DEPLOYER" != "$DEPLOYER_PUBKEY" ]]; then
    log "WARNING: baked DEPLOYER ($SPLITTER_DEPLOYER) != signer wallet ($DEPLOYER_PUBKEY)."
    log "WARNING: initialize must be signed by the baked DEPLOYER. Mismatch is only"
    log "WARNING: valid for a multisig/authority-split setup — double-check before proceeding."
fi

# --- Balance check (by address, so Ledgers are not prompted). ---
BALANCE=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Mainnet balance : $BALANCE SOL"

REQUIRED_BALANCE="3.0"
if awk "BEGIN {exit !($BALANCE < $REQUIRED_BALANCE)}"; then
    log "Insufficient mainnet balance. Need at least $REQUIRED_BALANCE SOL"
    log "(~2.5 SOL program rent for ~360 KB plus a safety margin)."
    exit 1
fi

# --- Build (default: always rebuild so DEPLOYER/declare_id are baked in). ---
if [[ "$SKIP_BUILD" == "true" ]]; then
    log "Skipping build (--skip-build). Using existing binary (verify freshness!)."
    if [[ ! -f "$PROGRAM_SO" ]]; then
        log "Program binary not found: $PROGRAM_SO"
        exit 1
    fi
else
    if [[ ! -f "$PROGRAM_MANIFEST" ]]; then
        log "Program manifest not found: $PROGRAM_MANIFEST"
        exit 1
    fi
    log "Building $PROGRAM_NAME (SBF)..."
    cd "$PROJECT_ROOT"
    if [[ -n "${SPLITTER_DEPLOYER:-}" ]]; then
        log "Baking SPLITTER_DEPLOYER=$SPLITTER_DEPLOYER into the binary."
        SPLITTER_DEPLOYER="$SPLITTER_DEPLOYER" cargo build-sbf --manifest-path "$PROGRAM_MANIFEST"
    else
        cargo build-sbf --manifest-path "$PROGRAM_MANIFEST"
    fi
fi

PROGRAM_SIZE=$(stat -f%z "$PROGRAM_SO" 2>/dev/null || stat -c%s "$PROGRAM_SO")
log "Program size    : $PROGRAM_SIZE bytes ($(echo "scale=2; $PROGRAM_SIZE / 1024" | bc) KB)"

# --- Upgrade detection: deploying over an existing program is an UPGRADE. ---
if solana program show "$PROGRAM_ID" --url "$SOLANA_URL" >/dev/null 2>&1; then
    UPGRADE_AUTHORITY=$(solana program show "$PROGRAM_ID" --url "$SOLANA_URL" 2>&1 | grep -i "upgrade authority" | awk '{print $NF}' || true)
    log "WARNING: program $PROGRAM_ID already exists on mainnet — this is an UPGRADE."
    log "On-chain upgrade authority: ${UPGRADE_AUTHORITY:-<unknown>}"
    if [[ -n "$UPGRADE_AUTHORITY" && "$UPGRADE_AUTHORITY" != "$DEPLOYER_PUBKEY" ]]; then
        log "ABORT: deployer $DEPLOYER_PUBKEY is not the on-chain upgrade authority."
        exit 1
    fi
else
    log "No existing program at $PROGRAM_ID — this is a FRESH deploy."
fi

BALANCE_BEFORE=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance before  : $BALANCE_BEFORE SOL"

# --- Explicit mainnet confirmation. ---
if [[ "$CONFIRM" != "yes" ]]; then
    echo
    echo "[mainnet-deploy] You are about to deploy $PROGRAM_NAME to MAINNET."
    echo "[mainnet-deploy]   Program : $PROGRAM_ID"
    echo "[mainnet-deploy]   Deployer: $DEPLOYER_PUBKEY"
    echo "[mainnet-deploy]   RPC     : $SOLANA_URL"
    read -r -p "[mainnet-deploy] Type DEPLOY-MAINNET to continue: " response
    if [[ "$response" != "DEPLOY-MAINNET" ]]; then
        log "Aborted by operator."
        exit 1
    fi
fi

# --- Deploy. ---
log "Deploying $PROGRAM_NAME to mainnet..."
solana program deploy "$PROGRAM_SO" \
    --program-id "$PROGRAM_KEYPAIR" \
    --keypair "$DEPLOYER_KEYPAIR" \
    --url "$SOLANA_URL" 2>&1 | tee -a "$DEPLOY_LOG"

# --- Report balance after deploy. ---
BALANCE_AFTER=$(solana balance "$DEPLOYER_PUBKEY" --url "$SOLANA_URL" 2>&1 | awk '{print $1}')
log "Balance after   : $BALANCE_AFTER SOL"

ACTUAL_COST=$(echo "$BALANCE_BEFORE - $BALANCE_AFTER" | bc)
log "Deploy cost     : $(printf "%.9f" "$ACTUAL_COST") SOL"

# Snapshot the IDL with the canonical program address, following the
# deployments/splitter_v14 convention (splitter.mainnet-<ts>.json).
IDL_SNAPSHOT="$DEPLOYMENTS_DIR/${PROGRAM_NAME}.mainnet.${TS}.json"
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
log "  1. Initialize (payer must be the on-chain DEPLOYER):"
log "     DEPLOYER_KEYPAIR_PATH=<deployer-key> npm run initialize:mainnet"
log "     If DEPLOYER lives on the Ledger, sign initialize with a Ledger-capable"
log "     client instead — see scripts/mainnet/README.md."
log "  2. Configure routes: npm run configure:mainnet"
log "  3. Verify: npm run check:mainnet"
log "  4. To close and recover rent later (upgrade authority signs):"
log "     solana program close $PROGRAM_ID --keypair $DEPLOYER_KEYPAIR --url $SOLANA_URL"
log ""
log "Full log saved to: $DEPLOY_LOG"
