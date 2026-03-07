import { useState, useEffect, useCallback } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import type { ChainInfo } from '../api/client.ts';

interface PortfolioEntry {
  recorderUrl: string;
  chainId: string;
  symbol: string;
  sharesOut: string;
  coinCount: string;
  blockHeight: number;
}

export function InvestorView() {
  const { recorderUrl, recorderUrls, setRecorderUrls } = useStore();
  const [portfolio, setPortfolio] = useState<PortfolioEntry[]>([]);
  const [loading, setLoading] = useState(false);
  const [urlInput, setUrlInput] = useState('');

  // Include the main recorder URL plus any additional ones
  const allUrls = [recorderUrl, ...recorderUrls.filter(u => u !== recorderUrl)];

  const refreshPortfolio = useCallback(async () => {
    setLoading(true);
    const entries: PortfolioEntry[] = [];

    for (const url of allUrls) {
      try {
        const client = new RecorderClient(url);
        const chains = await client.listChains();
        for (const chain of chains) {
          const info: ChainInfo = await client.chainInfo(chain.chain_id);
          entries.push({
            recorderUrl: url,
            chainId: chain.chain_id,
            symbol: info.symbol,
            sharesOut: info.shares_out,
            coinCount: info.coin_count,
            blockHeight: info.block_height,
          });
        }
      } catch {
        // Skip unreachable recorders
      }
    }

    setPortfolio(entries);
    setLoading(false);
  }, [allUrls.join(',')]);

  useEffect(() => {
    refreshPortfolio();
    const interval = setInterval(refreshPortfolio, 10000);
    return () => clearInterval(interval);
  }, [refreshPortfolio]);

  const addRecorder = () => {
    const url = urlInput.trim();
    if (url && !allUrls.includes(url)) {
      setRecorderUrls([...recorderUrls, url]);
      setUrlInput('');
    }
  };

  const removeRecorder = (url: string) => {
    if (url !== recorderUrl) {
      setRecorderUrls(recorderUrls.filter(u => u !== url));
    }
  };

  return (
    <div style={{ padding: 16 }}>
      <h2 style={{ fontSize: 16, marginTop: 0 }}>Investor Portfolio</h2>

      <div style={{ marginBottom: 16 }}>
        <h3 style={{ fontSize: 14, marginBottom: 8 }}>Recorders</h3>
        {allUrls.map(url => (
          <div key={url} style={{ display: 'flex', gap: 8, alignItems: 'center', marginBottom: 4 }}>
            <span style={{ fontSize: 13, fontFamily: 'monospace' }}>{url}</span>
            {url !== recorderUrl && (
              <button onClick={() => removeRecorder(url)} style={{ fontSize: 11 }}>Remove</button>
            )}
            {url === recorderUrl && (
              <span style={{ fontSize: 11, color: '#666' }}>(primary)</span>
            )}
          </div>
        ))}
        <div style={{ display: 'flex', gap: 8, marginTop: 8 }}>
          <input
            value={urlInput}
            onChange={e => setUrlInput(e.target.value)}
            placeholder="http://recorder:3000"
            style={{ flex: 1, fontSize: 13 }}
            onKeyDown={e => e.key === 'Enter' && addRecorder()}
          />
          <button onClick={addRecorder}>Add</button>
        </div>
      </div>

      <div style={{ marginBottom: 16 }}>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <h3 style={{ fontSize: 14, marginBottom: 8 }}>Chain Holdings</h3>
          <button onClick={refreshPortfolio} disabled={loading} style={{ fontSize: 12 }}>
            {loading ? 'Loading...' : 'Refresh'}
          </button>
        </div>
        {portfolio.length === 0 ? (
          <p style={{ color: '#666', fontSize: 13 }}>No chains found.</p>
        ) : (
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
            <thead>
              <tr style={{ borderBottom: '2px solid #ddd', textAlign: 'left' }}>
                <th style={{ padding: '6px 8px' }}>Symbol</th>
                <th style={{ padding: '6px 8px' }}>Recorder</th>
                <th style={{ padding: '6px 8px', textAlign: 'right' }}>Block</th>
                <th style={{ padding: '6px 8px', textAlign: 'right' }}>Total Coins</th>
                <th style={{ padding: '6px 8px' }}>Chain ID</th>
              </tr>
            </thead>
            <tbody>
              {portfolio.map(entry => (
                <tr key={entry.chainId} style={{ borderBottom: '1px solid #eee' }}>
                  <td style={{ padding: '6px 8px', fontWeight: 'bold' }}>{entry.symbol}</td>
                  <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontSize: 11 }}>
                    {entry.recorderUrl}
                  </td>
                  <td style={{ padding: '6px 8px', textAlign: 'right' }}>{entry.blockHeight}</td>
                  <td style={{ padding: '6px 8px', textAlign: 'right', fontFamily: 'monospace' }}>
                    {entry.coinCount}
                  </td>
                  <td style={{ padding: '6px 8px', fontFamily: 'monospace', fontSize: 11, color: '#666' }}>
                    {entry.chainId.slice(0, 12)}...
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
