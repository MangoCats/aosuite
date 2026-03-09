// Exchange trade history API client and types.

export interface TradeRecord {
  trade_id: string;
  buy_symbol: string;
  sell_symbol: string;
  buy_chain_id: string;
  sell_chain_id: string;
  buy_amount: string;
  sell_amount: string;
  rate: number;
  spread: number;
  status: string;
  requested_at: number;
  completed_at: number;
  error_message: string | null;
}

export interface PairPnl {
  pair: string;
  trade_count: number;
  total_buy: string;
  total_sell: string;
}

export interface TradesResponse {
  trades: TradeRecord[];
  total: number;
  pnl: PairPnl[];
}

export interface TradesQuery {
  from?: number;
  to?: number;
  symbol?: string;
  status?: string;
  limit?: number;
  offset?: number;
}

/** Fetch trade history from an exchange agent daemon. */
export async function fetchTrades(
  exchangeUrl: string,
  query: TradesQuery = {},
): Promise<TradesResponse> {
  const params = new URLSearchParams();
  if (query.from !== undefined) params.set('from', String(query.from));
  if (query.to !== undefined) params.set('to', String(query.to));
  if (query.symbol) params.set('symbol', query.symbol);
  if (query.status) params.set('status', query.status);
  if (query.limit !== undefined) params.set('limit', String(query.limit));
  if (query.offset !== undefined) params.set('offset', String(query.offset));

  const qs = params.toString();
  const res = await fetch(`${exchangeUrl}/trades${qs ? '?' + qs : ''}`);
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`${res.status}: ${body}`);
  }
  return res.json();
}

export interface ExchangeStatus {
  pairs: { sell: string; buy: string; rate: number; spread: number }[];
  positions: { symbol: string; balance: string; low_stock: boolean }[];
  pending_trades: number;
}

/** Fetch exchange agent status. */
export async function fetchExchangeStatus(exchangeUrl: string): Promise<ExchangeStatus> {
  const res = await fetch(`${exchangeUrl}/status`);
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`${res.status}: ${body}`);
  }
  return res.json();
}
