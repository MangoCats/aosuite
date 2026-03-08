// Tests for walletDb — IndexedDB-backed multi-key wallet.
// Uses fake-indexeddb for Node.js environment.

import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import {
  addKey, getKeys, getUnspentKeys, getUnsyncedKeys,
  updateKey, markKeySpent, markAllSynced,
  findByPublicKey, importKeyIfNew, clearAll,
  chainBalance, getDeviceId, setDeviceLabel, getDeviceLabel,
  type KeyEntry,
} from '../walletDb.ts';

function makeKey(overrides: Partial<KeyEntry> = {}): Omit<KeyEntry, 'id'> {
  return {
    chainId: 'c'.repeat(64),
    publicKey: 'a'.repeat(64),
    seedHex: 'b'.repeat(64),
    seqId: 1,
    amount: '1000000',
    status: 'unspent',
    acquiredAt: new Date().toISOString(),
    acquiredBy: 'device1',
    synced: false,
    ...overrides,
  };
}

beforeEach(async () => {
  await clearAll();
});

describe('walletDb', () => {
  it('adds and retrieves keys', async () => {
    const id = await addKey(makeKey());
    expect(id).toBeGreaterThan(0);

    const keys = await getKeys();
    expect(keys).toHaveLength(1);
    expect(keys[0].publicKey).toBe('a'.repeat(64));
    expect(keys[0].id).toBe(id);
  });

  it('filters by chain', async () => {
    await addKey(makeKey({ chainId: 'a'.repeat(64) }));
    await addKey(makeKey({ chainId: 'b'.repeat(64), publicKey: 'x'.repeat(64) }));

    const aKeys = await getKeys('a'.repeat(64));
    expect(aKeys).toHaveLength(1);
    expect(aKeys[0].chainId).toBe('a'.repeat(64));
  });

  it('filters unspent keys', async () => {
    await addKey(makeKey({ status: 'unspent' }));
    await addKey(makeKey({ status: 'spent', publicKey: 'c'.repeat(64) }));

    const unspent = await getUnspentKeys();
    expect(unspent).toHaveLength(1);
    expect(unspent[0].status).toBe('unspent');
  });

  it('filters unsynced keys', async () => {
    await addKey(makeKey({ synced: false }));
    await addKey(makeKey({ synced: true, publicKey: 'd'.repeat(64) }));

    const unsynced = await getUnsyncedKeys();
    expect(unsynced).toHaveLength(1);
    expect(unsynced[0].synced).toBe(false);
  });

  it('updates key fields', async () => {
    const id = await addKey(makeKey());
    await updateKey(id, { amount: '2000000', status: 'spent' });

    const keys = await getKeys();
    expect(keys[0].amount).toBe('2000000');
    expect(keys[0].status).toBe('spent');
  });

  it('marks key as spent', async () => {
    await addKey(makeKey({ publicKey: 'e'.repeat(64) }));
    await markKeySpent('e'.repeat(64), 'device2');

    const keys = await getKeys();
    expect(keys[0].status).toBe('spent');
    expect(keys[0].spentBy).toBe('device2');
    expect(keys[0].spentAt).toBeDefined();
  });

  it('marks all as synced', async () => {
    await addKey(makeKey({ synced: false }));
    await addKey(makeKey({ synced: false, publicKey: 'f'.repeat(64) }));

    await markAllSynced();
    const unsynced = await getUnsyncedKeys();
    expect(unsynced).toHaveLength(0);
  });

  it('finds by public key', async () => {
    await addKey(makeKey({ publicKey: 'g'.repeat(64) }));

    const found = await findByPublicKey('g'.repeat(64));
    expect(found).not.toBeNull();
    expect(found!.publicKey).toBe('g'.repeat(64));

    const notFound = await findByPublicKey('z'.repeat(64));
    expect(notFound).toBeNull();
  });

  it('importKeyIfNew is idempotent', async () => {
    const entry = makeKey({ publicKey: 'h'.repeat(64) });

    const added1 = await importKeyIfNew(entry);
    expect(added1).toBe(true);

    const added2 = await importKeyIfNew(entry);
    expect(added2).toBe(false);

    const keys = await getKeys();
    expect(keys).toHaveLength(1);
  });

  it('computes chain balance', async () => {
    const chain = 'i'.repeat(64);
    await addKey(makeKey({ chainId: chain, amount: '1000', publicKey: 'j'.repeat(64) }));
    await addKey(makeKey({ chainId: chain, amount: '2000', publicKey: 'k'.repeat(64) }));
    await addKey(makeKey({ chainId: chain, amount: '500', status: 'spent', publicKey: 'l'.repeat(64) }));

    const balance = await chainBalance(chain);
    expect(balance).toBe(3000n);
  });

  it('generates and persists device ID', async () => {
    const id1 = await getDeviceId();
    expect(id1).toHaveLength(32); // 16 bytes hex
    const id2 = await getDeviceId();
    expect(id2).toBe(id1); // same across calls
  });

  it('sets and gets device label', async () => {
    await setDeviceLabel('Test Device');
    const label = await getDeviceLabel();
    expect(label).toBe('Test Device');
  });

  it('clearAll removes everything', async () => {
    await addKey(makeKey());
    await clearAll();
    const keys = await getKeys();
    expect(keys).toHaveLength(0);
  });
});
