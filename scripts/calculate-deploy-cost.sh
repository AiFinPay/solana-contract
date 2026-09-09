#!/usr/bin/env bash
# Deploy cost calculator for an Anchor/Solana program.
# Queries the Solana cluster for rent-exempt minimums and estimates total
# deployment cost including program data account rent, transaction fees, and
# PDA rent.
#
# Usage:
#   ./calculate-deploy-cost.sh [--program <name>] [--url <rpc_url>]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DEPLOY_DIR="$PROJECT_ROOT/target/deploy"

# Defaults.
PROGRAM_NAME="splitter"
SOLANA_URL="https://api.devnet.solana.com"

# Parse arguments.
while [[ $# -gt 0 ]]; do
    case "$1" in
        --program)
            PROGRAM_NAME="$2"
            shift 2
            ;;
        --url)
            SOLANA_URL="$2"
            shift 2
            ;;
        --keypair)
            # Accepted for consistency with deploy scripts; this estimate script
            # does not perform transactions, so the keypair is not used.
            shift 2
            ;;
        -h|--help)
            echo "Usage: $0 [--program <program_name>] [--url <rpc_url>]"
            echo "  --program   Program name under programs/ (default: splitter)"
            echo "  --url       Solana cluster RPC URL (default: https://api.devnet.solana.com)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--program <program_name>] [--url <rpc_url>]"
            exit 1
            ;;
    esac
done

PROGRAM_SO="$DEPLOY_DIR/${PROGRAM_NAME}.so"
PROGRAM_MANIFEST="$PROJECT_ROOT/programs/$PROGRAM_NAME/Cargo.toml"

# Solana loader constants. Each deployment transaction writes up to 10 KB
# (LOADER_CHUNK_SIZE bytes) and pays a small fee per transaction. This is a
# rough historical approximation; real fees depend on the cluster fee market.
LOADER_CHUNK_SIZE=10240
FEE_PER_CHUNK_SOL="0.000005"

log() {
    echo "[deploy-cost] $*"
}

fmt_sol() {
    printf "%.9f" "$1"
}

# Compute rent-exempt minimum for a given account size by calling `solana rent`.
compute_rent() {
    local size=$1
    local rent_sol
    if command -v solana >/dev/null 2>&1; then
        rent_sol=$(solana rent "$size" --url "$SOLANA_URL" 2>&1 | grep "Rent-exempt minimum" | awk '{print $3}' || true)
        if [[ -n "$rent_sol" ]]; then
            echo "$rent_sol"
            return
        fi
    fi
    # Fallback: manual 2-year estimate at 3480 lamports/byte/year.
    echo "scale=9; ($size * 3480 * 2) / 1000000000" | bc
}

# Ensure the program binary exists.
if [[ ! -f "$PROGRAM_SO" ]]; then
    if [[ ! -f "$PROGRAM_MANIFEST" ]]; then
        log "Program manifest not found: $PROGRAM_MANIFEST"
        log "Usage: $0 [--program <program_name>] [--url <rpc_url>]"
        exit 1
    fi
    log "Program binary not found. Building $PROGRAM_NAME..."
    cd "$PROJECT_ROOT"
    cargo build-sbf --manifest-path "$PROGRAM_MANIFEST"
fi

PROGRAM_SIZE=$(stat -f%z "$PROGRAM_SO" 2>/dev/null || stat -c%s "$PROGRAM_SO")
PROGRAM_SIZE_KB=$(echo "scale=2; $PROGRAM_SIZE / 1024" | bc)

# Ceiling division for chunks.
CHUNKS=$(( (PROGRAM_SIZE + LOADER_CHUNK_SIZE - 1) / LOADER_CHUNK_SIZE ))
TX_FEE_SOL=$(echo "scale=9; $CHUNKS * $FEE_PER_CHUNK_SOL" | bc)

# Program data account rent is the dominant cost. The program data account
# stores the ELF binary on-chain and must be rent-exempt. Its size is roughly
# the ELF size plus BPF loader account overhead (use PROGRAM_SIZE as a lower
# bound; actual deployed size may be slightly larger).
PROGRAM_RENT=$(compute_rent "$PROGRAM_SIZE")

