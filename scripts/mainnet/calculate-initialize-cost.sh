#!/usr/bin/env bash
# Initialize cost calculator for the splitter program (MAINNET).
# Mirror of scripts/devnet/calculate-initialize-cost.sh pointed at mainnet-beta.
# Estimates the price of the `initialize` instruction: rent-exempt minimums
# for the three singleton PDAs it creates (paid by the payer/deployer) plus
# the transaction fee. Read-only: spends no SOL.
#
# Usage:
#   ./calculate-initialize-cost.sh [--url <rpc_url>]
#
# The account sizes must match the on-chain layouts in
# programs/splitter/src/instructions/initialize.rs (`space = 8 + T::INIT_SPACE`):
#   Config:        8 + (32 + 64 + 32 + 32 + 32 + 32 + 1 + 1) = 234
#   TokenList:     8 + (32 + 4 + 16 * 32 + 1) = 557
#   ProfilesIndex: 8 + (4 + 32 * 77 + 1 + 1) = 2478
#     (RouteProfileEntry = 32 + 2 + 2 + 1 + 8 + 32 = 77)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$(dirname "$SCRIPT_DIR")")"

# Defaults.
SOLANA_URL="https://api.mainnet-beta.solana.com"

# Parse arguments.
while [[ $# -gt 0 ]]; do
    case "$1" in
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
            echo "Usage: $0 [--url <rpc_url>]"
            echo "  --url       Solana cluster RPC URL (default: https://api.mainnet-beta.solana.com)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--url <rpc_url>]"
            exit 1
            ;;
    esac
done

# A single `initialize` transaction carries one signature, so the base fee is
# 5000 lamports. Real fees may be higher with prioritization / congestion.
TX_FEE_SOL="0.000005"

log() {
    echo "[mainnet-init-cost] $*"
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

# PDA sizes must match the on-chain account layouts (see header comment).
CONFIG_SIZE=234
TOKEN_LIST_SIZE=557
PROFILES_SIZE=2478

CONFIG_RENT=$(compute_rent "$CONFIG_SIZE")
TOKEN_LIST_RENT=$(compute_rent "$TOKEN_LIST_SIZE")
PROFILES_RENT=$(compute_rent "$PROFILES_SIZE")

log "=== splitter Initialize Cost Calculation (MAINNET) ==="
log "Config       ($CONFIG_SIZE bytes)      : $(fmt_sol "$CONFIG_RENT") SOL"
log "TokenList    ($TOKEN_LIST_SIZE bytes)      : $(fmt_sol "$TOKEN_LIST_RENT") SOL"
log "ProfilesIndex($PROFILES_SIZE bytes)     : $(fmt_sol "$PROFILES_RENT") SOL"

echo
RENT_TOTAL=$(echo "$CONFIG_RENT + $TOKEN_LIST_RENT + $PROFILES_RENT" | bc)
log "Total PDA rent                        : $(fmt_sol "$RENT_TOTAL") SOL"
log "Rent calculation: $(fmt_sol "$CONFIG_RENT") + $(fmt_sol "$TOKEN_LIST_RENT") + $(fmt_sol "$PROFILES_RENT") = $(fmt_sol "$RENT_TOTAL") SOL"

echo
TOTAL=$(echo "$RENT_TOTAL + $TX_FEE_SOL" | bc)
log "=== Total Initialize Price ==="
log "PDA rent (refundable on close) : $(fmt_sol "$RENT_TOTAL") SOL"
log "TX fee (non-refundable)        : $(fmt_sol "$TX_FEE_SOL") SOL"
log "TOTAL                          : $(fmt_sol "$TOTAL") SOL"
log "Calculation: $(fmt_sol "$RENT_TOTAL") + $(fmt_sol "$TX_FEE_SOL") = $(fmt_sol "$TOTAL") SOL"

echo
log "Notes:"
log "  • Paid up-front by the payer (must be the on-chain DEPLOYER) in a single initialize tx."
log "  • PDA rent is locked as rent-exempt and refundable when accounts are closed."
log "  • Transaction fee is non-refundable; actual fee may vary with congestion"
log "    and priority fees."
log "  • This script queries $SOLANA_URL via 'solana rent'; fallback uses a manual 2-year estimate."
