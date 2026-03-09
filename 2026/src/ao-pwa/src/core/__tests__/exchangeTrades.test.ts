import { describe, it, expect, vi, beforeEach } from 'vitest';
import { fetchTrades, fetchExchangeStatus } from '../exchangeTrades.ts';

// Mock fetch globally
const mockFetch = vi.fn();
vi.stubGlobal('fetch', mockFetch);

beforeEach(() => {
  mockFetch.mockReset();
});

describe('fetchTrades', () => {
  it('fetches trades with no query params', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({
        trades: [{ trade_id: 't1', buy_symbol: 'BCG', sell_symbol: 'RMF', status: 'completed' }],
        total: 1,
        pnl: [],
      }),
    });

    const result = await fetchTrades('http://localhost:3100');
    expect(result.trades).toHaveLength(1);
    expect(result.trades[0].trade_id).toBe('t1');
    expect(mockFetch).toHaveBeenCalledWith('http://localhost:3100/trades');
  });

  it('passes query parameters', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({ trades: [], total: 0, pnl: [] }),
    });

    await fetchTrades('http://localhost:3100', {
      from: 1000,
      to: 2000,
      symbol: 'BCG',
      status: 'completed',
      limit: 50,
    });

    const url = mockFetch.mock.calls[0][0] as string;
    expect(url).toContain('from=1000');
    expect(url).toContain('to=2000');
    expect(url).toContain('symbol=BCG');
    expect(url).toContain('status=completed');
    expect(url).toContain('limit=50');
  });

  it('throws on HTTP error', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 500,
      text: () => Promise.resolve('server error'),
    });

    await expect(fetchTrades('http://localhost:3100'))
      .rejects.toThrow('500: server error');
  });
});

describe('fetchExchangeStatus', () => {
  it('fetches status', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: true,
      json: () => Promise.resolve({
        pairs: [{ sell: 'BCG', buy: 'RMF', rate: 2.0, spread: 0.02 }],
        positions: [{ symbol: 'BCG', balance: '1000' }],
        pending_trades: 3,
      }),
    });

    const result = await fetchExchangeStatus('http://localhost:3100');
    expect(result.pending_trades).toBe(3);
    expect(result.positions).toHaveLength(1);
    expect(result.pairs).toHaveLength(1);
    expect(mockFetch).toHaveBeenCalledWith('http://localhost:3100/status');
  });

  it('throws on HTTP error', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 404,
      text: () => Promise.resolve('not found'),
    });

    await expect(fetchExchangeStatus('http://localhost:3100'))
      .rejects.toThrow('404');
  });
});
