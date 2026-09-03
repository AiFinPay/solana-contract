import { describe, it } from 'mocha';
import { expect } from 'chai';
import { readFileSync } from 'fs';

describe('splitter_light - @solana/kit example', () => {
  const idlPath = 'target/idl/splitter_light.json';
  
  it('Shows how to use @solana/kit with splitter_light', async () => {
    const idl = JSON.parse(readFileSync(idlPath, 'utf-8'));
    
    console.log('Program ID:', idl.address);
    console.log('Instructions:', idl.instructions.map((i: any) => i.name));
    
    expect(idl.address).to.exist;
    expect(idl.instructions.length).to.be.greaterThan(0);
  });

  it('Verifies settle_native instruction exists', async () => {
    const idl = JSON.parse(readFileSync(idlPath, 'utf-8'));
    
    const settleNative = idl.instructions.find((i: any) => i.name === 'settle_native');
    expect(settleNative).to.exist;
    expect(settleNative.accounts.length).to.equal(8);
  });
});
