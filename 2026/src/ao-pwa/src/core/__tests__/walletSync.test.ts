// Tests for walletSync — sync payload building and import.

import { describe, it, expect, beforeEach } from 'vitest';
import 'fake-indexeddb/auto';
import {
  buildSyncPayload, buildFullExportPayload,
  importSyncPayload, serializePayload, deserializePayload,
  type SyncPayload,
} from '../walletSync.ts';
import { addKey, clearAll, getKeys, getDeviceId } from '../walletDb.ts';

beforeEach(async () => {
  await clearAll();
});

describe('walletSync', () => {
  it('builds sync payload from unsynced keys', async () => {
    const deviceId = await getDeviceId();
    await addKey({
      chainId: 'a'.repeat(64),
      publicKey: 'b'.repeat(64),
      seedHex: 'c'.repeat(64),
      seqId: 5,
      amount: '1000',
      status: 'unspent',
      acquiredAt: '2026-03-08T00:00:00Z',
      acquiredBy: deviceId,
      synced: false,
    });
    await addKey({
      chainId: 'a'.repeat(64),
      publicKey: 'd'.repeat(64),
      seedHex: 'e'.repeat(64),
      seqId: 3,
      amount: '500',
      status: 'spent',
      acquiredAt: '2026-03-07T00:00:00Z',
      acquiredBy: deviceId,
      synced: false,
      spentAt: '2026-03-08T01:00:00Z',
    });

    const payload = await buildSyncPayload();
    expect(payload.v).toBe(1);
    expect(payload.type).toBe('key_sync');
    expect(payload.deviceId).toBe(deviceId);
    expect(payload.keys).toHaveLength(1); // only unspent unsynced
    expect(payload.keys[0].publicKey).toBe('b'.repeat(64));
    expect(payload.spent).toHaveLength(1); // spent unsynced
    expect(payload.spent[0].publicKey).toBe('d'.repeat(64));
  });

  it('builds full export with all unspent keys', async () => {
    await addKey({
      chainId: 'a'.repeat(64),
      publicKey: 'b'.repeat(64),
      seedHex: 'c'.repeat(64),
      seqId: 5,
      amount: '1000',
      status: 'unspent',
      acquiredAt: '2026-03-08T00:00:00Z',
      acquiredBy: 'dev1',
      synced: true, // even synced ones included in full export
    });

    const payload = await buildFullExportPayload();
    expect(payload.keys).toHaveLength(1);
    expect(payload.spent).toHaveLength(0);
  });

  it('serializes to spec-compliant snake_case and deserializes back', async () => {
    const payload: SyncPayload = {
      v: 1,
      type: 'key_sync',
      deviceId: 'f'.repeat(32),
      keys: [{
        chainId: 'a'.repeat(64),
        publicKey: 'b'.repeat(64),
        seedHex: 'c'.repeat(64),
        seqId: 1,
        amount: '100',
        acquiredAt: '2026-01-01T00:00:00Z',
      }],
      spent: [{
        chainId: 'd'.repeat(64),
        publicKey: 'e'.repeat(64),
        seqId: 2,
        spentAt: '2026-02-01T00:00:00Z',
      }],
    };

    const json = serializePayload(payload);
    const wire = JSON.parse(json);

    // Verify snake_case in wire format
    expect(wire.device_id).toBe('f'.repeat(32));
    expect(wire.deviceId).toBeUndefined();
    expect(wire.keys[0].chain_id).toBe('a'.repeat(64));
    expect(wire.keys[0].public_key).toBe('b'.repeat(64));
    expect(wire.keys[0].encrypted_seed).toBe('c'.repeat(64));
    expect(wire.keys[0].acquired_at).toBe('2026-01-01T00:00:00Z');
    expect(wire.spent[0].chain_id).toBe('d'.repeat(64));
    expect(wire.spent[0].spent_at).toBe('2026-02-01T00:00:00Z');

    // Verify round-trip
    const parsed = deserializePayload(json);
    expect(parsed.v).toBe(1);
    expect(parsed.keys).toHaveLength(1);
    expect(parsed.keys[0].publicKey).toBe('b'.repeat(64));
    expect(parsed.keys[0].seedHex).toBe('c'.repeat(64));
    expect(parsed.spent[0].spentAt).toBe('2026-02-01T00:00:00Z');
  });

  it('deserializes legacy camelCase payloads', () => {
    const legacyJson = JSON.stringify({
      v: 1,
      type: 'key_sync',
      deviceId: 'dev1',
      keys: [{
        chainId: 'a'.repeat(64),
        publicKey: 'b'.repeat(64),
        seedHex: 'c'.repeat(64),
        seqId: 1,
        amount: '100',
        acquiredAt: '2026-01-01T00:00:00Z',
      }],
      spent: [],
    });
    const parsed = deserializePayload(legacyJson);
    expect(parsed.deviceId).toBe('dev1');
    expect(parsed.keys[0].publicKey).toBe('b'.repeat(64));
    expect(parsed.keys[0].seedHex).toBe('c'.repeat(64));
  });

  it('rejects invalid payloads', () => {
    expect(() => deserializePayload('{}')).toThrow('Invalid sync payload');
    expect(() => deserializePayload('{"v":2,"type":"key_sync"}')).toThrow('Invalid sync payload');
  });

  it('imports keys from sync payload', async () => {
    const payload: SyncPayload = {
      v: 1,
      type: 'key_sync',
      deviceId: 'remote_device_123456',
      keys: [
        {
          chainId: 'a'.repeat(64),
          publicKey: 'b'.repeat(64),
          seedHex: 'c'.repeat(64),
          seqId: 10,
          amount: '5000',
          acquiredAt: '2026-03-08T00:00:00Z',
        },
      ],
      spent: [],
    };

    const result = await importSyncPayload(payload);
    expect(result.imported).toBe(1);

    const keys = await getKeys();
    expect(keys).toHaveLength(1);
    expect(keys[0].publicKey).toBe('b'.repeat(64));
    expect(keys[0].acquiredBy).toBe('remote_device_123456');
    expect(keys[0].synced).toBe(true); // imported keys marked synced
  });

  it('import is idempotent', async () => {
    const payload: SyncPayload = {
      v: 1,
      type: 'key_sync',
      deviceId: 'dev1',
      keys: [{
        chainId: 'a'.repeat(64),
        publicKey: 'b'.repeat(64),
        seedHex: 'c'.repeat(64),
        seqId: 1,
        amount: '100',
        acquiredAt: '2026-01-01T00:00:00Z',
      }],
      spent: [],
    };

    const r1 = await importSyncPayload(payload);
    expect(r1.imported).toBe(1);

    const r2 = await importSyncPayload(payload);
    expect(r2.imported).toBe(0);

    const keys = await getKeys();
    expect(keys).toHaveLength(1);
  });

  it('imports spent notifications', async () => {
    // First add a key
    await addKey({
      chainId: 'a'.repeat(64),
      publicKey: 'b'.repeat(64),
      seedHex: 'c'.repeat(64),
      seqId: 1,
      amount: '100',
      status: 'unspent',
      acquiredAt: '2026-01-01T00:00:00Z',
      acquiredBy: 'dev1',
      synced: true,
    });

    // Import a payload marking it as spent
    const payload: SyncPayload = {
      v: 1,
      type: 'key_sync',
      deviceId: 'dev2',
      keys: [],
      spent: [{
        chainId: 'a'.repeat(64),
        publicKey: 'b'.repeat(64),
        seqId: 1,
        spentAt: '2026-03-08T12:00:00Z',
      }],
    };

    const result = await importSyncPayload(payload);
    expect(result.spentMarked).toBe(1);

    const keys = await getKeys();
    expect(keys[0].status).toBe('spent');
    expect(keys[0].spentBy).toBe('dev2');
  });
});
