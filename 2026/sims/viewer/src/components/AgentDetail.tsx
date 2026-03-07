import { useStore } from '../store';
import type { AgentState, TransactionEvent } from '../api';
import { pauseAgent, resumeAgent } from '../api';
import { sharesToCoins } from '../utils';
import { AppScreenPanel } from './AppScreenPanel';

export function AgentDetail({ name }: { name: string }) {
  const { agents, transactions, setAgentPaused } = useStore();

  const agent = agents.find((a) => a.name === name);
  if (!agent) {
    return <div style={{ padding: 16, color: '#666' }}>Agent "{name}" not found. Waiting for data...</div>;
  }

  const agentTxns = transactions
    .filter((t) => t.from_agent === name || t.to_agent === name)
    .sort((a, b) => b.id - a.id);

  return (
    <div>
      <AgentHeader agent={agent} onTogglePause={setAgentPaused} />
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16, marginBottom: 20 }}>
        <WalletPanel agent={agent} />
        <AppScreenPanel agent={agent} transactions={agentTxns} />
      </div>
      <KeyInventoryPanel agent={agent} />
      <TransactionHistory txns={agentTxns} agentName={name} />
    </div>
  );
}

function AgentHeader({ agent, onTogglePause }: { agent: AgentState; onTogglePause: (name: string, paused: boolean) => void }) {
  const roleColors: Record<string, string> = {
    vendor: '#2b8a3e', exchange: '#e67700', consumer: '#1971c2', recorder: '#862e9c',
    validator: '#862e9c', attacker: '#e03131',
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
      <div style={{ flex: 1 }}>
        <h2 style={{ fontSize: 18, fontWeight: 600, margin: 0 }}>{agent.name}</h2>
        <span style={{ fontSize: 13, color: '#666' }}>
          {agent.role} — {agent.paused ? 'paused' : agent.status}
        </span>
      </div>
      {agent.role !== 'recorder' && (
        <button
          onClick={() => {
            const next = !agent.paused;
            onTogglePause(agent.name, next);
            (next ? pauseAgent(agent.name) : resumeAgent(agent.name));
          }}
          style={{
            padding: '6px 14px', fontSize: 13, fontWeight: 600,
            border: '1px solid #dee2e6', borderRadius: 6, cursor: 'pointer',
            background: agent.paused ? '#2b8a3e' : '#e03131',
            color: '#fff',
          }}
        >
          {agent.paused ? 'Resume' : 'Pause'}
        </button>
      )}
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
              <th style={{ ...miniTh, textAlign: 'right' }}>Coins</th>
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
                <td style={{ ...miniTd, textAlign: 'right', fontWeight: 600 }}>
                  {sharesToCoins(c.shares, c.total_shares, c.coin_count)}
                </td>
                <td style={{ ...miniTd, textAlign: 'right', fontFamily: 'monospace', fontSize: 12, color: '#666' }}>
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

function KeyInventoryPanel({ agent }: { agent: AgentState }) {
  if (agent.key_summary.length === 0) return null;

  const now = Date.now();
  const STALE_MS = 5 * 60 * 1000; // 5 minutes — keys older than this get a warning

  return (
    <div style={{ ...panelStyle, marginBottom: 20 }}>
      <h3 style={panelTitle}>Key Inventory</h3>
      <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: 13 }}>
        <thead>
          <tr>
            <th style={miniTh}>Chain</th>
            <th style={{ ...miniTh, textAlign: 'right' }}>Total</th>
            <th style={{ ...miniTh, textAlign: 'right' }}>Unspent</th>
            <th style={{ ...miniTh, textAlign: 'right' }}>Spent</th>
            <th style={{ ...miniTh, textAlign: 'right' }}>Unspent Amount</th>
            <th style={miniTh}>Age</th>
          </tr>
        </thead>
        <tbody>
          {agent.key_summary.map((ks) => {
            const chainHolding = agent.chains.find((c) => c.chain_id === ks.chain_id);
            const symbol = chainHolding?.symbol || ks.chain_id.slice(0, 8);
            const coinAmount = chainHolding
              ? sharesToCoins(ks.total_unspent_amount, chainHolding.total_shares, chainHolding.coin_count)
              : ks.total_unspent_amount;
            const isStale = ks.oldest_unspent_ms != null && (now - ks.oldest_unspent_ms) > STALE_MS;

            return (
              <tr key={ks.chain_id}>
                <td style={miniTd}><strong>{symbol}</strong></td>
                <td style={{ ...miniTd, textAlign: 'right' }}>{ks.total_keys}</td>
                <td style={{ ...miniTd, textAlign: 'right', fontWeight: 600 }}>{ks.unspent_keys}</td>
                <td style={{ ...miniTd, textAlign: 'right', color: '#868e96' }}>{ks.spent_keys}</td>
                <td style={{ ...miniTd, textAlign: 'right' }}>{coinAmount}</td>
                <td style={miniTd}>
                  {ks.oldest_unspent_ms != null ? (
                    <span style={{ color: isStale ? '#e03131' : '#868e96', fontWeight: isStale ? 600 : 400 }}>
                      {formatAge(now - ks.oldest_unspent_ms)}
                      {isStale ? ' ⚠' : ''}
                    </span>
                  ) : '-'}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function formatAge(ms: number): string {
  if (ms < 1000) return '<1s';
  const secs = Math.floor(ms / 1000);
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ${secs % 60}s`;
  return `${Math.floor(mins / 60)}h ${mins % 60}m`;
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
