# TypeScript Tests with @solana/kit

This directory contains TypeScript/Mocha tests using the modern **@solana/kit** SDK.

## Setup

Tests are configured to run with Mocha + Chai. The TypeScript configuration is in `tsconfig.json`.

## Running Tests

```bash
# Compile TypeScript
pnpm exec tsc

# Run compiled tests
pnpm exec mocha target/tests/**/*.js

# Or run with environment variables for localnet
ANCHOR_PROVIDER_URL=http://localhost:8899 ANCHOR_WALLET=~/.config/solana/id.json pnpm exec mocha target/tests/**/*.js
```

## Example Test Structure

```typescript
import { describe, it } from 'mocha';
import { expect } from 'chai';
import { createClient, generateKeyPairSigner } from '@solana/kit';
import { readFileSync } from 'fs';

describe('splitter_light', () => {
  const client = createClient({ rpcUrl: 'http://localhost:8899' });

  it('interacts with the program', async () => {
    const idl = JSON.parse(readFileSync('target/idl/splitter_light.json', 'utf-8'));
    console.log('Program ID:', idl.address);
  });
});
```

## Using @solana/kit vs @coral-xyz/anchor

This project uses **@solana/kit** (the official Solana SDK) instead of Anchor's JS client. Key differences:

### @solana/kit (Recommended - Modern)
```typescript
import { createClient, createTransactionMessage, appendTransactionMessageInstruction } from '@solana/kit';

const client = createClient({ rpcUrl });
const transactionMessage = await createTransactionMessage({ feePayer, latestBlockhash });
const transactionWithInstruction = await appendTransactionMessageInstruction(instruction, transactionMessage);
```

### @coral-xyz/anchor (Legacy)
```typescript
import * as anchor from '@coral-xyz/anchor';

const provider = anchor.AnchorProvider.env();
const program = new anchor.Program(idl, provider);
await program.methods.settleNative(...).accounts({...}).rpc();
```

## Building Program IDLs

Before running tests, build the program to generate IDLs:

```bash
anchor build
```

This generates:
- `target/idl/splitter_light.json` - IDL for TypeScript tests
- `target/types/splitter_light.ts` - TypeScript types (for Anchor-style tests)
- `target/deploy/splitter_light.so` - Deployed program

## Notes

- Tests do **not** affect deployed code size (`.so` file) - they're completely separate
- Rust tests in `src/lib.rs` are excluded from production builds with `#[cfg(test)]`
- TypeScript tests run independently using Mocha
