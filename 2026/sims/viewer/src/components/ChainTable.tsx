import { useStore } from '../store';

export function ChainTable() {
  const { chains, selectAgent } = useStore();

  return (
    <table style={tableStyle}>
      <thead>
        <tr>
          <th style={thStyle}>Symbol</th>
          <th style={thStyle}>Chain ID</th>
          <th style={{ ...thStyle, textAlign: 'right' }}>UTXOs</th>
          <th style={thStyle}>Agents</th>
        </tr>
      </thead>
      <tbody>
        {chains.map((c) => (
          <tr key={c.chain_id}>
            <td style={tdStyle}><strong>{c.symbol}</strong></td>
            <td style={{ ...tdStyle, fontFamily: 'monospace', fontSize: 12 }}>
              {c.chain_id.slice(0, 16)}...
            </td>
            <td style={{ ...tdStyle, textAlign: 'right' }}>{c.total_utxos}</td>
            <td style={tdStyle}>
              {c.agents.map((a) => (
                <span
                  key={a}
                  onClick={() => selectAgent(a)}
                  style={{ cursor: 'pointer', color: '#228be6', marginRight: 8 }}
                >
                  {a}
                </span>
              ))}
            </td>
          </tr>
        ))}
        {chains.length === 0 && (
          <tr><td colSpan={4} style={{ ...tdStyle, color: '#999', textAlign: 'center' }}>No chains yet</td></tr>
        )}
      </tbody>
    </table>
  );
}

const tableStyle: React.CSSProperties = {
  width: '100%', borderCollapse: 'collapse', fontSize: 14,
};

const thStyle: React.CSSProperties = {
  textAlign: 'left', padding: '8px 10px', borderBottom: '2px solid #dee2e6',
  fontSize: 12, fontWeight: 600, color: '#495057', textTransform: 'uppercase',
  letterSpacing: '0.5px',
};

const tdStyle: React.CSSProperties = {
  padding: '8px 10px', borderBottom: '1px solid #f1f3f5',
};
