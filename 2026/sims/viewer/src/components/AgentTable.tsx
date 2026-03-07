import { useStore } from '../store';
import type { AgentState } from '../api';

export function AgentTable() {
  const { agents, agentSort, setAgentSort, agentFilter, setAgentFilter, selectAgent } = useStore();

  const filtered = agents.filter((a) => {
    if (!agentFilter) return true;
    const q = agentFilter.toLowerCase();
    return a.name.toLowerCase().includes(q) || a.role.includes(q) || a.status.includes(q);
  });

  const sorted = [...filtered].sort((a, b) => {
    const dir = agentSort.asc ? 1 : -1;
    const key = agentSort.key as keyof AgentState;
    if (key === 'transactions') return (a.transactions - b.transactions) * dir;
    const av = String(a[key] ?? '');
    const bv = String(b[key] ?? '');
    return av.localeCompare(bv) * dir;
  });

  return (
    <div>
      <input
        placeholder="Filter agents..."
        value={agentFilter}
        onChange={(e) => setAgentFilter(e.target.value)}
        style={filterStyle}
      />
      <table style={tableStyle}>
        <thead>
          <tr>
            <SortTh label="Name" field="name" sort={agentSort} onSort={setAgentSort} />
            <SortTh label="Role" field="role" sort={agentSort} onSort={setAgentSort} />
            <SortTh label="Status" field="status" sort={agentSort} onSort={setAgentSort} />
            <SortTh label="Txns" field="transactions" sort={agentSort} onSort={setAgentSort} />
            <th style={thStyle}>UTXOs</th>
            <th style={thStyle}>Shares</th>
            <th style={thStyle}>Last Action</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((a) => {
            const utxos = a.chains.reduce((s, c) => s + c.unspent_utxos, 0);
            const sharesDisplay = a.chains.length === 0
              ? '0'
              : a.chains.length === 1
                ? formatShares(a.chains[0].shares)
                : a.chains.map((c) => `${c.symbol}:${formatShares(c.shares)}`).join(' ');
            return (
              <tr
                key={a.name}
                onClick={() => selectAgent(a.name)}
                style={{ cursor: 'pointer' }}
                onMouseOver={(e) => (e.currentTarget.style.background = '#f1f3f5')}
                onMouseOut={(e) => (e.currentTarget.style.background = '')}
              >
                <td style={tdStyle}><strong>{a.name}</strong></td>
                <td style={tdStyle}><RoleBadge role={a.role} /></td>
                <td style={tdStyle}>{a.status}</td>
                <td style={{ ...tdStyle, textAlign: 'right' }}>{a.transactions}</td>
                <td style={{ ...tdStyle, textAlign: 'right' }}>{utxos}</td>
                <td style={{ ...tdStyle, textAlign: 'right', fontFamily: 'monospace', fontSize: 12 }}>{sharesDisplay}</td>
                <td style={{ ...tdStyle, fontSize: 12, color: '#666' }}>{a.last_action}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function RoleBadge({ role }: { role: string }) {
  const colors: Record<string, string> = {
    vendor: '#2b8a3e',
    exchange: '#e67700',
    consumer: '#1971c2',
    recorder: '#862e9c',
  };
  return (
    <span style={{
      display: 'inline-block',
      padding: '2px 8px',
      borderRadius: 10,
      fontSize: 11,
      fontWeight: 600,
      color: '#fff',
      background: colors[role] || '#868e96',
    }}>
      {role}
    </span>
  );
}

function SortTh({ label, field, sort, onSort }: {
  label: string; field: string;
  sort: { key: string; asc: boolean };
  onSort: (key: string) => void;
}) {
  const arrow = sort.key === field ? (sort.asc ? ' \u25B2' : ' \u25BC') : '';
  return (
    <th style={{ ...thStyle, cursor: 'pointer' }} onClick={() => onSort(field)}>
      {label}{arrow}
    </th>
  );
}

function formatShares(s: string): string {
  if (s.length <= 12) return s;
  return `${s[0]}.${s.slice(1, 3)}e${s.length - 1}`;
}

const filterStyle: React.CSSProperties = {
  width: '100%', padding: '8px 12px', border: '1px solid #dee2e6',
  borderRadius: 4, fontSize: 14, marginBottom: 12,
};

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
