import { useState, useEffect } from 'react';
import { RecorderClient } from '../api/client.ts';
import { loadCachedTx, getLastScannedHeight, saveTxRecords, parseBlocks } from '../core/txHistory.ts';
import type { TxRecord } from '../core/txHistory.ts';
import { walletDb } from '../core/walletDb.ts';
import {
  aggregateSales, filterByDateRange, sharesToCoins, salesReportCsv,
  type Period, type SalesSummary,
} from '../core/salesReport.ts';

interface SalesReportProps {
  recorderUrl: string;
  chainId: string;
  symbol: string;
  coinCount: string;
}

export function SalesReport({ recorderUrl, chainId, symbol, coinCount }: SalesReportProps) {
  const [records, setRecords] = useState<TxRecord[]>([]);
  const [period, setPeriod] = useState<Period>('daily');
  const [summaries, setSummaries] = useState<SalesSummary[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Date range: default last 30 days
  const [startDate, setStartDate] = useState(() => {
    const d = new Date();
    d.setDate(d.getDate() - 30);
    return d.toISOString().split('T')[0];
  });
  const [endDate, setEndDate] = useState(() => new Date().toISOString().split('T')[0]);

  // Load cached data on mount
  useEffect(() => {
    loadCachedTx(chainId).then(cached => {
      setRecords(cached);
    }).catch(() => {});
  }, [chainId]);

  // Re-aggregate when records, period, or date range changes
  useEffect(() => {
    const startMs = new Date(startDate).getTime();
    const endMs = new Date(endDate).getTime() + 86400000; // include end date
    const filtered = filterByDateRange(records, startMs, endMs);
    setSummaries(aggregateSales(filtered, period));
  }, [records, period, startDate, endDate]);

  async function handleRefresh() {
    setLoading(true);
    setError(null);
    try {
      const client = new RecorderClient(recorderUrl);
      const info = await client.chainInfo(chainId);
      const currentHeight = info.block_height;
      const lastScanned = await getLastScannedHeight(chainId);

      if (lastScanned < currentHeight) {
        const keys = await walletDb.getKeys(chainId);
        const walletPubkeys = new Set(keys.map(k => k.publicKey));
        const walletSeqIds = new Map<number, string>();
        for (const k of keys) {
          if (k.seqId !== null) walletSeqIds.set(k.seqId, k.publicKey);
        }

        // Fetch in batches of 100
        const allNew: TxRecord[] = [];
        for (let from = lastScanned; from < currentHeight; from += 100) {
          const to = Math.min(from + 100, currentHeight);
          const blocks = await client.getBlocks(chainId, from, to);
          const parsed = parseBlocks(blocks, walletPubkeys, walletSeqIds, from);
          allNew.push(...parsed);
        }

        await saveTxRecords(chainId, allNew, currentHeight);
      }

      const all = await loadCachedTx(chainId);
      setRecords(all);
    } catch (e) {
      setError(`Refresh failed: ${e}`);
    }
    setLoading(false);
  }

  function handleExportCsv() {
    const startMs = new Date(startDate).getTime();
    const endMs = new Date(endDate).getTime() + 86400000;
    const filtered = filterByDateRange(records, startMs, endMs);
    const agg = aggregateSales(filtered, period);
    const csv = salesReportCsv(agg, symbol, coinCount, period);
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const link = document.createElement('a');
    link.download = `sales-${symbol.toLowerCase()}-${period}.csv`;
    link.href = url;
    link.click();
    URL.revokeObjectURL(url);
  }

  const coinTotal = BigInt(coinCount || '0');

  // Grand totals
  const grandShares = summaries.reduce((sum, s) => sum + s.totalShares, 0n);
  const grandTxCount = summaries.reduce((sum, s) => sum + s.txCount, 0);

  return (
    <div style={{ marginBottom: 16, padding: 12, background: '#f9f9f9', borderRadius: 4 }}>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>Sales Report</div>

      {error && (
        <div style={{ color: '#c00', fontSize: 12, marginBottom: 8 }}>{error}</div>
      )}

      {/* Controls */}
      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginBottom: 8, alignItems: 'center' }}>
        <select value={period} onChange={e => setPeriod(e.target.value as Period)}
          style={{ padding: '4px 6px', fontSize: 13 }}>
          <option value="daily">Daily</option>
          <option value="weekly">Weekly</option>
          <option value="monthly">Monthly</option>
        </select>

        <input type="date" value={startDate} onChange={e => setStartDate(e.target.value)}
          style={{ padding: '4px 6px', fontSize: 13 }} />
        <span style={{ fontSize: 13 }}>to</span>
        <input type="date" value={endDate} onChange={e => setEndDate(e.target.value)}
          style={{ padding: '4px 6px', fontSize: 13 }} />

        <button onClick={handleRefresh} disabled={loading} style={{ fontSize: 12, padding: '4px 8px' }}>
          {loading ? 'Scanning...' : 'Refresh'}
        </button>
        <button onClick={handleExportCsv} disabled={summaries.length === 0}
          style={{ fontSize: 12, padding: '4px 8px' }}>
          Export CSV
        </button>
      </div>

      {/* Summary table */}
      {summaries.length === 0 ? (
        <div style={{ fontSize: 12, color: '#999' }}>
          {records.length === 0 ? 'No transaction data — click Refresh to scan.' : 'No sales in selected range.'}
        </div>
      ) : (
        <>
          <div style={{ maxHeight: 250, overflow: 'auto' }}>
            <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse', fontFamily: 'monospace' }}>
              <thead>
                <tr style={{ borderBottom: '2px solid #ccc', textAlign: 'left' }}>
                  <th style={{ padding: '4px 6px' }}>Period</th>
                  <th style={{ padding: '4px 6px', textAlign: 'right' }}>Revenue</th>
                  <th style={{ padding: '4px 6px', textAlign: 'right' }}>Txns</th>
                  <th style={{ padding: '4px 6px', textAlign: 'right' }}>Avg</th>
                </tr>
              </thead>
              <tbody>
                {summaries.map(s => (
                  <tr key={s.label} style={{ borderBottom: '1px solid #eee' }}>
                    <td style={{ padding: '4px 6px' }}>{s.label}</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right' }}>
                      {coinTotal > 0n ? sharesToCoins(s.totalShares, coinTotal) : s.totalShares.toString()}
                    </td>
                    <td style={{ padding: '4px 6px', textAlign: 'right' }}>{s.txCount}</td>
                    <td style={{ padding: '4px 6px', textAlign: 'right' }}>
                      {coinTotal > 0n && s.txCount > 0
                        ? sharesToCoins(s.totalShares / BigInt(s.txCount), coinTotal)
                        : s.txCount > 0
                          ? (s.totalShares / BigInt(s.txCount)).toString()
                          : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
              <tfoot>
                <tr style={{ borderTop: '2px solid #ccc', fontWeight: 'bold' }}>
                  <td style={{ padding: '4px 6px' }}>Total</td>
                  <td style={{ padding: '4px 6px', textAlign: 'right' }}>
                    {coinTotal > 0n ? sharesToCoins(grandShares, coinTotal) : grandShares.toString()}
                  </td>
                  <td style={{ padding: '4px 6px', textAlign: 'right' }}>{grandTxCount}</td>
                  <td style={{ padding: '4px 6px', textAlign: 'right' }}>
                    {coinTotal > 0n && grandTxCount > 0
                      ? sharesToCoins(grandShares / BigInt(grandTxCount), coinTotal)
                      : '—'}
                  </td>
                </tr>
              </tfoot>
            </table>
          </div>
        </>
      )}
    </div>
  );
}
