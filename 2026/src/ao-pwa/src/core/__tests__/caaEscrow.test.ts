import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  buildCaa, overallCaaStatus, pollCaaStatus,
  type CaaChainInput, type EscrowDisplayStatus,
} from '../caaEscrow.ts';
import { generateSigningKey } from '../sign.ts';
import { findChild, children, asVbcValue, asBytes } from '../dataitem.ts';
import * as tc from '../typecodes.ts';
import { bytesToHex } from '../hex.ts';

// Generate deterministic-ish chain IDs
function fakeChainId(n: number): string {
  return (n.toString(16).padStart(2, '0')).repeat(32);
}

async function makeTwoChainInputs(): Promise<[CaaChainInput, CaaChainInput]> {
  const keyA = await generateSigningKey();
  const keyB = await generateSigningKey();
  const recvA = await generateSigningKey();
  const recvB = await generateSigningKey();

  const chainA: CaaChainInput = {
    chainId: fakeChainId(1),
    recorderUrl: 'http://localhost:3001',
    givers: [{ seqId: 0n, amount: 100n, key: keyA }],
    receivers: [{ pubkey: recvA.publicKey, amount: 90n, key: recvA }],
    feeRate: { num: 1n, den: 100n },
  };

  const chainB: CaaChainInput = {
    chainId: fakeChainId(2),
    recorderUrl: 'http://localhost:3002',
    givers: [{ seqId: 0n, amount: 200n, key: keyB }],
    receivers: [{ pubkey: recvB.publicKey, amount: 180n, key: recvB }],
    feeRate: { num: 1n, den: 100n },
  };

  return [chainA, chainB];
}

describe('buildCaa', () => {
  it('requires at least 2 chains', async () => {
    const [chainA] = await makeTwoChainInputs();
    await expect(buildCaa([chainA], 300)).rejects.toThrow('at least 2 chains');
  });

  it('rejects more than 8 chains', async () => {
    const [chainA, chainB] = await makeTwoChainInputs();
    const chains = Array(9).fill(chainA);
    await expect(buildCaa(chains, 300)).rejects.toThrow('8 chains');
  });

  it('builds a valid 2-chain CAA DataItem', async () => {
    const [chainA, chainB] = await makeTwoChainInputs();
    const caa = await buildCaa([chainA, chainB], 300);

    expect(caa.typeCode).toBe(tc.CAA);
    expect(caa.value.kind).toBe('container');

    const kids = children(caa);

    // Should have: ESCROW_DEADLINE, LIST_SIZE, 2x CAA_COMPONENT, N AUTH_SIGs
    const deadline = findChild(caa, tc.ESCROW_DEADLINE);
    expect(deadline).toBeDefined();
    expect(asBytes(deadline!)?.length).toBe(8);

    const listSize = findChild(caa, tc.LIST_SIZE);
    expect(listSize).toBeDefined();
    expect(asVbcValue(listSize!)).toBe(2n);

    const components = kids.filter(c => c.typeCode === tc.CAA_COMPONENT);
    expect(components).toHaveLength(2);

    // Check chain orders
    const order0 = findChild(components[0], tc.CHAIN_ORDER);
    expect(asVbcValue(order0!)).toBe(0n);
    const order1 = findChild(components[1], tc.CHAIN_ORDER);
    expect(asVbcValue(order1!)).toBe(1n);

    // Chain refs
    const ref0 = findChild(components[0], tc.CHAIN_REF);
    expect(bytesToHex(asBytes(ref0!)!)).toBe(fakeChainId(1));
    const ref1 = findChild(components[1], tc.CHAIN_REF);
    expect(bytesToHex(asBytes(ref1!)!)).toBe(fakeChainId(2));

    // First component should have COORDINATOR_BOND (non-last chain)
    const bond = findChild(components[0], tc.COORDINATOR_BOND);
    expect(bond).toBeDefined();

    // Last component should NOT have COORDINATOR_BOND
    const bond1 = findChild(components[1], tc.COORDINATOR_BOND);
    expect(bond1).toBeUndefined();

    // Each component should have an ASSIGNMENT child
    const assign0 = findChild(components[0], tc.ASSIGNMENT);
    expect(assign0).toBeDefined();
    const assign1 = findChild(components[1], tc.ASSIGNMENT);
    expect(assign1).toBeDefined();

    // Overall AUTH_SIGs at top level
    const overallSigs = kids.filter(c => c.typeCode === tc.AUTH_SIG);
    // 4 keys total (2 givers + 2 receivers), all with signing keys
    expect(overallSigs.length).toBe(4);

    // Each overall sig should have ED25519_PUB (not PAGE_INDEX — that's per-component)
    for (const sig of overallSigs) {
      const pub = findChild(sig, tc.ED25519_PUB);
      expect(pub).toBeDefined();
    }
  });

  it('sets per-component AUTH_SIGs with PAGE_INDEX', async () => {
    const [chainA, chainB] = await makeTwoChainInputs();
    const caa = await buildCaa([chainA, chainB], 300);
    const components = children(caa).filter(c => c.typeCode === tc.CAA_COMPONENT);

    for (const comp of components) {
      const sigs = children(comp).filter(c => c.typeCode === tc.AUTH_SIG);
      expect(sigs.length).toBeGreaterThan(0);
      for (const sig of sigs) {
        const pageIdx = findChild(sig, tc.PAGE_INDEX);
        expect(pageIdx).toBeDefined();
      }
    }
  });
});

