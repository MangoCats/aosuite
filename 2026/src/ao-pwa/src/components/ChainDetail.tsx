import { useEffect } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';

export function ChainDetail() {
  const { recorderUrl, selectedChainId, chainInfo, setChainInfo } = useStore();

  useEffect(() => {
    if (!selectedChainId) return;
    const client = new RecorderClient(recorderUrl);
    let cancelled = false;

    async function load() {
      try {
        const info = await client.chainInfo(selectedChainId!);
        if (!cancelled) setChainInfo(info);
      } catch {
        // ignore refresh errors
      }
    }

    load();
    const interval = setInterval(load, 5000);
    return () => { cancelled = true; clearInterval(interval); };
  }, [recorderUrl, selectedChainId, setChainInfo]);

  if (!selectedChainId) {
    return <div style={{ padding: 16, color: '#666' }}>Select a chain to view details.</div>;
  }

  if (!chainInfo) {
    return <div style={{ padding: 16 }}>Loading...</div>;
  }

  return (
    <div style={{ padding: 16 }}>
      <h2 style={{ fontSize: 16 }}>{chainInfo.symbol}</h2>
      <table style={{ borderCollapse: 'collapse', fontSize: 14 }}>
        <tbody>
          <Row label="Chain ID" value={chainInfo.chain_id} />
          <Row label="Block Height" value={String(chainInfo.block_height)} />
          <Row label="Shares Out" value={chainInfo.shares_out} />
          <Row label="Coins" value={chainInfo.coin_count} />
          <Row label="Fee Rate" value={`${chainInfo.fee_rate_num} / ${chainInfo.fee_rate_den}`} />
          <Row label="Expiry Period" value={`${chainInfo.expiry_period}s`} />
          <Row label="Expiry Mode" value={String(chainInfo.expiry_mode)} />
          <Row label="Next Seq ID" value={String(chainInfo.next_seq_id)} />
        </tbody>
      </table>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <tr>
      <td style={{ padding: '4px 12px 4px 0', fontWeight: 500, color: '#444' }}>{label}</td>
      <td style={{ padding: '4px 0', fontFamily: 'monospace', wordBreak: 'break-all' }}>{value}</td>
    </tr>
  );
}
