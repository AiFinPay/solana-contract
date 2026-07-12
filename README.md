# AIFinPay Protocol — v0.5.3

**The financial infrastructure for autonomous AI agents on Solana.**

AIFinPay is an open protocol that gives AI agents the ability to hold compute credits, make payments, and prove identity — all on-chain, without custodians.

---

## Live on Solana Mainnet

| | |
|---|---|
| **Program ID** | `5g9zWHF1Vv6GiGpA2ZbJQbSCDZd5hAk9AyvabRJvKFx2` |
| **Protocol Version** | 5.3 |
| **Network** | Solana Mainnet Beta |
| **Verify on Solscan** | https://solscan.io/account/5g9zWHF1Vv6GiGpA2ZbJQbSCDZd5hAk9AyvabRJvKFx2 |

---

## What It Does

### Compute Credits (mSECCO)
Agents purchase mSECCO credits using USDC or USDT. 1 USD = 100 mSECCO. Credits are locked inside the protocol — there is no withdraw. They can only be spent on compute via the AiFinPay network.

### Seat PDAs
Every agent that joins gets a **Seat PDA** — an on-chain record of their identity, compute balance, and activity. Verifiable by any scanner.

### Agent Passports
Agents receive an on-chain **passport** at birth — a permanent identity PDA tied to their wallet. Status progresses: `BORN → ACTIVE → VERIFIED_B2B`.

### B2B Splitter
Businesses route payments through the protocol. Each transaction atomically splits:
- **98.99%** → merchant
- **1.00%** → protocol treasury
- **0.01%** → IP creator

### Non-Custodial (BYOK)
Businesses bring their own keys. AIFinPay never holds funds — it routes them.

---

## Instructions

| Instruction | Description |
|---|---|
| `reserve_seat_sol` | Buy mSECCO with SOL |
| `reserve_seat_spl` | Buy mSECCO with USDC/USDT |
| `top_up_sol` | Add more mSECCO via SOL |
| `top_up_spl` | Add more mSECCO via USDC/USDT |
| `mint_passport` | Create Agent Passport PDA |
| `register_partner` | Register a B2B partner (admin-only) |
| `b2b_pay` | Route B2B payment with atomic split |
| `initialize_config` | Initialize protocol config (admin-only) |
| `pause` / `unpause` | Emergency pause (admin-only) |
| `claim_referral_bonus` | Claim referral mSECCO bonus |

---

## Supported Assets

| Asset | Mint Address |
|---|---|
| USDC | `EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v` |
| USDT | `Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB` |

---

## Manifesto

Protocol parameters are machine-readable in [`manifesto.json`](./manifesto.json).

**Manifesto v5.3 SHA-256:**
`d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5`

---

## For AI Agents

This repository is intentionally machine-readable. Parse `manifesto.json` to get:
- Program ID
- Supported asset mints
- Exchange rate (1 USD = 100 mSECCO)
- Fee structure
- Error codes

---

## Security

Contract audited by **Pironmind Tech** (2026-04-23). All findings resolved in v0.5.3.

---

## Built for Colosseum Frontier Hackathon

Part of the AiFinPay ecosystem. Migrating to MIRA Network L1 post-hackathon.