log "=== ${PROGRAM_NAME} Deploy Cost Calculation ==="
log "Program binary     : $PROGRAM_SO"
log "Program size       : $PROGRAM_SIZE bytes (${PROGRAM_SIZE_KB} KB)"
log "Loader chunk size  : $LOADER_CHUNK_SIZE bytes"
log "Chunks required    : $CHUNKS"
log "Fee per chunk      : $FEE_PER_CHUNK_SOL SOL"
log "TX fees only       : $(fmt_sol "$TX_FEE_SOL") SOL"
log "TX fee calculation : $CHUNKS * $FEE_PER_CHUNK_SOL = $(fmt_sol "$TX_FEE_SOL") SOL"

echo
log "=== Program Data Rent Exemption ==="
log "Program data account size : $PROGRAM_SIZE bytes"
log "Program data rent           : $(fmt_sol "$PROGRAM_RENT") SOL"
log "(dominant deploy cost; must be paid up-front and is refundable on close)"

echo
log "=== PDA Rent Exemption ==="

# PDA sizes must match the on-chain account layouts.
# Config: 8 (discriminator) + 32 (admin) + 64 (signer) + 32 (pauser) +
#         32 (treasury) + 32 (token_list) + 32 (profiles) + 1 (bump) + 1 (is_paused) = 234
CONFIG_SIZE=234
# TokenList: 8 + 32 (admin) + 4 (vec len) + 16 * 32 (tokens) + 1 (bump) = 557
TOKEN_LIST_SIZE=557
# ProfilesIndex: 8 + (4 (vec len) + 32 * 77 (entries) + 1 (count) + 1 (bump)) = 2478
# RouteProfileEntry: 32 (route_id) + 2 (treasury_bps) + 2 (ip_creator_bps) +
#                    1 (enabled) + 8 (configured_at) + 32 (route_treasury) = 77
PROFILES_SIZE=2478

CONFIG_RENT=$(compute_rent "$CONFIG_SIZE")
TOKEN_LIST_RENT=$(compute_rent "$TOKEN_LIST_SIZE")
PROFILES_RENT=$(compute_rent "$PROFILES_SIZE")

log "Config       ($CONFIG_SIZE bytes)     : $(fmt_sol "$CONFIG_RENT") SOL"
log "TokenList    ($TOKEN_LIST_SIZE bytes) : $(fmt_sol "$TOKEN_LIST_RENT") SOL"
log "ProfilesIndex($PROFILES_SIZE bytes)  : $(fmt_sol "$PROFILES_RENT") SOL"

PDA_TOTAL_RENT=$(echo "$CONFIG_RENT + $TOKEN_LIST_RENT + $PROFILES_RENT" | bc)
log "Total PDA rent                        : $(fmt_sol "$PDA_TOTAL_RENT") SOL"
log "PDA rent calculation : $(fmt_sol "$CONFIG_RENT") + $(fmt_sol "$TOKEN_LIST_RENT") + $(fmt_sol "$PROFILES_RENT") = $(fmt_sol "$PDA_TOTAL_RENT") SOL"

echo
TOTAL_DEPLOY=$(echo "$PROGRAM_RENT + $TX_FEE_SOL" | bc)
TOTAL_LAUNCH=$(echo "$TOTAL_DEPLOY + $PDA_TOTAL_RENT" | bc)
log "=== Total Deploy Cost ==="
log "Program data rent : $(fmt_sol "$PROGRAM_RENT") SOL"
log "TX fees only      : $(fmt_sol "$TX_FEE_SOL") SOL"
log "Deploy subtotal   : $(fmt_sol "$TOTAL_DEPLOY") SOL"
log "Deploy calculation: $(fmt_sol "$PROGRAM_RENT") + $(fmt_sol "$TX_FEE_SOL") = $(fmt_sol "$TOTAL_DEPLOY") SOL"

echo
log "=== Total Estimated Launch Cost (deploy + PDAs) ==="
log "PDA rent      : $(fmt_sol "$PDA_TOTAL_RENT") SOL"
log "Deploy cost   : $(fmt_sol "$TOTAL_DEPLOY") SOL"
log "TOTAL         : $(fmt_sol "$TOTAL_LAUNCH") SOL"
log "Calculation   : $(fmt_sol "$PDA_TOTAL_RENT") + $(fmt_sol "$TOTAL_DEPLOY") = $(fmt_sol "$TOTAL_LAUNCH") SOL"

echo
log "Notes:"
log "  • Program data rent and PDA rent are refundable when accounts are closed."
log "  • Transaction fees are non-refundable."
log "  • Actual transaction fees may vary based on cluster congestion."
log "  • This script queries $SOLANA_URL via 'solana rent'; fallback uses a manual 2-year estimate."
log "  • The program data account size in reality may be slightly larger than the ELF size."


