// Transaction history — parses recorder blocks into display-ready records.
// Supports IndexedDB caching for offline access and CSV export.

import type { DataItemJson } from './dataitem.ts';
import { hexToBytes } from './hex.ts';
import { decodeBigint } from './bigint.ts';
import { AO_MULTIPLIER } from './timestamp.ts';

// ── Types ────────────────────────────────────────────────────────────

export interface TxRecord {
  /** Block height (0-based). */
  blockHeight: number;
  /** Page index within the block. */
  pageIndex: number;
  /** Unix milliseconds (from block timestamp). */
  timestampMs: number;
  /** 'sent' | 'received' — relative to wallet owner. */
  direction: 'sent' | 'received';
  /** Share amount transferred (the wallet-relevant portion). */
  amount: string; // decimal string (bigint)
  /** Counterparty public key hex (truncated for display). */
  counterparty: string; // hex 64
  /** Whether the assignment contains a blob reference. */
  hasBlob: boolean;
  /** Giver seq IDs (for CSV). */
  giverSeqIds: number[];
  /** Receiver seq IDs — computed from block's FIRST_SEQ + position. */
  receiverSeqId: number | null;
}

// ── Block Parsing ────────────────────────────────────────────────────

/** Find a child JSON item by type name. */
function findChild(item: DataItemJson, typeName: string): DataItemJson | undefined {
  return item.items?.find(c => c.type === typeName);
}

/** Find all children with a given type name. */
function findChildren(item: DataItemJson, typeName: string): DataItemJson[] {
  return item.items?.filter(c => c.type === typeName) ?? [];
}

/** Decode an AMOUNT hex value to bigint. */
function decodeAmount(hexValue: string): bigint {
  const bytes = hexToBytes(hexValue);
  const [value] = decodeBigint(bytes, 0);
  return value;
}

/** Convert AO timestamp bytes (hex) to Unix milliseconds. */
function timestampToMs(hexValue: string): number {
  const bytes = hexToBytes(hexValue);
  const view = new DataView(bytes.buffer, bytes.byteOffset, 8);
  const aoTs = view.getBigInt64(0, false);
  const unixSecs = aoTs / AO_MULTIPLIER;
  return Number(unixSecs) * 1000;
}

interface ParsedParticipant {
  pubkey: string | null;  // hex, null for givers
  seqId: number | null;   // non-null for givers
  amount: bigint;
}

/** Parse participants from an ASSIGNMENT DataItemJson. */
function parseParticipants(assignment: DataItemJson): ParsedParticipant[] {
  const participants: ParsedParticipant[] = [];
  for (const p of findChildren(assignment, 'PARTICIPANT')) {
    const pub = findChild(p, 'ED25519_PUB');
    const seq = findChild(p, 'SEQ_ID');
    const amt = findChild(p, 'AMOUNT');
    if (!amt || typeof amt.value !== 'string') continue;

    participants.push({
      pubkey: pub && typeof pub.value === 'string' ? pub.value : null,
      seqId: seq && typeof seq.value === 'number' ? seq.value : null,
      amount: decodeAmount(amt.value),
    });
  }
  return participants;
}

/** Check if an assignment contains any blob-related items. */
function hasBlob(authorization: DataItemJson): boolean {
  const assignment = findChild(authorization, 'ASSIGNMENT');
  if (!assignment) return false;
  return findChildren(assignment, 'DATA_BLOB').length > 0
    || findChildren(assignment, 'SHA256').length > 0; // substituted blob hash
}

/**
 * Parse a list of block JSONs into TxRecords relevant to the given wallet keys.
 *
 * @param blocks - Array of BLOCK DataItemJson from getBlocks()
 * @param walletPubkeys - Set of wallet public key hex strings
 * @param walletSeqIds - Map from seq_id to pubkey hex (givers we own)
 * @param startHeight - Block height of the first block in the array
 */
