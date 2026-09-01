#!/usr/bin/env bash
# Deploy cost calculator for Solana programs
# Calculates rent for PDA accounts and program deployment fees

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

echo "=============================================="
echo "  Solana Program Deploy Cost Calculator"
echo "=============================================="
echo

# Build programs if not already built
if [ ! -f "$PROJECT_ROOT/target/deploy/splitter.so" ]; then
    echo "Building splitter..."
    cd "$PROJECT_ROOT"
    cargo build-sbf --features splitter
fi

if [ ! -f "$PROJECT_ROOT/target/deploy/splitter_light.so" ]; then
    echo "Building splitter_light..."
    cd "$PROJECT_ROOT"
    cargo build-sbf --features splitter_light
fi

echo

# Get program sizes
SPLITTER_SIZE=$(stat -f%z "$PROJECT_ROOT/target/deploy/splitter.so" 2>/dev/null || stat -c%s "$PROJECT_ROOT/target/deploy/splitter.so")
LIGHT_SIZE=$(stat -f%z "$PROJECT_ROOT/target/deploy/splitter_light.so" 2>/dev/null || stat -c%s "$PROJECT_ROOT/target/deploy/splitter_light.so")

echo "Program sizes:"
echo "  splitter.so:      $SPLITTER_SIZE bytes ($(echo "scale=2; $SPLITTER_SIZE/1024" | bc) KB)"
echo "  splitter_light.so: $LIGHT_SIZE bytes ($(echo "scale=2; $LIGHT_SIZE/1024" | bc) KB)"
echo

# Calculate deployment fees (chunks of 1024 bytes)
SPLITTER_CHUNKS=$(( (SPLITTER_SIZE / 1024) + 1 ))
LIGHT_CHUNKS=$(( (LIGHT_SIZE / 1024) + 1 ))

# Fee per chunk ~0.000005 SOL
SPLITTER_DEPLOY_FEE=$(echo "scale=6; $SPLITTER_CHUNKS * 0.000005" | bc)
LIGHT_DEPLOY_FEE=$(echo "scale=6; $LIGHT_CHUNKS * 0.000005" | bc)

echo "Deployment fees:"
echo "  splitter:         $SPLITTER_CHUNKS chunks = $SPLITTER_DEPLOY_FEE SOL"
echo "  splitter_light:   $LIGHT_CHUNKS chunks = $LIGHT_DEPLOY_FEE SOL"
echo

# Calculate rent for PDA accounts using solana CLI
echo "PDA Rent exemption (via solana rent):"
echo "-------------------------------------------"

# Config: 8 + 32 + 64 + 32 + 32 + 32 + 32 + 1 + 1 = 234 bytes
CONFIG_SIZE=234
CONFIG_RENT=$(solana rent $CONFIG_SIZE 2>&1 | grep "Rent-exempt minimum" | awk '{print $3}')
echo "  Config ($CONFIG_SIZE bytes):     $CONFIG_RENT SOL"

# TokenList max: 8 + 32 + 4 + (20 * 32) + 1 = 685 bytes
TOKEN_LIST_SIZE=685
TOKEN_LIST_RENT=$(solana rent $TOKEN_LIST_SIZE 2>&1 | grep "Rent-exempt minimum" | awk '{print $3}')
echo "  TokenList ($TOKEN_LIST_SIZE bytes):   $TOKEN_LIST_RENT SOL"

# ProfilesIndex max: 8 + 4 + (10 * 77) + 1 + 1 = 793 bytes
# RouteProfileEntry: 32 + 2 + 2 + 1 + 8 + 32 = 77 bytes
PROFILES_SIZE=793
PROFILES_RENT=$(solana rent $PROFILES_SIZE 2>&1 | grep "Rent-exempt minimum" | awk '{print $3}')
echo "  ProfilesIndex ($PROFILES_SIZE bytes): $PROFILES_RENT SOL"

echo "-------------------------------------------"

# Calculate total rent (parse SOL values)
CONFIG_RENT_NUM=$(echo "$CONFIG_RENT" | tr -d 'SOL')
TOKEN_LIST_RENT_NUM=$(echo "$TOKEN_LIST_RENT" | tr -d 'SOL')
PROFILES_RENT_NUM=$(echo "$PROFILES_RENT" | tr -d 'SOL')

TOTAL_RENT=$(echo "$CONFIG_RENT_NUM + $TOKEN_LIST_RENT_NUM + $PROFILES_RENT_NUM" | bc)

echo "  TOTAL PDA RENT:   $TOTAL_RENT SOL"
echo

# Final totals
echo "=============================================="
echo "  TOTAL DEPLOY COST"
echo "=============================================="
SPLITTER_TOTAL=$(echo "$SPLITTER_DEPLOY_FEE + $TOTAL_RENT" | bc)
echo "  splitter:         $SPLITTER_TOTAL SOL"
echo "                    (deploy: $SPLITTER_DEPLOY_FEE + rent: $TOTAL_RENT)"
echo
echo "  splitter_light:   $LIGHT_DEPLOY_FEE SOL"
echo "                    (no PDA initialization)"
echo "=============================================="
echo
echo "Note: PDA rent is refundable upon account closure."
echo "      Program deployment fees are non-refundable."
