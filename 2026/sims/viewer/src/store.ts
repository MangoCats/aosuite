import { create } from 'zustand';
import type { AgentState, TransactionEvent, ChainSummary, ScenarioMeta } from './api';

export interface Toast {
  id: number;
  text: string;
  timestamp: number;
}

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

  // Speed
  speed: number;
  setSpeed: (s: number) => void;

  // Map overlays
  showHeatMap: boolean;
  toggleHeatMap: () => void;
  showCoverage: boolean;
  toggleCoverage: () => void;
  showAuditOverlay: boolean;
  toggleAuditOverlay: () => void;
  showCaaArcs: boolean;
  toggleCaaArcs: () => void;

  // Agent control
  setAgentPaused: (name: string, paused: boolean) => void;

  // Onboarding
  scenarioMeta: ScenarioMeta | null;
  setScenarioMeta: (meta: ScenarioMeta) => void;
  showWelcome: boolean;
  setShowWelcome: (show: boolean) => void;

  // Toasts
  toasts: Toast[];
  addToast: (text: string) => void;
  removeToast: (id: number) => void;
  toastsMuted: boolean;
  toggleToastsMuted: () => void;
}

let nextToastId = 1;

export const useStore = create<ViewerStore>((set) => ({
  agents: [],
  transactions: [],
  chains: [],
  selectedAgent: null,
  tab: 'map',

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

  speed: 1,
  setSpeed: (speed) => set({ speed }),

  showHeatMap: false,
  toggleHeatMap: () => set((s) => ({ showHeatMap: !s.showHeatMap })),
  showCoverage: false,
  toggleCoverage: () => set((s) => ({ showCoverage: !s.showCoverage })),
  showAuditOverlay: false,
  toggleAuditOverlay: () => set((s) => ({ showAuditOverlay: !s.showAuditOverlay })),
  showCaaArcs: true,
  toggleCaaArcs: () => set((s) => ({ showCaaArcs: !s.showCaaArcs })),

  setAgentPaused: (name, paused) => set((s) => ({
    agents: s.agents.map((a) => a.name === name ? { ...a, paused } : a),
  })),

  // Onboarding
  scenarioMeta: null,
  setScenarioMeta: (scenarioMeta) => set({ scenarioMeta }),
  showWelcome: true,
  setShowWelcome: (showWelcome) => set({ showWelcome }),

  // Toasts
  toasts: [],
  addToast: (text) => set((s) => ({
    toasts: [...s.toasts.slice(-2), { id: nextToastId++, text, timestamp: Date.now() }],
  })),
  removeToast: (id) => set((s) => ({
    toasts: s.toasts.filter((t) => t.id !== id),
  })),
  toastsMuted: false,
  toggleToastsMuted: () => set((s) => ({ toastsMuted: !s.toastsMuted })),
}));