export function parseBlocks(
  blocks: DataItemJson[],
  walletPubkeys: Set<string>,
  walletSeqIds: Map<number, string>,
  startHeight: number,
): TxRecord[] {
  const records: TxRecord[] = [];

  for (let i = 0; i < blocks.length; i++) {
    const block = blocks[i];
    const height = startHeight + i;

    const blockSigned = findChild(block, 'BLOCK_SIGNED');
    if (!blockSigned) continue;

    // Extract block timestamp from blockmaker's AUTH_SIG
    const authSig = findChild(blockSigned, 'AUTH_SIG');
    const tsItem = authSig ? findChild(authSig, 'TIMESTAMP') : undefined;
    const timestampMs = tsItem && typeof tsItem.value === 'string'
      ? timestampToMs(tsItem.value) : 0;

    const blockContents = findChild(blockSigned, 'BLOCK_CONTENTS');
    if (!blockContents) continue;

    // Get FIRST_SEQ for computing receiver seq_ids
    const firstSeqItem = findChild(blockContents, 'FIRST_SEQ');
    let nextSeq = firstSeqItem && typeof firstSeqItem.value === 'number'
      ? firstSeqItem.value : 0;

    const pages = findChildren(blockContents, 'PAGE');

    for (const page of pages) {
      const pageIndexItem = findChild(page, 'PAGE_INDEX');
      const pageIndex = pageIndexItem && typeof pageIndexItem.value === 'number'
        ? pageIndexItem.value : 0;

      const authorization = findChild(page, 'AUTHORIZATION');
      if (!authorization) continue;

      const assignment = findChild(authorization, 'ASSIGNMENT');
      if (!assignment) continue;

      const participants = parseParticipants(assignment);
      const blobPresent = hasBlob(authorization);

      // Separate givers and receivers
      const givers = participants.filter(p => p.seqId !== null);
      const receivers = participants.filter(p => p.pubkey !== null);

      // Track receiver seq_id assignment (recorder assigns incrementally)
      const receiverSeqStart = nextSeq;
      nextSeq += receivers.length;

      // Check if this wallet is involved as giver
      const myGiverEntries = givers.filter(g => walletSeqIds.has(g.seqId!));
      // Check if this wallet is involved as receiver
      const myReceiverEntries = receivers.filter(r => walletPubkeys.has(r.pubkey!));

      if (myGiverEntries.length > 0) {
        // Sent transaction — counterparty is the first receiver we don't own
        const counterparty = receivers.find(r => !walletPubkeys.has(r.pubkey!));
        const totalSent = myGiverEntries.reduce((sum, g) => sum + g.amount, 0n);
        records.push({
          blockHeight: height,
          pageIndex,
          timestampMs,
          direction: 'sent',
          amount: totalSent.toString(),
          counterparty: counterparty?.pubkey ?? '',
          hasBlob: blobPresent,
          giverSeqIds: myGiverEntries.map(g => g.seqId!),
          receiverSeqId: null,
        });
      }

      if (myReceiverEntries.length > 0) {
        // Received transaction — counterparty is the giver's pubkey (via walletSeqIds map)
        const giverPubkey = givers.length > 0 && givers[0].seqId !== null
          ? walletSeqIds.get(givers[0].seqId!) ?? '' : '';
        // If giver isn't in our wallet, try to identify from participants
        const counterpartyPubkey = giverPubkey || '';

        for (const recv of myReceiverEntries) {
          const recvIdx = receivers.indexOf(recv);
          records.push({
            blockHeight: height,
            pageIndex,
            timestampMs,
            direction: 'received',
            amount: recv.amount.toString(),
            counterparty: counterpartyPubkey,
            hasBlob: blobPresent,
            giverSeqIds: givers.map(g => g.seqId!),
            receiverSeqId: receiverSeqStart + recvIdx,
          });
        }
      }
    }
  }

  return records;
}

// ── CSV Export ────────────────────────────────────────────────────────

/** Export interface — extensible for future formats (SA-BAS, OFX, XBRL-CSV). */
export interface Exporter {
  readonly format: string;
  readonly mimeType: string;
  readonly extension: string;
  export(records: TxRecord[], chainSymbol: string, coinCount: string): string;
}

