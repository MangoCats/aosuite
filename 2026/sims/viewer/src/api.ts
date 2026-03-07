// Viewer API client — connects to ao-sims viewer server.

export interface ChainHolding {
  chain_id: string;
  symbol: string;
  shares: string;
  unspent_utxos: number;
}

export interface AgentState {
  name: string;
  role: string;
  status: string;
  chains: ChainHolding[];
  transactions: number;
  last_action: string;
}

export interface TransactionEvent {
  id: number;
  timestamp_ms: number;
  chain_id: string;
  symbol: string;
  from_agent: string;
  to_agent: string;
  block_height: number;
  description: string;
}

export interface ChainSummary {
  chain_id: string;
  symbol: string;
  total_utxos: number;
  agents: string[];
}

export interface WsMessage {
  type: 'snapshot' | 'update';
  agents: AgentState[];
  transactions: TransactionEvent[];
}

const BASE = ''; // proxied via vite dev server

export async function fetchAgents(): Promise<AgentState[]> {
  const res = await fetch(`${BASE}/api/agents`);
  return res.json();
}

export async function fetchAgent(name: string): Promise<AgentState> {
  const res = await fetch(`${BASE}/api/agents/${name}`);
  return res.json();
}

export async function fetchChains(): Promise<ChainSummary[]> {
  const res = await fetch(`${BASE}/api/chains`);
  return res.json();
}

export async function fetchTransactions(since = 0, limit = 200): Promise<TransactionEvent[]> {
  const res = await fetch(`${BASE}/api/transactions?since=${since}&limit=${limit}`);
  return res.json();
}

export async function fetchAgentTransactions(name: string): Promise<TransactionEvent[]> {
  const res = await fetch(`${BASE}/api/agents/${name}/transactions`);
  return res.json();
}

export function connectWs(onMessage: (msg: WsMessage) => void): WebSocket {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const wsUrl = `${protocol}//${window.location.host}/api/ws`;
  const ws = new WebSocket(wsUrl);
  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data) as WsMessage;
    onMessage(msg);
  };
  return ws;
}
