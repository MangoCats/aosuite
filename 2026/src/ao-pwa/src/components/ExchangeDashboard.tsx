import { useState, useEffect, useCallback } from 'react';
import {
  fetchTrades, fetchExchangeStatus,
  type TradeRecord, type PairPnl, type ExchangeStatus,
} from '../core/exchangeTrades.ts';

interface Props {
  exchangeUrl: string;
}

type Period = '1d' | '7d' | '30d' | 'all';

function periodToSecs(period: Period): number | undefined {
  const now = Math.floor(Date.now() / 1000);
  switch (period) {
    case '1d': return now - 86400;
    case '7d': return now - 7 * 86400;
    case '30d': return now - 30 * 86400;
    case 'all': return undefined;
  }
}

export function ExchangeDashboard({ exchangeUrl }: Props) {
  const [trades, setTrades] = useState<TradeRecord[]>([]);
  const [total, setTotal] = useState(0);
  const [pnl, setPnl] = useState<PairPnl[]>([]);
  const [status, setStatus] = useState<ExchangeStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [period, setPeriod] = useState<Period>('7d');
  const [statusFilter, setStatusFilter] = useState<string>('');

  const loadData = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const [tradesResp, statusResp] = await Promise.all([
        fetchTrades(exchangeUrl, {
          from: periodToSecs(period),
          status: statusFilter || undefined,
          limit: 200,
        }),
        fetchExchangeStatus(exchangeUrl),
      ]);
      setTrades(tradesResp.trades);
      setTotal(tradesResp.total);
      setPnl(tradesResp.pnl);
      setStatus(statusResp);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Failed to load');
    } finally {
      setLoading(false);
    }
  }, [exchangeUrl, period, statusFilter]);

  useEffect(() => { loadData(); }, [loadData]);

  const handleExportCsv = useCallback(() => {
    if (trades.length === 0) return;
    const esc = (v: string) => /[",\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
    const header = 'trade_id,buy_symbol,sell_symbol,buy_amount,sell_amount,rate,spread,status,completed_at,error';
    const lines = trades.map(t =>
      [t.trade_id, t.buy_symbol, t.sell_symbol, t.buy_amount, t.sell_amount,
       String(t.rate), String(t.spread), t.status, String(t.completed_at),
       t.error_message ?? ''].map(esc).join(',')
    );
    const csv = [header, ...lines].join('\n');
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `exchange-trades.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [trades]);

  return (
    <div style={{ padding: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <span style={{ fontSize: 15, fontWeight: 600 }}>Exchange Dashboard</span>
        <button onClick={loadData} disabled={loading} style={{ fontSize: 12 }}>
          {loading ? 'Loading...' : 'Refresh'}
        </button>
      </div>

      {error && (
        <div style={{ color: '#c00', fontSize: 12, marginBottom: 8 }}>{error}</div>
      )}

      {/* Status summary */}
      {status && (
        <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap', marginBottom: 12 }}>
          <div style={{ padding: 8, background: '#f5f5f5', borderRadius: 4, fontSize: 12 }}>
            <div style={{ fontWeight: 500 }}>Pending Trades</div>
            <div style={{ fontSize: 18, fontWeight: 600 }}>{status.pending_trades}</div>
          </div>
          {status.positions.map(p => (
            <div key={p.symbol} style={{
              padding: 8,
              background: p.low_stock ? '#fff3e0' : '#f5f5f5',
              borderRadius: 4, fontSize: 12,
              border: p.low_stock ? '1px solid #ff9800' : 'none',
            }}>
              <div style={{ fontWeight: 500 }}>
                {p.symbol}
                {p.low_stock && <span style={{ color: '#e65100', marginLeft: 4 }}>LOW</span>}
              </div>
              <div style={{ fontSize: 14, fontFamily: 'monospace' }}>{p.balance}</div>
            </div>
          ))}
        </div>
      )}

      {/* P&L Summary */}
      {pnl.length > 0 && (
        <div style={{ marginBottom: 12, padding: 8, background: '#f9f9f9', borderRadius: 4 }}>
          <div style={{ fontSize: 12, fontWeight: 500, marginBottom: 4 }}>Trade Volume by Pair</div>
          {pnl.map(p => (
            <div key={p.pair} style={{ fontSize: 12, display: 'flex', gap: 12 }}>
              <span style={{ fontWeight: 500, width: 80 }}>{p.pair}</span>
              <span>{p.trade_count} trades</span>
              <span>bought: {p.total_buy}</span>
              <span>sold: {p.total_sell}</span>
            </div>
          ))}
        </div>
      )}

      {/* Controls */}
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 8, flexWrap: 'wrap' }}>
        {(['1d', '7d', '30d', 'all'] as Period[]).map(p => (
          <button
            key={p}
            onClick={() => setPeriod(p)}
            style={{
              fontSize: 11, padding: '2px 8px',
              fontWeight: period === p ? 600 : 400,
              background: period === p ? '#e0e0e0' : '#f5f5f5',
              border: '1px solid #ccc', borderRadius: 3,
            }}
          >
            {p}
          </button>
        ))}
        <select
          value={statusFilter}
          onChange={e => setStatusFilter(e.target.value)}
          style={{ fontSize: 11, padding: '2px 4px' }}
        >
          <option value="">All statuses</option>
          <option value="completed">Completed</option>
          <option value="failed">Failed</option>
          <option value="expired">Expired</option>
        </select>
        <span style={{ fontSize: 11, color: '#666' }}>
          {total} total
        </span>
        {trades.length > 0 && (
          <button onClick={handleExportCsv} style={{ fontSize: 11 }}>Export CSV</button>
        )}
      </div>

      {/* Trade list */}
      {trades.length > 0 && (
        <div style={{ maxHeight: 400, overflow: 'auto', border: '1px solid #eee', borderRadius: 4 }}>
          <table style={{ width: '100%', fontSize: 11, borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ background: '#f5f5f5', position: 'sticky', top: 0 }}>
                <th style={thStyle}>Date</th>
                <th style={thStyle}>Pair</th>
                <th style={thStyle}>Buy</th>
                <th style={thStyle}>Sell</th>
                <th style={thStyle}>Rate</th>
                <th style={thStyle}>Status</th>
              </tr>
            </thead>
            <tbody>
              {trades.map(t => (
                <tr key={t.trade_id} style={{ borderBottom: '1px solid #f0f0f0' }}>
                  <td style={tdStyle}>{formatDate(t.completed_at)}</td>
                  <td style={tdStyle}>{t.buy_symbol}/{t.sell_symbol}</td>
                  <td style={{ ...tdStyle, fontFamily: 'monospace' }}>{t.buy_amount}</td>
                  <td style={{ ...tdStyle, fontFamily: 'monospace' }}>{t.sell_amount}</td>
                  <td style={tdStyle}>{t.rate.toFixed(4)}</td>
                  <td style={{
                    ...tdStyle,
                    color: t.status === 'completed' ? '#090' : t.status === 'failed' ? '#c00' : '#666',
                  }}>
                    {t.status}
                    {t.error_message && (
                      <span title={t.error_message}> !</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {trades.length === 0 && !loading && (
        <div style={{ fontSize: 12, color: '#666' }}>No trades in this period.</div>
      )}
    </div>
  );
}

const thStyle: React.CSSProperties = {
  padding: '4px 6px', textAlign: 'left', fontWeight: 500, fontSize: 10,
};

const tdStyle: React.CSSProperties = {
  padding: '3px 6px',
};

function formatDate(secs: number): string {
  if (!secs) return '—';
  const d = new Date(secs * 1000);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
    + ' ' + d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}
