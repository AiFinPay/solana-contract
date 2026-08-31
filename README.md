# AiFinPay Solana Splitter

Signed, multi-route gross settlement Anchor program on Solana.

## Build

```bash
cargo build-sbf
```

## Test

```bash
cargo test
```

## Lint

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Program ID

```
G6neYBZe8AzMvNPcewBhmzLao4CtZqc2uPVzPQBbAdrT
```

## Overview

Single Anchor v1.1.2 program with two settlement routes (ROUTE_AGENT_X402, ROUTE_MERCHANT_AIFP1). Uses secp256k1 ecrecover for signer verification. Tests run via `litesvm`, not Anchor's JS harness.
