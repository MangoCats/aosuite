import { useState, useEffect, useCallback } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import * as walletDb from '../core/walletDb.ts';
import {
  parseBlocks, loadCachedTx, saveTxRecords, getLastScannedHeight,
  csvExporter, type TxRecord,
} from '../core/txHistory.ts';

export function TransactionHistory() {
  const { recorderUrl, selectedChainId, chainInfo } = useStore();
  const [records, setRecords] = useState<TxRecord[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Load cached transactions on mount / chain change
  useEffect(() => {
    if (!selectedChainId) return;
    loadCachedTx(selectedChainId).then(cached => {
      setRecords(cached.sort((a, b) => b.blockHeight - a.blockHeight || b.pageIndex - a.pageIndex));
    });
  }, [selectedChainId]);

  const scanHistory = useCallback(async () => {
    if (!selectedChainId || !recorderUrl) return;
    setLoading(true);
    setError(null);
    try {
      const client = new RecorderClient(recorderUrl);

      // Get wallet keys to match against
      const keys = await walletDb.getKeys(selectedChainId);
      const walletPubkeys = new Set(keys.map(k => k.publicKey));
      const walletSeqIds = new Map<number, string>();
      for (const k of keys) {
        if (k.seqId !== null) walletSeqIds.set(k.seqId, k.publicKey);
      }

      // Get current chain height
      const info = await client.chainInfo(selectedChainId);
      const currentHeight = info.block_height;

      // Fetch from last scanned position
      const lastScanned = await getLastScannedHeight(selectedChainId);
      if (lastScanned >= currentHeight) {
        // Already up to date — just reload cache
        const cached = await loadCachedTx(selectedChainId);
        setRecords(cached.sort((a, b) => b.blockHeight - a.blockHeight || b.pageIndex - a.pageIndex));
        setLoading(false);
        return;
      }

      // Fetch blocks in batches of 100
      const BATCH = 100;
      let allNew: TxRecord[] = [];
      for (let from = lastScanned; from < currentHeight; from += BATCH) {
        const to = Math.min(from + BATCH, currentHeight);
        const blocks = await client.getBlocks(selectedChainId, from, to);
        const parsed = parseBlocks(blocks, walletPubkeys, walletSeqIds, from);
        allNew = allNew.concat(parsed);
      }

      // Save to IndexedDB
      if (allNew.length > 0) {
        await saveTxRecords(selectedChainId, allNew, currentHeight);
      }

      // Reload full cache
      const cached = await loadCachedTx(selectedChainId);
      setRecords(cached.sort((a, b) => b.blockHeight - a.blockHeight || b.pageIndex - a.pageIndex));
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Scan failed');
    } finally {
      setLoading(false);
    }
  }, [selectedChainId, recorderUrl]);

  const handleExportCsv = useCallback(() => {
    if (records.length === 0) return;
    const symbol = chainInfo?.symbol ?? 'AO';
    const coinCount = chainInfo?.coin_count ?? '0';
    const csv = csvExporter.export(records, symbol, coinCount);
    const blob = new Blob([csv], { type: 'text/csv' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `tx-history-${selectedChainId?.slice(0, 8)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  }, [records, chainInfo, selectedChainId]);

  if (!selectedChainId) return null;

  return (
    <div style={{ marginTop: 16 }}>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 8 }}>
        <span style={{ fontSize: 13, fontWeight: 500 }}>Transaction History</span>
        <button onClick={scanHistory} disabled={loading} style={{ fontSize: 12 }}>
          {loading ? 'Scanning...' : 'Refresh'}
        </button>
        {records.length > 0 && (
          <button onClick={handleExportCsv} style={{ fontSize: 12 }}>
            Export CSV
          </button>
        )}
      </div>
      {error && (
        <div style={{ color: '#c00', fontSize: 12, marginBottom: 4 }}>{error}</div>
      )}
      {records.length === 0 && !loading && (
        <div style={{ fontSize: 12, color: '#666' }}>
          No transactions found. Tap Refresh to scan blocks.
        </div>
      )}
      {records.length > 0 && (
        <div style={{ maxHeight: 300, overflow: 'auto', border: '1px solid #eee', borderRadius: 4 }}>
          <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
            <thead>
              <tr style={{ background: '#f5f5f5', position: 'sticky', top: 0 }}>
                <th style={thStyle}>Date</th>
                <th style={thStyle}>Dir</th>
                <th style={thStyle}>Amount</th>
                <th style={thStyle}>Counterparty</th>
                <th style={thStyle}>Block</th>
              </tr>
            </thead>
            <tbody>
              {records.map((r, i) => (
                <tr key={i} style={{ borderBottom: '1px solid #f0f0f0' }}>
                  <td style={tdStyle}>{formatDate(r.timestampMs)}</td>
                  <td style={{
                    ...tdStyle,
                    color: r.direction === 'received' ? '#090' : '#c00',
                    fontWeight: 500,
                  }}>
                    {r.direction === 'received' ? '+' : '-'}
                  </td>
                  <td style={{ ...tdStyle, fontFamily: 'monospace' }}>{r.amount}</td>
                  <td style={{ ...tdStyle, fontFamily: 'monospace' }}>
                    {r.counterparty ? r.counterparty.slice(0, 12) + '...' : '—'}
                    {r.hasBlob && ' 📎'}
                  </td>
                  <td style={tdStyle}>#{r.blockHeight}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

const thStyle: React.CSSProperties = {
  padding: '4px 8px', textAlign: 'left', fontWeight: 500, fontSize: 11,
};

const tdStyle: React.CSSProperties = {
  padding: '3px 8px',
};

function formatDate(ms: number): string {
  if (!ms) return '—';
  const d = new Date(ms);
  return d.toLocaleDateString(undefined, { month: 'short', day: 'numeric' })
    + ' ' + d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}
