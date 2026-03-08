// Wallet sync: UTXO validation, sync payload building, and QR key transfer.
// Spec: specs/WalletSync.md §2–3

import { RecorderClient } from '../api/client.ts';
import * as db from './walletDb.ts';
import type { KeyEntry } from './walletDb.ts';

// ── UTXO Validation (§2) ────────────────────────────────────────────

export interface ValidationResult {
  validated: number;
  stillUnspent: number;
  newlySpent: number;
  unknownSpends: KeyEntry[];  // spent by unrecognized device
  errors: string[];
}

/**
 * Validate all held keys for a chain against the recorder.
 * Updates key status in IndexedDB. Returns summary.
 */
export async function validateKeysOnChain(
  client: RecorderClient,
  chainId: string,
): Promise<ValidationResult> {
  const keys = await db.getKeys(chainId);
  const unspent = keys.filter(k => k.status === 'unspent');
  const deviceId = await db.getDeviceId();
  const peers = await db.getPeers();
  const peerIds = new Set([deviceId, ...peers.map(p => p.deviceId)]);

  const result: ValidationResult = {
    validated: 0,
    stillUnspent: 0,
    newlySpent: 0,
    unknownSpends: [],
    errors: [],
  };

  for (const key of unspent) {
    if (key.seqId === null) continue; // unconfirmed — can't check yet

    try {
      const utxo = await client.getUtxo(chainId, key.seqId);
      result.validated++;

      if (utxo.status === 'Unspent') {
        result.stillUnspent++;
        // Update amount if recorder has different value
        if (key.amount !== utxo.amount) {
          await db.updateKey(key.id!, { amount: utxo.amount });
        }
      } else {
        // Key was spent — figure out by whom
        result.newlySpent++;
        const spentBy = key.spentBy ?? 'unknown';
        await db.updateKey(key.id!, {
          status: 'spent',
          spentBy,
          spentAt: new Date().toISOString(),
        });
        if (!peerIds.has(spentBy)) {
          result.unknownSpends.push(key);
        }
      }
    } catch (e) {
      result.errors.push(`seq ${key.seqId}: ${e}`);
    }
  }

  return result;
}

/**
 * Discover UTXOs on a chain that match any of the wallet's public keys.
 * Used after migration or initial import to associate keys with seq IDs.
 */
export async function discoverUtxos(
  client: RecorderClient,
  chainId: string,
  nextSeqId: number,
): Promise<number> {
  const keys = await db.getKeys();
  const keyMap = new Map<string, KeyEntry>();
  for (const k of keys) {
    if (k.status !== 'spent' && k.status !== 'expired') {
      keyMap.set(k.publicKey, k);
    }
  }

  let found = 0;
  for (let seq = 0; seq < nextSeqId; seq++) {
    try {
      const utxo = await client.getUtxo(chainId, seq);
      const key = keyMap.get(utxo.pubkey);
      if (key) {
        await db.updateKey(key.id!, {
          chainId,
          seqId: utxo.seq_id,
          amount: utxo.amount,
          status: utxo.status === 'Unspent' ? 'unspent' : 'spent',
        });
        found++;
      }
    } catch {
      // seq may not exist
    }
  }

  return found;
}

// ── Sync Payload (§3) ───────────────────────────────────────────────

export interface SyncPayload {
  v: 1;
  type: 'key_sync';
  deviceId: string;
  keys: SyncKeyEntry[];
  spent: SyncSpentEntry[];
}

interface SyncKeyEntry {
  chainId: string;
  publicKey: string;
  seedHex: string;
  seqId: number | null;
  amount: string | null;
  acquiredAt: string;
}

interface SyncSpentEntry {
  chainId: string;
  publicKey: string;
  seqId: number | null;
  spentAt: string;
}

/** Build a sync payload with all unsynced keys and recently spent keys.
 *  If passphrase is provided, decrypts encrypted seeds for transport.
 *  The sync payload always contains plaintext seeds — outer encryption
 *  (relay AES-GCM or QR air-gap) handles transport security. */
export async function buildSyncPayload(passphrase?: string | null): Promise<SyncPayload> {
  const deviceId = await db.getDeviceId();
  const allKeys = await db.getKeys();

  const unsyncedKeys = allKeys.filter(k => !k.synced && k.status !== 'spent');
  const recentlySpent = allKeys.filter(k => k.status === 'spent' && !k.synced);

  const keys: SyncKeyEntry[] = [];
  for (const k of unsyncedKeys) {
    keys.push({
      chainId: k.chainId,
      publicKey: k.publicKey,
      seedHex: await db.getSeedHex(k, passphrase ?? null),
      seqId: k.seqId,
      amount: k.amount,
      acquiredAt: k.acquiredAt,
    });
  }

  return {
    v: 1,
    type: 'key_sync',
    deviceId,
    keys,
    spent: recentlySpent.map(k => ({
      chainId: k.chainId,
      publicKey: k.publicKey,
      seqId: k.seqId,
      spentAt: k.spentAt ?? new Date().toISOString(),
    })),
  };
}

