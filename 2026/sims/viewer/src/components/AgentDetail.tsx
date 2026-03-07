import { useStore } from '../store';
import type { AgentState, TransactionEvent } from '../api';

export function AgentDetail({ name }: { name: string }) {
  const { agents, transactions } = useStore();

  const agent = agents.find((a) => a.name === name);
  if (!agent) {
    return <div style={{ padding: 16, color: '#666' }}>Agent "{name}" not found. Waiting for data...</div>;
  }

  const agentTxns = transactions
    .filter((t) => t.from_agent === name || t.to_agent === name)
    .sort((a, b) => b.id - a.id);

  return (
    <div>
      <AgentHeader agent={agent} />
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 20 }}>
        <WalletPanel agent={agent} />
        <StatsPanel agent={agent} txnCount={agentTxns.length} />
      </div>
      <TransactionHistory txns={agentTxns} agentName={name} />
    </div>
  );
}

function AgentHeader({ agent }: { agent: AgentState }) {
  const roleColors: Record<string, string> = {
    vendor: '#2b8a3e', exchange: '#e67700', consumer: '#1971c2', recorder: '#862e9c',
  };
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 20 }}>
      <div style={{
        width: 48, height: 48, borderRadius: '50%',
        background: roleColors[agent.role] || '#868e96',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        color: '#fff', fontWeight: 700, fontSize: 20,
      }}>
        {agent.name[0]}
      </div>
      <div>
        <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>{agent.name}</h2>
        <span style={{ fontSize: 13, color: '#666' }}>
          {agent.role} — {agent.status}
        </span>
      </div>
    </div>
  );
}

function WalletPanel({ agent }: { agent: AgentState }) {
  return (
    <div style={panelStyle}>
      <h3 style={panelTitle}>Wallet</h3>
      {agent.chains.length === 0 ? (
        <div style={{ color: '#999', fontSize: 13 }}>No chain holdings</div>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr>
              <th style={miniTh}>Chain</th>
              <th style={{ ...miniTh, textAlign: 'right' }}>Shares</th>
              <th style={{ ...miniTh, textAlign: 'right' }}>UTXOs</th>
            </tr>
          </thead>
          <tbody>
            {agent.chains.map((c) => (
              <tr key={c.chain_id}>
                <td style={miniTd}>
                  <strong>{c.symbol}</strong>
                  <span style={{ color: '#999', fontSize: 11, marginLeft: 4 }}>
                    ({c.chain_id.slice(0, 8)}...)
                  </span>
                </td>
                <td style={{ ...miniTd, textAlign: 'right', fontFamily: 'monospace' }}>
                  {formatShares(c.shares)}
                </td>
                <td style={{ ...miniTd, textAlign: 'right' }}>{c.unspent_utxos}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}

function StatsPanel({ agent, txnCount }: { agent: AgentState; txnCount: number }) {
  const totalUtxos = agent.chains.reduce((s, c) => s + c.unspent_utxos, 0);
  return (
    <div style={panelStyle}>
      <h3 style={panelTitle}>Stats</h3>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <StatCard label="Transactions" value={agent.transactions} />
        <StatCard label="Events" value={txnCount} />
        <StatCard label="Chains" value={agent.chains.length} />
        <StatCard label="Active UTXOs" value={totalUtxos} />
      </div>
      <div style={{ marginTop: 12, fontSize: 12, color: '#666' }}>
        Last action: {agent.last_action}
      </div>
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: number }) {
  return (
    <div style={{ background: '#f8f9fa', borderRadius: 6, padding: '8px 12px' }}>
      <div style={{ fontSize: 20, fontWeight: 700 }}>{value}</div>
      <div style={{ fontSize: 11, color: '#868e96', textTransform: 'uppercase' }}>{label}</div>
    </div>
  );
}

function TransactionHistory({ txns, agentName }: { txns: TransactionEvent[]; agentName: string }) {
  return (
    <div style={panelStyle}>
      <h3 style={panelTitle}>Transaction History ({txns.length})</h3>
      {txns.length === 0 ? (
        <div style={{ color: '#999', fontSize: 13 }}>No transactions yet</div>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
          <thead>
            <tr>
              <th style={miniTh}>#</th>
              <th style={miniTh}>Time</th>
              <th style={miniTh}>Chain</th>
              <th style={miniTh}>Direction</th>
              <th style={miniTh}>Counterparty</th>
              <th style={miniTh}>Block</th>
              <th style={miniTh}>Description</th>
            </tr>
          </thead>
          <tbody>
            {txns.slice(0, 100).map((t) => {
              const isSender = t.from_agent === agentName;
              const counterparty = isSender ? t.to_agent : t.from_agent;
              return (
                <tr key={t.id}>
                  <td style={{ ...miniTd, color: '#999' }}>{t.id}</td>
                  <td style={{ ...miniTd, fontFamily: 'monospace' }}>{formatTime(t.timestamp_ms)}</td>
                  <td style={miniTd}><strong>{t.symbol}</strong></td>
                  <td style={miniTd}>
                    <span style={{ color: isSender ? '#e03131' : '#2b8a3e' }}>
                      {isSender ? 'sent' : 'received'}
                    </span>
                  </td>
                  <td style={miniTd}>{counterparty}</td>
                  <td style={{ ...miniTd, textAlign: 'right' }}>{t.block_height || '-'}</td>
                  <td style={{ ...miniTd, color: '#666' }}>{t.description}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}

function formatShares(s: string): string {
  if (s.length <= 12) return s;
  return `${s[0]}.${s.slice(1, 3)}e${s.length - 1}`;
}

function formatTime(ms: number): string {
  if (!ms) return '-';
  return new Date(ms).toLocaleTimeString();
}

const panelStyle: React.CSSProperties = {
  border: '1px solid #dee2e6', borderRadius: 8, padding: 16, background: '#fff',
};

const panelTitle: React.CSSProperties = {
  fontSize: 14, fontWeight: 600, marginBottom: 12, color: '#495057',
};

const miniTh: React.CSSProperties = {
  textAlign: 'left', padding: '4px 8px', borderBottom: '1px solid #dee2e6',
  fontSize: 11, fontWeight: 600, color: '#868e96', textTransform: 'uppercase',
};

const miniTd: React.CSSProperties = {
  padding: '4px 8px', borderBottom: '1px solid #f8f9fa',
};
