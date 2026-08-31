#!/usr/bin/env bash
# Rebuild settlement-v1 and compare the artifact against BUILD-EVIDENCE.md.
# Fails closed: any missing tool, missing evidence, or hash mismatch = exit 1.
# cargo-build-sbf ships its own pinned rustc (platform-tools), so the hash is
# expected to reproduce across hosts — unlike a host-rustc build (see the NEAR
# lesson: toolchain + host + paths all mattered there).
set -euo pipefail
cd "$(dirname "$0")/.."

EVIDENCE=BUILD-EVIDENCE.md
[ -s "$EVIDENCE" ] || { echo "✗ $EVIDENCE missing"; exit 1; }
EXPECTED=$(grep -Eo '^sha256: [0-9a-f]{64}' "$EVIDENCE" | awk '{print $2}')
[ -n "$EXPECTED" ] || { echo "✗ no 'sha256: <hash>' line in $EVIDENCE"; exit 1; }
command -v cargo-build-sbf >/dev/null || { echo "✗ cargo-build-sbf not installed"; exit 1; }

echo "toolchain: $(cargo-build-sbf --version 2>&1 | tr '\n' ' ')"
cargo-build-sbf

SO=target/deploy/aifinpay_settlement_v1.so
[ -s "$SO" ] || { echo "✗ artifact missing"; exit 1; }
ACTUAL=$(shasum -a 256 "$SO" 2>/dev/null | awk '{print $1}') || ACTUAL=$(sha256sum "$SO" | awk '{print $1}')
SIZE=$(wc -c < "$SO" | tr -d ' ')
echo "built:    $SIZE bytes  sha256 $ACTUAL"
echo "expected: sha256 $EXPECTED"
if [ "$ACTUAL" != "$EXPECTED" ]; then
  echo "✗ MISMATCH — artifact does not reproduce the recorded build"
  exit 1
fi
echo "✓ byte-identical to recorded evidence"
