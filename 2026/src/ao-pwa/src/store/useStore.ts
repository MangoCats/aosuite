// Global application store — Zustand with localStorage persistence for wallet.

import { create } from 'zustand';
import type { ChainInfo, ChainListEntry } from '../api/client.ts';
import type { ChimeStyle } from '../core/chime.ts';
import { isChimeStyle } from '../core/chime.ts';

// localStorage helpers (silent on failure for SSR / private browsing)
function loadString(key: string): string | null {
  try { return localStorage.getItem(key); } catch { return null; }
}
function saveString(key: string, value: string | null) {
  try {
    if (value === null) localStorage.removeItem(key);
    else localStorage.setItem(key, value);
  } catch { /* ignore */ }
}
function loadNumber(key: string, fallback: number): number {
  const s = loadString(key);
  if (s === null) return fallback;
  const n = Number(s);
  return Number.isFinite(n) ? n : fallback;
}

export interface NotificationSettings {
  enabled: boolean;
  chimeStyle: ChimeStyle;
  volume: number;        // 0–1
  quietStart: number;    // hour 0–23
  quietEnd: number;      // hour 0–23
  quickMuteUntil: number | null; // Unix ms timestamp, null = not muted
}

export interface AppState {
  // Connection
  recorderUrl: string;
  connected: boolean;
  setRecorderUrl: (url: string) => void;
  setConnected: (connected: boolean) => void;

  // Chains
  chains: ChainListEntry[];
  setChains: (chains: ChainListEntry[]) => void;
  selectedChainId: string | null;
  selectChain: (chainId: string | null) => void;
  chainInfo: ChainInfo | null;
  setChainInfo: (info: ChainInfo | null) => void;

  // Active key — session-only cache of the current IndexedDB key.
  // No longer persisted to localStorage (IndexedDB is the source of truth).
  walletLabel: string | null;
  publicKeyHex: string | null;
  seedHex: string | null;
  setWallet: (label: string, publicKeyHex: string, seedHex: string) => void;
  clearWallet: () => void;

  // Multi-key wallet state (IndexedDB-backed, see core/walletDb.ts)
  walletKeyCount: number;
  unsyncedKeyCount: number;
  setWalletKeyCount: (count: number) => void;
  setUnsyncedKeyCount: (count: number) => void;

  // Wallet passphrase (session-only, never persisted)
  walletPassphrase: string | null;
  setWalletPassphrase: (passphrase: string | null) => void;

  // Cached balance (IndexedDB-backed, shown immediately on load)
  cachedBalance: string | null; // stringified bigint, null = unknown
  lastValidatedAt: Record<string, number>; // chainId → Unix ms timestamp
  setCachedBalance: (balance: string | null) => void;
  setLastValidatedAt: (chainId: string, ts: number) => void;

  // Relay sync
  relayUrl: string;
  relayConnected: boolean;
  setRelayUrl: (url: string) => void;
  setRelayConnected: (connected: boolean) => void;

  // Multi-recorder connections for investor view
  recorderUrls: string[];
  setRecorderUrls: (urls: string[]) => void;

  // Notifications
  notification: NotificationSettings;
  setNotification: (patch: Partial<NotificationSettings>) => void;

  // UI
  view: 'vendor' | 'consumer' | 'investor' | 'cooperative';
  setView: (view: 'vendor' | 'consumer' | 'investor' | 'cooperative') => void;
  error: string | null;
  setError: (error: string | null) => void;

  // Power-user features
  showRefutation: boolean;
  setShowRefutation: (show: boolean) => void;

  // Cooperative metadata
  pendingCoopNote: string | null;
  setPendingCoopNote: (note: string | null) => void;

  // CAA Escrow
  activeEscrows: EscrowEntry[];
  addEscrow: (entry: EscrowEntry) => void;
  updateEscrow: (id: string, patch: Partial<EscrowEntry>) => void;
  removeEscrow: (id: string) => void;
}

/** A tracked CAA escrow swap. */
export interface EscrowEntry {
  id: string;              // client-side unique ID (crypto.randomUUID)
  caaHash: string;         // server-assigned, empty until first submission
  status: 'submitting' | 'escrowed' | 'binding' | 'finalized' | 'expired' | 'failed';
  chains: { chainId: string; recorderUrl: string; symbol: string }[];
  createdAt: number;       // Unix ms
  deadlineUnixSecs: number;
  errorMessage?: string;
}