/** Build a full wallet export payload (all unspent keys).
 *  If passphrase is provided, decrypts encrypted seeds for transport. */
export async function buildFullExportPayload(passphrase?: string | null): Promise<SyncPayload> {
  const deviceId = await db.getDeviceId();
  const allKeys = await db.getKeys();
  const unspent = allKeys.filter(k => k.status === 'unspent' || k.status === 'unconfirmed');

  const keys: SyncKeyEntry[] = [];
  for (const k of unspent) {
    keys.push({
      chainId: k.chainId,
      publicKey: k.publicKey,
      seedHex: await db.getSeedHex(k, passphrase ?? null),
      seqId: k.seqId,
      amount: k.amount,
      acquiredAt: k.acquiredAt,
    });
  }

  return {
    v: 1,
    type: 'key_sync',
    deviceId,
    keys,
    spent: [],
  };
}

/** Import keys from a sync payload. Returns count of new keys imported.
 *  If passphrase is provided, encrypts incoming seeds before storing. */
export async function importSyncPayload(
  payload: SyncPayload,
  passphrase?: string | null,
): Promise<{ imported: number; spentMarked: number }> {
  const deviceId = await db.getDeviceId();
  let imported = 0;
  let spentMarked = 0;

  // Import new keys — encrypt seeds with local passphrase if set
  for (const k of payload.keys) {
    const storedSeed = passphrase
      ? await db.encryptSeedHex(k.seedHex, passphrase)
      : k.seedHex;
    const added = await db.importKeyIfNew({
      chainId: k.chainId,
      publicKey: k.publicKey,
      seedHex: storedSeed,
      seedEncrypted: !!passphrase,
      seqId: k.seqId,
      amount: k.amount,
      status: k.seqId !== null ? 'unspent' : 'unconfirmed',
      acquiredAt: k.acquiredAt,
      acquiredBy: payload.deviceId,
      synced: true, // we just received it, so it's synced
    });
    if (added) imported++;
  }

  // Mark spent keys
  for (const s of payload.spent) {
    await db.markKeySpent(s.publicKey, payload.deviceId);
    spentMarked++;
  }

  return { imported, spentMarked };
}

/** Serialize payload to spec-compliant snake_case JSON for QR/file export. */
export function serializePayload(payload: SyncPayload): string {
  return JSON.stringify({
    v: payload.v,
    type: payload.type,
    device_id: payload.deviceId,
    keys: payload.keys.map(k => ({
      chain_id: k.chainId,
      public_key: k.publicKey,
      encrypted_seed: k.seedHex,
      seq_id: k.seqId,
      amount: k.amount,
      acquired_at: k.acquiredAt,
    })),
    spent: payload.spent.map(s => ({
      chain_id: s.chainId,
      public_key: s.publicKey,
      seq_id: s.seqId,
      spent_at: s.spentAt,
    })),
  });
}

/** Deserialize and validate a sync payload from JSON string.
 *  Accepts both spec snake_case and legacy camelCase field names. */
export function deserializePayload(json: string): SyncPayload {
  const obj = JSON.parse(json);
  if (obj.v !== 1 || obj.type !== 'key_sync') {
    throw new Error('Invalid sync payload: expected v=1, type=key_sync');
  }
  // Accept both snake_case (spec) and camelCase (legacy)
  const deviceId = obj.device_id ?? obj.deviceId;
  if (!deviceId || !Array.isArray(obj.keys) || !Array.isArray(obj.spent)) {
    throw new Error('Invalid sync payload: missing required fields');
  }
  return {
    v: 1,
    type: 'key_sync',
    deviceId,
    keys: obj.keys.map((k: Record<string, unknown>) => ({
      chainId: (k.chain_id ?? k.chainId) as string,
      publicKey: (k.public_key ?? k.publicKey) as string,
      seedHex: (k.encrypted_seed ?? k.seedHex) as string,
      seqId: (k.seq_id ?? k.seqId ?? null) as number | null,
      amount: (k.amount ?? null) as string | null,
      acquiredAt: (k.acquired_at ?? k.acquiredAt) as string,
    })),
    spent: obj.spent.map((s: Record<string, unknown>) => ({
      chainId: (s.chain_id ?? s.chainId) as string,
      publicKey: (s.public_key ?? s.publicKey) as string,
      seqId: (s.seq_id ?? s.seqId ?? null) as number | null,
      spentAt: (s.spent_at ?? s.spentAt) as string,
    })),
  };
}
