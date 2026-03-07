import { useStore } from '../store/useStore.ts';

export function Header() {
  const { view, setView, walletLabel, connected, recorderUrl } = useStore();

  return (
    <header style={{ padding: '12px 16px', borderBottom: '1px solid #ddd', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
      <div style={{ display: 'flex', gap: 16, alignItems: 'center' }}>
        <h1 style={{ margin: 0, fontSize: 18 }}>Assign Onward</h1>
        <nav style={{ display: 'flex', gap: 8 }}>
          <button
            onClick={() => setView('consumer')}
            style={{ fontWeight: view === 'consumer' ? 'bold' : 'normal' }}
          >
            Consumer
          </button>
          <button
            onClick={() => setView('vendor')}
            style={{ fontWeight: view === 'vendor' ? 'bold' : 'normal' }}
          >
            Vendor
          </button>
          <button
            onClick={() => setView('investor')}
            style={{ fontWeight: view === 'investor' ? 'bold' : 'normal' }}
          >
            Investor
          </button>
        </nav>
      </div>
      <div style={{ display: 'flex', gap: 12, alignItems: 'center', fontSize: 13 }}>
        <span style={{ color: connected ? '#090' : '#c00' }}>
          {connected ? 'Connected' : 'Disconnected'}
        </span>
        <span style={{ color: '#666' }}>{recorderUrl}</span>
        {walletLabel && <span>{walletLabel}</span>}
      </div>
    </header>
  );
}
