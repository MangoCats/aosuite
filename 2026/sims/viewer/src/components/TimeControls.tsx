import { useStore } from '../store';

export function TimeControls() {
  const { paused, setPaused, transactions, timeFilter, setTimeFilter } = useStore();

  if (transactions.length === 0) return null;

  const minTs = transactions[0]?.timestamp_ms ?? 0;
  const maxTs = transactions[transactions.length - 1]?.timestamp_ms ?? 0;
  const currentTs = timeFilter ?? maxTs;
  const elapsed = Math.round((currentTs - minTs) / 1000);

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 10,
      padding: '6px 12px', background: '#f8f9fa',
      borderRadius: 6, marginBottom: 12, fontSize: 13,
    }}>
      <button
        onClick={() => {
          if (!paused) {
            setPaused(true);
            setTimeFilter(maxTs);
          } else {
            setPaused(false);
            setTimeFilter(null);
          }
        }}
        style={{
          padding: '4px 12px', border: '1px solid #dee2e6', borderRadius: 4,
          background: paused ? '#e7f5ff' : '#fff', cursor: 'pointer', fontSize: 13,
          fontWeight: paused ? 600 : 400,
        }}
      >
        {paused ? '▶ Resume' : '⏸ Pause'}
      </button>

      {paused && (
        <>
          <input
            type="range"
            min={minTs}
            max={maxTs}
            value={currentTs}
            onChange={(e) => setTimeFilter(Number(e.target.value))}
            style={{ flex: 1, minWidth: 120 }}
          />
          <span style={{ color: '#666', whiteSpace: 'nowrap' }}>
            {elapsed}s / {Math.round((maxTs - minTs) / 1000)}s
          </span>
        </>
      )}

      {!paused && (
        <span style={{ color: '#868e96' }}>
          Live — {transactions.length} events
        </span>
      )}
    </div>
  );
}
