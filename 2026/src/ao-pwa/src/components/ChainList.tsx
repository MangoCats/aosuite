import { useEffect } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';

export function ChainList() {
  const { recorderUrl, chains, setChains, selectChain, selectedChainId, setConnected, setError } = useStore();

  useEffect(() => {
    const client = new RecorderClient(recorderUrl);
    let cancelled = false;

    async function load() {
      try {
        const list = await client.listChains();
        if (!cancelled) {
          setChains(list);
          setConnected(true);
          setError(null);
        }
      } catch (e) {
        if (!cancelled) {
          setConnected(false);
          setError(`Failed to connect: ${e}`);
        }
      }
    }

    load();
    const interval = setInterval(load, 10000);
    return () => { cancelled = true; clearInterval(interval); };
  }, [recorderUrl, setChains, setConnected, setError]);

  if (chains.length === 0) {
    return <div style={{ padding: 16, color: '#666' }}>No chains found. Start a recorder first.</div>;
  }

  return (
    <div style={{ padding: 16 }}>
      <h2 style={{ fontSize: 16, marginBottom: 8 }}>Chains</h2>
      <ul style={{ listStyle: 'none', padding: 0 }}>
        {chains.map(c => (
          <li key={c.chain_id} style={{ marginBottom: 4 }}>
            <button
              onClick={() => selectChain(c.chain_id)}
              style={{
                padding: '6px 12px',
                border: '1px solid #ccc',
                borderRadius: 4,
                background: selectedChainId === c.chain_id ? '#e0e8ff' : '#fff',
                cursor: 'pointer',
                width: '100%',
                textAlign: 'left',
              }}
            >
              <strong>{c.symbol}</strong>
              <span style={{ marginLeft: 8, fontSize: 12, color: '#666' }}>
                {c.chain_id.slice(0, 12)}... — block {c.block_height}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
