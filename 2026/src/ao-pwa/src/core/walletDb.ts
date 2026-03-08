// Multi-key wallet with IndexedDB storage and device identity.
// Replaces the single-seed localStorage approach for multi-device sync.
//
// Spec: specs/WalletSync.md §6

import { bytesToHex, hexToBytes } from './hex.ts';
import { encryptSeed, decryptSeed, type EncryptedSeed } from './wallet.ts';

// ── Types ────────────────────────────────────────────────────────────

export type KeyStatus = 'unspent' | 'spent' | 'expired' | 'unconfirmed';

export interface KeyEntry {
  /** Auto-increment ID in IndexedDB. */
  id?: number;
  chainId: string;           // hex 64
  publicKey: string;         // hex 64
  seedHex: string;           // hex 64 plaintext OR encrypted blob (see seedEncrypted)
  seedEncrypted?: boolean;   // true if seedHex is an EncryptedSeed JSON string
  seqId: number | null;      // assigned by recorder; null if pre-recording
  amount: string | null;     // share amount as decimal string
  status: KeyStatus;
  acquiredAt: string;        // ISO 8601
  acquiredBy: string;        // device_id that created this key
  synced: boolean;           // synced to all peers?
  spentBy?: string;          // device_id that spent (if known)
  spentAt?: string;          // ISO 8601
}

// ── Seed Encryption Helpers ─────────────────────────────────────────

/** Encrypt a plaintext seed hex string. Returns the EncryptedSeed as JSON string. */
export async function encryptSeedHex(seedHex: string, passphrase: string): Promise<string> {
  const encrypted = await encryptSeed(hexToBytes(seedHex), passphrase);
  return JSON.stringify(encrypted);
}

/** Decrypt an encrypted seed (stored as JSON string) back to hex. */
export async function decryptSeedHex(encryptedJson: string, passphrase: string): Promise<string> {
  const encrypted: EncryptedSeed = JSON.parse(encryptedJson);
  const seed = await decryptSeed(encrypted, passphrase);
  return bytesToHex(seed);
}

/** Get the usable seed hex from a KeyEntry. If encrypted, decrypts with passphrase. */
export async function getSeedHex(entry: KeyEntry, passphrase: string | null): Promise<string> {
  if (!entry.seedEncrypted) return entry.seedHex;
  if (!passphrase) throw new Error('Passphrase required to decrypt seed');
  return decryptSeedHex(entry.seedHex, passphrase);
}

/** Encrypt all plaintext seeds in the database with the given passphrase. */
export async function encryptAllSeeds(passphrase: string): Promise<number> {
  const allKeys = await getKeys();
  let encrypted = 0;
  for (const key of allKeys) {
    if (!key.seedEncrypted && key.id != null) {
      const encryptedSeed = await encryptSeedHex(key.seedHex, passphrase);
      await updateKey(key.id, { seedHex: encryptedSeed, seedEncrypted: true });
      encrypted++;
    }
  }
  return encrypted;
}

export interface PeerDevice {
  deviceId: string;
  label: string;
  relayKey: string;          // hex 64 (shared symmetric key)
  pairedAt: string;          // ISO 8601
  lastSeen?: string;         // ISO 8601
}

export interface WalletConfig {
  deviceId: string;
  deviceLabel: string;
  relayUrl?: string;
  lastBackupAt?: string; // ISO timestamp of last backup export
}

// ── IndexedDB ────────────────────────────────────────────────────────

