import { useStore } from '../store';
import type { AgentState } from '../api';
import { holdingToCoins } from '../utils';

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
            <th style={thStyle}>Coins</th>
            <th style={thStyle}>Last Action</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((a) => {
            const utxos = a.chains.reduce((s, c) => s + c.unspent_utxos, 0);
            const coinsDisplay = a.chains.length === 0
              ? '-'
              : a.chains.map((c) => holdingToCoins(c)).join(', ');
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
                <td style={tdStyle}>
                  {a.paused
                    ? <span style={{ color: '#e03131', fontWeight: 600 }}>paused</span>
                    : a.status}
                </td>
                <td style={{ ...tdStyle, textAlign: 'right' }}>{a.transactions}</td>
                <td style={{ ...tdStyle, textAlign: 'right' }}>{utxos}</td>
                <td style={{ ...tdStyle, textAlign: 'right', fontSize: 12 }}>{coinsDisplay}</td>
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
    validator: '#862e9c',
    attacker: '#e03131',
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
