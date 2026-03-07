import { create } from 'zustand';
import type { AgentState, TransactionEvent, ChainSummary } from './api';

export interface ViewerStore {
  agents: AgentState[];
  transactions: TransactionEvent[];
  chains: ChainSummary[];
  selectedAgent: string | null;
  tab: 'agents' | 'chains' | 'transactions' | 'map';

  setAgents: (agents: AgentState[]) => void;
  addTransactions: (txns: TransactionEvent[]) => void;
  setChains: (chains: ChainSummary[]) => void;
  selectAgent: (name: string | null) => void;
  setTab: (tab: 'agents' | 'chains' | 'transactions' | 'map') => void;

  // Sorting
  agentSort: { key: string; asc: boolean };
  setAgentSort: (key: string) => void;
  txSort: { key: string; asc: boolean };
  setTxSort: (key: string) => void;

  // Filters
  agentFilter: string;
  setAgentFilter: (f: string) => void;
  txFilter: string;
  setTxFilter: (f: string) => void;

  // Time controls
  paused: boolean;
  setPaused: (p: boolean) => void;
  timeFilter: number | null;  // null = live (show all), number = max timestamp_ms
  setTimeFilter: (t: number | null) => void;
}

export const useStore = create<ViewerStore>((set) => ({
  agents: [],
  transactions: [],
  chains: [],
  selectedAgent: null,
  tab: 'agents',

  setAgents: (agents) => set({ agents }),
  addTransactions: (txns) => set((s) => {
    const existing = new Set(s.transactions.map(t => t.id));
    const novel = txns.filter(t => !existing.has(t.id));
    return { transactions: [...s.transactions, ...novel] };
  }),
  setChains: (chains) => set({ chains }),
  selectAgent: (name) => set({ selectedAgent: name }),
  setTab: (tab) => set({ tab }),

  agentSort: { key: 'name', asc: true },
  setAgentSort: (key) => set((s) => ({
    agentSort: s.agentSort.key === key
      ? { key, asc: !s.agentSort.asc }
      : { key, asc: true },
  })),
  txSort: { key: 'id', asc: false },
  setTxSort: (key) => set((s) => ({
    txSort: s.txSort.key === key
      ? { key, asc: !s.txSort.asc }
      : { key, asc: true },
  })),

  agentFilter: '',
  setAgentFilter: (agentFilter) => set({ agentFilter }),
  txFilter: '',
  setTxFilter: (txFilter) => set({ txFilter }),

  paused: false,
  setPaused: (paused) => set({ paused }),
  timeFilter: null,
  setTimeFilter: (timeFilter) => set({ timeFilter }),
}));
