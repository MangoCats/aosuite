// Global application store — Zustand with localStorage persistence for wallet.

import { create } from 'zustand';
import type { ChainInfo, ChainListEntry } from '../api/client.ts';

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

  // Wallet — seed persisted to localStorage
  walletLabel: string | null;
  publicKeyHex: string | null;
  seedHex: string | null;
  setWallet: (label: string, publicKeyHex: string, seedHex: string) => void;
  clearWallet: () => void;

  // Multi-recorder connections for investor view
  recorderUrls: string[];
  setRecorderUrls: (urls: string[]) => void;

  // UI
  view: 'vendor' | 'consumer' | 'investor';
  setView: (view: 'vendor' | 'consumer' | 'investor') => void;
  error: string | null;
  setError: (error: string | null) => void;
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

  walletLabel: loadString('ao_wallet_label'),
  publicKeyHex: loadString('ao_wallet_pubkey'),
  seedHex: loadString('ao_wallet_seed'),
  setWallet: (walletLabel, publicKeyHex, seedHex) => {
    saveString('ao_wallet_label', walletLabel);
    saveString('ao_wallet_pubkey', publicKeyHex);
    saveString('ao_wallet_seed', seedHex);
    set({ walletLabel, publicKeyHex, seedHex });
  },
  clearWallet: () => {
    saveString('ao_wallet_label', null);
    saveString('ao_wallet_pubkey', null);
    saveString('ao_wallet_seed', null);
    set({ walletLabel: null, publicKeyHex: null, seedHex: null });
  },

  recorderUrls: [],
  setRecorderUrls: (recorderUrls) => set({ recorderUrls }),

  view: 'consumer',
  setView: (view) => set({ view }),
  error: null,
  setError: (error) => set({ error }),
}));