describe('overallCaaStatus', () => {
  it('returns unknown for empty array', () => {
    expect(overallCaaStatus([])).toBe('unknown');
  });

  it('returns finalized when all chains finalized', () => {
    const statuses: EscrowDisplayStatus[] = [
      { caaHash: 'abc', chainId: '01', chainSymbol: 'A', status: 'finalized', chainOrder: 0, deadlineUnixSecs: 0, blockHeight: 1, hasProof: true },
      { caaHash: 'abc', chainId: '02', chainSymbol: 'B', status: 'finalized', chainOrder: 1, deadlineUnixSecs: 0, blockHeight: 2, hasProof: true },
    ];
    expect(overallCaaStatus(statuses)).toBe('finalized');
  });

  it('returns binding when mixed finalized/escrowed', () => {
    const statuses: EscrowDisplayStatus[] = [
      { caaHash: 'abc', chainId: '01', chainSymbol: 'A', status: 'escrowed', chainOrder: 0, deadlineUnixSecs: 0, blockHeight: 1, hasProof: true },
      { caaHash: 'abc', chainId: '02', chainSymbol: 'B', status: 'finalized', chainOrder: 1, deadlineUnixSecs: 0, blockHeight: 2, hasProof: true },
    ];
    expect(overallCaaStatus(statuses)).toBe('binding');
  });

  it('returns expired when any chain expired', () => {
    const statuses: EscrowDisplayStatus[] = [
      { caaHash: 'abc', chainId: '01', chainSymbol: 'A', status: 'expired', chainOrder: 0, deadlineUnixSecs: 0, blockHeight: 1, hasProof: false },
      { caaHash: 'abc', chainId: '02', chainSymbol: 'B', status: 'escrowed', chainOrder: 1, deadlineUnixSecs: 0, blockHeight: 2, hasProof: false },
    ];
    expect(overallCaaStatus(statuses)).toBe('expired');
  });

  it('returns escrowed when all escrowed', () => {
    const statuses: EscrowDisplayStatus[] = [
      { caaHash: 'abc', chainId: '01', chainSymbol: 'A', status: 'escrowed', chainOrder: 0, deadlineUnixSecs: 0, blockHeight: 1, hasProof: true },
      { caaHash: 'abc', chainId: '02', chainSymbol: 'B', status: 'escrowed', chainOrder: 1, deadlineUnixSecs: 0, blockHeight: 2, hasProof: true },
    ];
    expect(overallCaaStatus(statuses)).toBe('escrowed');
  });
});

describe('pollCaaStatus', () => {
  const mockFetch = vi.fn();
  beforeEach(() => {
    mockFetch.mockReset();
    vi.stubGlobal('fetch', mockFetch);
  });

  it('aggregates status from multiple chains', async () => {
    // Chain A responds
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({
        caa_hash: 'abc',
        status: 'finalized',
        chain_order: 0,
        deadline: 1000,
        block_height: 5,
        has_proof: true,
      }),
    });
    // Chain B responds
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({
        caa_hash: 'abc',
        status: 'escrowed',
        chain_order: 1,
        deadline: 1000,
        block_height: 3,
        has_proof: true,
      }),
    });

    const result = await pollCaaStatus('abc', [
      { chainId: '01', recorderUrl: 'http://localhost:3001', symbol: 'A' },
      { chainId: '02', recorderUrl: 'http://localhost:3002', symbol: 'B' },
    ]);

    expect(result).toHaveLength(2);
    expect(result[0].status).toBe('finalized');
    expect(result[1].status).toBe('escrowed');
    expect(result[0].chainSymbol).toBe('A');
  });

  it('handles fetch failures gracefully', async () => {
    mockFetch.mockRejectedValueOnce(new Error('network error'));

    const result = await pollCaaStatus('abc', [
      { chainId: '01', recorderUrl: 'http://localhost:3001', symbol: 'A' },
    ]);

    expect(result).toHaveLength(0); // failed fetch = no result
  });
});
