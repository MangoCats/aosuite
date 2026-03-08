// Viewer API client — connects to ao-sims viewer server.

export interface ChainHolding {
  chain_id: string;
  symbol: string;
  shares: string;
  unspent_utxos: number;
  coin_count: string;
  total_shares: string;
}

export interface WalletChainSummary {
  chain_id: string;
  total_keys: number;
  unspent_keys: number;
  spent_keys: number;
  total_unspent_amount: string;
  oldest_unspent_ms: number | null;
}

export interface AgentState {
  name: string;
  role: string;
  status: string;
  lat: number;
  lon: number;
  chains: ChainHolding[];
  key_summary: WalletChainSummary[];
  coverage_radius: number | null;
  paused: boolean;
  trading_rates: TradingRate[];
  validator_status: ValidatorStatus | null;
  attacker_status: AttackerStatus | null;
  caa_status: CaaExchangeStatus | null;
  transactions: number;
  last_action: string;
}

export interface ValidatorStatus {
  monitored_chains: MonitoredChainStatus[];
  alerts: AlertEntry[];
  total_blocks_verified: number;
}

export interface MonitoredChainStatus {
  chain_id: string;
  symbol: string;
  validated_height: number;
  chain_height: number;
  status: string;
  last_poll_ms: number;
}

export interface AlertEntry {
  timestamp_ms: number;
  chain_id: string;
  alert_type: string;
  message: string;
}

export interface AttackerStatus {
  attack_type: string;
  attempts: number;
  rejections: number;
  unexpected_accepts: number;
  last_result: string;
}

export interface CaaExchangeStatus {
  total_caas: number;
  successful: number;
  failed: number;
  last_caa_hash: string;
  last_status: string;
}

export interface TradingRate {
  sell: string;
  buy: string;
  rate: number;
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

export async function fetchSpeed(): Promise<number> {
  const res = await fetch(`${BASE}/api/speed`);
  const data = await res.json();
  return data.speed;
}

export async function setSpeed(speed: number): Promise<number> {
  const res = await fetch(`${BASE}/api/speed`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ speed }),
  });
  const data = await res.json();
  return data.speed;
}

export async function pauseAgent(name: string): Promise<void> {
  await fetch(`${BASE}/api/agents/${name}/pause`, { method: 'POST' });
}

export async function resumeAgent(name: string): Promise<void> {
  await fetch(`${BASE}/api/agents/${name}/resume`, { method: 'POST' });
}

export function connectWs(onMessage: (msg: WsMessage) => void): { close: () => void } {
  let ws: WebSocket | null = null;
  let closed = false;
  let retryDelay = 1000;

  function connect() {
    if (closed) return;
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/api/ws`;
    ws = new WebSocket(wsUrl);
    ws.onmessage = (e) => {
      const msg = JSON.parse(e.data) as WsMessage;
      onMessage(msg);
    };
    ws.onopen = () => {
      retryDelay = 1000; // reset on successful connect
    };
    ws.onclose = () => {
      if (!closed) setTimeout(connect, retryDelay);
      retryDelay = Math.min(retryDelay * 2, 30000);
    };
    ws.onerror = () => {
      ws?.close();
    };
  }

  connect();

  return {
    close: () => {
      closed = true;
      ws?.close();
    },
  };
}
