// Offline assignment queue — stores signed authorizations in IndexedDB
// and auto-submits them when the recorder becomes reachable.

import type { DataItemJson } from './dataitem.ts';

const DB_NAME = 'ao-offline';
const DB_VERSION = 1;
const STORE_NAME = 'pending_assignments';

export interface QueuedAssignment {
  id?: number;              // auto-increment key
  chainId: string;
  recorderUrl: string;
  authorization: DataItemJson;
  queuedAt: number;         // unix ms
  status: 'pending' | 'submitted' | 'failed';
  error?: string;
}

function openDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);
    req.onupgradeneeded = () => {
      const db = req.result;
      if (!db.objectStoreNames.contains(STORE_NAME)) {
        db.createObjectStore(STORE_NAME, { keyPath: 'id', autoIncrement: true });
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

/** Queue a signed assignment for later submission. */
export async function enqueue(entry: Omit<QueuedAssignment, 'id'>): Promise<void> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    tx.objectStore(STORE_NAME).add(entry);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** Get all pending (unsubmitted) assignments. */
export async function getPending(): Promise<QueuedAssignment[]> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readonly');
    const req = tx.objectStore(STORE_NAME).getAll();
    req.onsuccess = () => {
      const all = req.result as QueuedAssignment[];
      resolve(all.filter(a => a.status === 'pending'));
    };
    req.onerror = () => reject(req.error);
  });
}

/** Mark a queued assignment as submitted (removes it from pending). */
export async function markSubmitted(id: number): Promise<void> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const req = store.get(id);
    req.onsuccess = () => {
      const entry = req.result as QueuedAssignment;
      if (entry) {
        entry.status = 'submitted';
        store.put(entry);
      }
    };
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** Mark a queued assignment as failed. */
export async function markFailed(id: number, error: string): Promise<void> {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const tx = db.transaction(STORE_NAME, 'readwrite');
    const store = tx.objectStore(STORE_NAME);
    const req = store.get(id);
    req.onsuccess = () => {
      const entry = req.result as QueuedAssignment;
      if (entry) {
        entry.status = 'failed';
        entry.error = error;
        store.put(entry);
      }
    };
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** Try to submit all pending assignments. Returns count of successfully submitted. */
export async function flushPending(): Promise<number> {
  const pending = await getPending();
  let submitted = 0;

  for (const entry of pending) {
    try {
      const res = await fetch(`${entry.recorderUrl}/chain/${entry.chainId}/submit`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(entry.authorization),
      });
      if (res.ok) {
        await markSubmitted(entry.id!);
        submitted++;
      } else {
        const body = await res.text();
        await markFailed(entry.id!, `${res.status}: ${body}`);
      }
    } catch {
      // Network error — leave as pending for next retry
    }
  }

  return submitted;
}

/** Count of pending assignments. */
export async function pendingCount(): Promise<number> {
  const pending = await getPending();
  return pending.length;
}
