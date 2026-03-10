import { useStore } from '../store';
import type { TransactionEvent } from '../api';
import { isErrorEvent } from './TransactionToasts';

export function TransactionLog() {
  const { transactions, txSort, setTxSort, txFilter, setTxFilter, selectAgent, timeFilter } = useStore();

  const filtered = transactions.filter((t) => {
    if (timeFilter !== null && t.timestamp_ms > timeFilter) return false;
    if (!txFilter) return true;
    const q = txFilter.toLowerCase();
    return (
      t.from_agent.toLowerCase().includes(q) ||
      t.to_agent.toLowerCase().includes(q) ||
      t.symbol.toLowerCase().includes(q) ||
      t.description.toLowerCase().includes(q)
    );
  });

  const sorted = [...filtered].sort((a, b) => {
    const dir = txSort.asc ? 1 : -1;
    const key = txSort.key as keyof TransactionEvent;
    if (key === 'id' || key === 'timestamp_ms' || key === 'block_height') {
      return ((a[key] as number) - (b[key] as number)) * dir;
    }
    return String(a[key]).localeCompare(String(b[key])) * dir;
  });

  return (
    <div>
      <input
        placeholder="Filter transactions..."
        value={txFilter}
        onChange={(e) => setTxFilter(e.target.value)}
        style={filterStyle}
      />
      <table style={tableStyle}>
        <thead>
          <tr>
            <SortTh label="#" field="id" sort={txSort} onSort={setTxSort} />
            <SortTh label="Time" field="timestamp_ms" sort={txSort} onSort={setTxSort} />
            <SortTh label="Chain" field="symbol" sort={txSort} onSort={setTxSort} />
            <SortTh label="From" field="from_agent" sort={txSort} onSort={setTxSort} />
            <SortTh label="To" field="to_agent" sort={txSort} onSort={setTxSort} />
            <SortTh label="Block" field="block_height" sort={txSort} onSort={setTxSort} />
            <th style={thStyle}>Description</th>
          </tr>
        </thead>
        <tbody>
          {sorted.map((t) => {
            const isErr = isErrorEvent(t);
            const rowStyle = isErr ? { background: '#fff5f5' } : {};
            const descColor = isErr ? '#c92a2a' : '#666';
            return (
              <tr key={t.id} style={rowStyle}>
                <td style={{ ...tdStyle, color: '#999', fontSize: 12 }}>{t.id}</td>
                <td style={{ ...tdStyle, fontSize: 12, fontFamily: 'monospace' }}>{formatTime(t.timestamp_ms)}</td>
                <td style={tdStyle}><strong>{t.symbol}</strong></td>
                <td style={tdStyle}>
                  <AgentLink name={t.from_agent} onClick={() => selectAgent(t.from_agent)} />
                </td>
                <td style={tdStyle}>
                  <AgentLink name={t.to_agent} onClick={() => selectAgent(t.to_agent)} />
                </td>
                <td style={{ ...tdStyle, textAlign: 'right' }}>{t.block_height || '-'}</td>
                <td style={{ ...tdStyle, fontSize: 12, color: descColor }}>{t.description}</td>
              </tr>
            );
          })}
          {sorted.length === 0 && (
            <tr><td colSpan={7} style={{ ...tdStyle, color: '#999', textAlign: 'center' }}>No transactions yet</td></tr>
          )}
        </tbody>
      </table>
    </div>
  );
}

function AgentLink({ name, onClick }: { name: string; onClick: () => void }) {
  return (
    <span onClick={onClick} style={{ cursor: 'pointer', color: '#228be6' }}>
      {name}
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

function formatTime(ms: number): string {
  if (!ms) return '-';
  const d = new Date(ms);
  return d.toLocaleTimeString();
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
