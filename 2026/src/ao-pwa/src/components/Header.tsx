import { useStore } from '../store/useStore.ts';

interface HeaderProps {
  onSyncClick?: () => void;
}

export function Header({ onSyncClick }: HeaderProps) {
  const { view, setView, walletLabel, connected, recorderUrl, unsyncedKeyCount } = useStore();

  return (
    <header className="app-header">
      <div className="header-left">
        <h1 style={{ margin: 0, fontSize: 18 }}>Assign Onward</h1>
        <nav className="header-nav">
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
            onClick={() => setView('cooperative')}
            style={{ fontWeight: view === 'cooperative' ? 'bold' : 'normal' }}
          >
            Co-op
          </button>
          <button
            onClick={() => setView('investor')}
            style={{ fontWeight: view === 'investor' ? 'bold' : 'normal' }}
          >
            Investor
          </button>
        </nav>
      </div>
      <div className="header-right">
        {unsyncedKeyCount > 0 && (
          <button
            onClick={onSyncClick}
            style={{
              background: '#fff3cd', border: '1px solid #ffc107', borderRadius: 12,
              padding: '2px 10px', fontSize: 12, cursor: 'pointer',
            }}
            title="Unsynced keys — tap to sync"
          >
            Sync: {unsyncedKeyCount}
          </button>
        )}
        <span style={{ color: connected ? '#090' : '#c00' }}>
          {connected ? 'Connected' : 'Disconnected'}
        </span>
        <span style={{ color: '#666', wordBreak: 'break-all' }}>{recorderUrl}</span>
        {walletLabel && (
          <span title={useStore.getState().publicKeyHex ?? undefined}>
            {walletLabel}
          </span>
        )}
      </div>
    </header>
  );
}