export const useStore = create<AppState>((set) => ({
  recorderUrl: loadString('ao_recorder_url') ?? 'http://localhost:3000',
  connected: false,
  setRecorderUrl: (recorderUrl) => {
    saveString('ao_recorder_url', recorderUrl);
    set({ recorderUrl });
  },
  setConnected: (connected) => set({ connected }),

  chains: [],
  setChains: (chains) => set({ chains }),
  selectedChainId: null,
  selectChain: (selectedChainId) => set({ selectedChainId, chainInfo: null }),
  chainInfo: null,
  setChainInfo: (chainInfo) => set({ chainInfo }),

  walletLabel: null,
  publicKeyHex: null,
  seedHex: null,
  setWallet: (walletLabel, publicKeyHex, seedHex) => {
    set({ walletLabel, publicKeyHex, seedHex });
  },
  clearWallet: () => {
    set({ walletLabel: null, publicKeyHex: null, seedHex: null });
  },

  walletKeyCount: 0,
  unsyncedKeyCount: 0,
  setWalletKeyCount: (walletKeyCount) => set({ walletKeyCount }),
  setUnsyncedKeyCount: (unsyncedKeyCount) => set({ unsyncedKeyCount }),

  walletPassphrase: null,
  setWalletPassphrase: (walletPassphrase) => set({ walletPassphrase }),

  cachedBalance: null,
  lastValidatedAt: {},
  setCachedBalance: (cachedBalance) => set({ cachedBalance }),
  setLastValidatedAt: (chainId, ts) => set(state => ({
    lastValidatedAt: { ...state.lastValidatedAt, [chainId]: ts },
  })),

  relayUrl: loadString('ao_relay_url') ?? 'ws://localhost:3001',
  relayConnected: false,
  setRelayUrl: (relayUrl) => {
    saveString('ao_relay_url', relayUrl);
    set({ relayUrl });
  },
  setRelayConnected: (relayConnected) => set({ relayConnected }),

  recorderUrls: [],
  setRecorderUrls: (recorderUrls) => set({ recorderUrls }),

  notification: {
    enabled: loadString('ao_notify_enabled') !== 'false', // default on
    chimeStyle: (() => { const s = loadString('ao_notify_chime'); return s && isChimeStyle(s) ? s : 'bell'; })(),
    volume: Math.max(0, Math.min(1, loadNumber('ao_notify_volume', 0.7))),
    quietStart: Math.max(0, Math.min(23, Math.floor(loadNumber('ao_notify_quiet_start', 22)))),
    quietEnd: Math.max(0, Math.min(23, Math.floor(loadNumber('ao_notify_quiet_end', 8)))),
    quickMuteUntil: null, // session-only, not persisted
  },
  setNotification: (patch) => set(state => {
    const next = { ...state.notification, ...patch };
    saveString('ao_notify_enabled', String(next.enabled));
    saveString('ao_notify_chime', next.chimeStyle);
    saveString('ao_notify_volume', String(next.volume));
    saveString('ao_notify_quiet_start', String(next.quietStart));
    saveString('ao_notify_quiet_end', String(next.quietEnd));
    return { notification: next };
  }),

  view: 'consumer',
  setView: (view) => set({ view }),
  error: null,
  setError: (error) => set({ error }),

  showRefutation: loadString('ao_show_refutation') === 'true',
  setShowRefutation: (showRefutation) => {
    saveString('ao_show_refutation', String(showRefutation));
    set({ showRefutation });
  },

  pendingCoopNote: null,
  setPendingCoopNote: (note) => set({ pendingCoopNote: note }),

  activeEscrows: loadEscrows(),
  addEscrow: (entry) => set(state => {
    const next = [...state.activeEscrows, entry];
    saveEscrows(next);
    return { activeEscrows: next };
  }),
  updateEscrow: (id, patch) => set(state => {
    const next = state.activeEscrows.map(e =>
      e.id === id ? { ...e, ...patch } : e,
    );
    saveEscrows(next);
    return { activeEscrows: next };
  }),
  removeEscrow: (id) => set(state => {
    const next = state.activeEscrows.filter(e => e.id !== id);
    saveEscrows(next);
    return { activeEscrows: next };
  }),
}));

function loadEscrows(): EscrowEntry[] {
  const s = loadString('ao_escrows');
  if (!s) return [];
  try { return JSON.parse(s); } catch { return []; }
}

function saveEscrows(escrows: EscrowEntry[]) {
  saveString('ao_escrows', JSON.stringify(escrows));
}