/** CSV exporter — default format. */
export const csvExporter: Exporter = {
  format: 'CSV',
  mimeType: 'text/csv',
  extension: '.csv',
  export(records: TxRecord[], chainSymbol: string, coinCount: string): string {
    const header = 'date,time,direction,amount_shares,amount_coins,counterparty,block_height,seq_id,has_blob';
    const coinTotal = BigInt(coinCount || '0');
    const lines = records.map(r => {
      const d = new Date(r.timestampMs);
      const date = d.toISOString().split('T')[0];
      const time = d.toISOString().split('T')[1].replace('Z', '');
      const shares = r.amount;
      // Coin value: shares / coinCount (approximate — display only)
      const coins = coinTotal > 0n
        ? (Number(BigInt(shares)) / Number(coinTotal)).toFixed(6)
        : '';
      const seqId = r.direction === 'sent'
        ? r.giverSeqIds.join(';')
        : (r.receiverSeqId?.toString() ?? '');
      const cp = r.counterparty ? r.counterparty.slice(0, 16) + '...' : '';
      return `${date},${time},${r.direction},${shares},${coins},${cp},${r.blockHeight},${seqId},${r.hasBlob}`;
    });
    return [header, ...lines].join('\n');
  },
};

/** All registered exporters. Future formats plug in here. */
export const exporters: Exporter[] = [csvExporter];

// ── IndexedDB Cache ──────────────────────────────────────────────────

const TX_DB_NAME = 'ao-tx-history';
const TX_DB_VERSION = 1;
const TX_STORE = 'transactions';
const TX_META_STORE = 'meta';

function openTxDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(TX_DB_NAME, TX_DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(TX_STORE)) {
        const store = db.createObjectStore(TX_STORE, { autoIncrement: true });
        store.createIndex('chainId', 'chainId', { unique: false });
        store.createIndex('blockHeight', 'blockHeight', { unique: false });
      }
      if (!db.objectStoreNames.contains(TX_META_STORE)) {
        db.createObjectStore(TX_META_STORE, { keyPath: 'chainId' });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

interface StoredTx extends TxRecord {
  chainId: string;
}

interface TxMeta {
  chainId: string;
  lastScannedHeight: number;
}

/** Get the last scanned block height for a chain. */
export async function getLastScannedHeight(chainId: string): Promise<number> {
  const db = await openTxDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(TX_META_STORE, 'readonly');
      const req = tx.objectStore(TX_META_STORE).get(chainId);
      req.onsuccess = () => {
        const meta = req.result as TxMeta | undefined;
        resolve(meta?.lastScannedHeight ?? 0);
      };
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

/** Save parsed transactions and update scan cursor. */
export async function saveTxRecords(
  chainId: string,
  records: TxRecord[],
  scannedUpTo: number,
): Promise<void> {
  const db = await openTxDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction([TX_STORE, TX_META_STORE], 'readwrite');
      const store = tx.objectStore(TX_STORE);
      for (const r of records) {
        store.add({ ...r, chainId } as StoredTx);
      }
      tx.objectStore(TX_META_STORE).put({ chainId, lastScannedHeight: scannedUpTo } as TxMeta);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}

/** Load cached transactions for a chain. */
export async function loadCachedTx(chainId: string): Promise<TxRecord[]> {
  const db = await openTxDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction(TX_STORE, 'readonly');
      const req = tx.objectStore(TX_STORE).index('chainId').getAll(chainId);
      req.onsuccess = () => resolve(req.result as StoredTx[]);
      req.onerror = () => reject(req.error);
    });
  } finally {
    db.close();
  }
}

/** Clear cached transactions for a chain (e.g. after re-scan). */
export async function clearCachedTx(chainId: string): Promise<void> {
  const db = await openTxDb();
  try {
    return await new Promise((resolve, reject) => {
      const tx = db.transaction([TX_STORE, TX_META_STORE], 'readwrite');
      const store = tx.objectStore(TX_STORE);
      const idx = store.index('chainId');
      const req = idx.openCursor(chainId);
      req.onsuccess = () => {
        const cursor = req.result;
        if (cursor) {
          cursor.delete();
          cursor.continue();
        }
      };
      tx.objectStore(TX_META_STORE).delete(chainId);
      tx.oncomplete = () => resolve();
      tx.onerror = () => reject(tx.error);
    });
  } finally {
    db.close();
  }
}