const DB_NAME = 'ao-wallet';
const DB_VERSION = 1;

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains('keys')) {
        const keyStore = db.createObjectStore('keys', { keyPath: 'id', autoIncrement: true });
        keyStore.createIndex('chainId', 'chainId', { unique: false });
        keyStore.createIndex('publicKey', 'publicKey', { unique: false });
        keyStore.createIndex('status', 'status', { unique: false });
        keyStore.createIndex('synced', 'synced', { unique: false });
      }
      if (!db.objectStoreNames.contains('peers')) {
        db.createObjectStore('peers', { keyPath: 'deviceId' });
      }
      if (!db.objectStoreNames.contains('config')) {
        db.createObjectStore('config', { keyPath: 'key' });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

// ── Device Identity ──────────────────────────────────────────────────

/** Singleton guard: prevents parallel getDeviceId() calls from racing. */
let deviceIdPromise: Promise<string> | null = null;

/** Get or create a persistent device ID (random 16 bytes, hex). */
export function getDeviceId(): Promise<string> {
  if (!deviceIdPromise) {
    deviceIdPromise = getDeviceIdInner().catch(e => {
      deviceIdPromise = null; // allow retry on failure
      throw e;
    });
  }
  return deviceIdPromise;
}

async function getDeviceIdInner(): Promise<string> {
  const config = await getConfig();
  if (config?.deviceId) return config.deviceId;

  const id = bytesToHex(crypto.getRandomValues(new Uint8Array(16)));
  await setConfig({ deviceId: id, deviceLabel: 'This Device' });
  return id;
}

export async function getDeviceLabel(): Promise<string> {
  const config = await getConfig();
  return config?.deviceLabel ?? 'This Device';
}

export async function setDeviceLabel(label: string): Promise<void> {
  const config = await getConfig() ?? { deviceId: await getDeviceId(), deviceLabel: label };
  config.deviceLabel = label;
  await setConfig(config);
}

export async function getLastBackupAt(): Promise<string | null> {
  const config = await getConfig();
  return config?.lastBackupAt ?? null;
}

export async function setLastBackupAt(isoTimestamp: string): Promise<void> {
  const config = await getConfig() ?? { deviceId: await getDeviceId(), deviceLabel: 'This Device' };
  config.lastBackupAt = isoTimestamp;
  await setConfig(config);
}

// ── Config CRUD ──────────────────────────────────────────────────────

async function getConfig(): Promise<WalletConfig | null> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('config', 'readonly');
      const req = tx.objectStore('config').get('wallet_config');
      req.onsuccess = () => {
        const result = req.result;
        resolve(result ? result.value as WalletConfig : null);
      };
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

async function setConfig(config: WalletConfig): Promise<void> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('config', 'readwrite');
      tx.objectStore('config').put({ key: 'wallet_config', value: config });
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

// ── Key CRUD ─────────────────────────────────────────────────────────

/** Add a new key entry. Returns the auto-generated ID. */
export async function addKey(entry: Omit<KeyEntry, 'id'>): Promise<number> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('keys', 'readwrite');
      const req = tx.objectStore('keys').add(entry);
      req.onsuccess = () => resolve(req.result as number);
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Get all keys, optionally filtered by chain. */
export async function getKeys(chainId?: string): Promise<KeyEntry[]> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('keys', 'readonly');
      const store = tx.objectStore('keys');
      let req: IDBRequest;
      if (chainId) {
        req = store.index('chainId').getAll(chainId);
      } else {
        req = store.getAll();
      }
      req.onsuccess = () => resolve(req.result as KeyEntry[]);
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

/** Get all unspent keys, optionally filtered by chain. */
export async function getUnspentKeys(chainId?: string): Promise<KeyEntry[]> {
  const keys = await getKeys(chainId);
  return keys.filter(k => k.status === 'unspent');
}

/** Get unsynced keys (for QR export). */
export async function getUnsyncedKeys(): Promise<KeyEntry[]> {
  const keys = await getKeys();
  return keys.filter(k => !k.synced);
}

/** Update a key entry by ID. */
export async function updateKey(id: number, updates: Partial<KeyEntry>): Promise<void> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('keys', 'readwrite');
      const store = tx.objectStore('keys');
      const req = store.get(id);
      req.onsuccess = () => {
        const entry = req.result as KeyEntry;
        if (entry) {
          Object.assign(entry, updates);
          store.put(entry);
        }
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Mark a key as spent. */
export async function markKeySpent(publicKey: string, spentBy: string): Promise<void> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('keys', 'readwrite');
      const store = tx.objectStore('keys');
      const req = store.index('publicKey').getAll(publicKey);
      req.onsuccess = () => {
        const entries = req.result as KeyEntry[];
        for (const entry of entries) {
          if (entry.status === 'unspent') {
            entry.status = 'spent';
            entry.spentBy = spentBy;
            entry.spentAt = new Date().toISOString();
            store.put(entry);
          }
        }
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Mark all unsynced keys as synced. */
export async function markAllSynced(): Promise<void> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('keys', 'readwrite');
      const store = tx.objectStore('keys');
      const req = store.getAll();
      req.onsuccess = () => {
        const entries = req.result as KeyEntry[];
        for (const entry of entries) {
          if (!entry.synced) {
            entry.synced = true;
            store.put(entry);
          }
        }
      };
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Find a key by public key hex. */
export async function findByPublicKey(publicKey: string): Promise<KeyEntry | null> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('keys', 'readonly');
      const req = tx.objectStore('keys').index('publicKey').getAll(publicKey);
      req.onsuccess = () => {
        const results = req.result as KeyEntry[];
        resolve(results.length > 0 ? results[0] : null);
      };
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

/** Import a key if it doesn't already exist (by publicKey). Idempotent.
 *  Uses a single readwrite transaction to avoid check-then-insert race. */
export async function importKeyIfNew(entry: Omit<KeyEntry, 'id'>): Promise<boolean> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('keys', 'readwrite');
      const store = tx.objectStore('keys');
      const req = store.index('publicKey').getAll(entry.publicKey);
      req.onsuccess = () => {
        if ((req.result as KeyEntry[]).length > 0) {
          resolve(false);
        } else {
          store.add(entry);
          resolve(true);
        }
      };
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Delete all keys and config. */
export async function clearAll(): Promise<void> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(['keys', 'peers', 'config'], 'readwrite');
      tx.objectStore('keys').clear();
      tx.objectStore('peers').clear();
      tx.objectStore('config').clear();
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Get total balance (unspent shares) for a chain. */
export async function chainBalance(chainId: string): Promise<bigint> {
  const keys = await getUnspentKeys(chainId);
  let total = 0n;
  for (const k of keys) {
    if (k.amount) total += BigInt(k.amount);
  }
  return total;
}

// ── Peer CRUD ────────────────────────────────────────────────────────

export async function addPeer(peer: PeerDevice): Promise<void> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('peers', 'readwrite');
      tx.objectStore('peers').put(peer);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

export async function getPeers(): Promise<PeerDevice[]> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('peers', 'readonly');
      const req = tx.objectStore('peers').getAll();
      req.onsuccess = () => resolve(req.result as PeerDevice[]);
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

export async function removePeer(deviceId: string): Promise<void> {
  const db = await openDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction('peers', 'readwrite');
      tx.objectStore('peers').delete(deviceId);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

// ── Migration from localStorage ──────────────────────────────────────

/** One-time migration: import single-seed localStorage wallet into IndexedDB. */
export async function migrateFromLocalStorage(): Promise<boolean> {
  const seedHex = localStorage.getItem('ao_wallet_seed');
  const pubkeyHex = localStorage.getItem('ao_wallet_pubkey');
  if (!seedHex || !pubkeyHex) return false;

  // Check if already migrated
  const existing = await findByPublicKey(pubkeyHex);
  if (existing) return false;

  const deviceId = await getDeviceId();

  await addKey({
    chainId: '',  // unknown — will be populated when UTXO is discovered
    publicKey: pubkeyHex,
    seedHex,
    seqId: null,
    amount: null,
    status: 'unconfirmed',
    acquiredAt: new Date().toISOString(),
    acquiredBy: deviceId,
    synced: true,  // no peers yet, so technically synced
  });

  // Remove old localStorage entries
  localStorage.removeItem('ao_wallet_seed');
  localStorage.removeItem('ao_wallet_pubkey');
  localStorage.removeItem('ao_wallet_label');

  return true;
}
