// Global application store — Zustand

import { create } from 'zustand';
import type { ChainInfo, ChainListEntry } from '../api/client.ts';

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

  // Wallet
  walletLabel: string | null;
  publicKeyHex: string | null;
  setWallet: (label: string, publicKeyHex: string) => void;
  clearWallet: () => void;

  // UI
  view: 'vendor' | 'consumer';
  setView: (view: 'vendor' | 'consumer') => void;
  error: string | null;
  setError: (error: string | null) => void;
}

export const useStore = create<AppState>((set) => ({
  recorderUrl: 'http://localhost:3000',
  connected: false,
  setRecorderUrl: (recorderUrl) => set({ recorderUrl }),
  setConnected: (connected) => set({ connected }),

  chains: [],
  setChains: (chains) => set({ chains }),
  selectedChainId: null,
  selectChain: (selectedChainId) => set({ selectedChainId, chainInfo: null }),
  chainInfo: null,
  setChainInfo: (chainInfo) => set({ chainInfo }),

  walletLabel: null,
  publicKeyHex: null,
  setWallet: (walletLabel, publicKeyHex) => set({ walletLabel, publicKeyHex }),
  clearWallet: () => set({ walletLabel: null, publicKeyHex: null }),

  view: 'consumer',
  setView: (view) => set({ view }),
  error: null,
  setError: (error) => set({ error }),
}));
