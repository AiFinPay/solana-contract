# Mainnet scripts — splitter v1.4

Mirrors of `scripts/devnet/` pointed at mainnet-beta and the canonical
program ID from [`Anchor.toml`](../../Anchor.toml).
Deployment is designed for a **Ledger-held deployer**.

| Script | Purpose | Spends SOL? |
|---|---|---|
| `mainnet-deploy.sh` | Deploy / upgrade canonical program (Ledger signs) | YES (~2.5 + margin) |
| `calculate-deploy-cost.sh` | Estimate deploy + PDA rent | No (read-only) |
| `calculate-initialize-cost.sh` | Estimate `initialize` cost | No (read-only) |
| `simulate-mainnet-deploy.sh` | Throwaway deploy to measure real cost | YES (recoverable via close) |
| `check-mainnet-splitter.ts` | Readiness check (program, PDAs, routes) | No |
| `initialize-mainnet-splitter.ts` | `initialize` (file-keypair signing only) | YES (PDA rent) |
| `configure-mainnet-route.ts` | `configure_route` for both routes (admin signs) | YES (fees) |

## Pre-flight (do once, in order)

1. **Set `DEPLOYER`.** Replace the placeholder in
   `programs/splitter/src/constants.rs` with the Ledger address that will
   deploy AND pay for `initialize` (`initialize` accepts no other payer).
   `mainnet-deploy.sh` aborts while the placeholder bytes are present.
2. **Estimate.** `./calculate-deploy-cost.sh` and
   `./calculate-initialize-cost.sh` — fund the Ledger with deploy cost +
   PDA rent + margin (script requires ≥ 3 SOL).
3. **(Optional) Measure.** `./simulate-mainnet-deploy.sh` deploys a
   throwaway program ID with real SOL to record the true cost, then offers
   to `close` it and recover rent. Never touches the canonical ID.
4. **Connect Ledger.** Unlock, open the Solana app, verify every address
   on the device screen.

## Launch sequence

```bash
# 1. Deploy (Ledger signs; rebuilds SBF so DEPLOYER is baked in)
./scripts/mainnet/mainnet-deploy.sh
# flags: --keypair usb://ledger?key=0  --yes  --skip-build  --url <rpc>

# 2. Initialize — payer must be DEPLOYER
DEPLOYER_KEYPAIR_PATH=<path> ADMIN_PUBKEY=... SIGNER_PUBKEY=... \
  PAUSER_PUBKEY=... TREASURY_PUBKEY=... STABLECOINS=... \
  ROUTE_IDS=AGENT_X402,MERCHANT_AIFP1 TREASURY_BPS=0,100 IP_CREATOR_BPS=0,0 \
  npm run initialize:mainnet

# 3. Configure routes (admin signs)
ADMIN_KEYPAIR_PATH=<path> TREASURY_PUBKEY=... npm run configure:mainnet

# 4. Verify
npm run check:mainnet
```

## Ledger-held DEPLOYER + `initialize`

`initialize-mainnet-splitter.ts` signs with a **file** keypair and refuses
to run without `DEPLOYER_KEYPAIR_PATH`. If DEPLOYER lives on the Ledger,
do NOT export the key — send the identical `initialize` instruction via a
Ledger-capable client instead (e.g. Anchor CLI with
`--provider.wallet "usb://ledger?key=<n>"`, same program ID, same PDAs,
same Borsh params), then continue with steps 3–4.

## Safety rules

- `mainnet-deploy.sh` deploys ONLY to the canonical ID: the program keypair
  must be `keypairs/splitter-keypair.json` and match `declare_id!`.
  A fresh keypair is never generated (unlike devnet/localnet scripts).
- If the program already exists, the deploy is treated as an **upgrade**:
  the script checks that the signer is the on-chain upgrade authority.
- Destructive prompts require typing `DEPLOY-MAINNET` / `SIMULATE-MAINNET`
  (`--yes` skips — use only in attended runs).
- Mainnet keys are never committed: `keypairs/` is gitignored.
