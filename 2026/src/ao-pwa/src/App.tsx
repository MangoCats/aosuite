import { Header } from './components/Header.tsx';
import { ChainList } from './components/ChainList.tsx';
import { ChainDetail } from './components/ChainDetail.tsx';
import { Settings } from './components/Settings.tsx';
import { ConsumerView } from './components/ConsumerView.tsx';
import { VendorView } from './components/VendorView.tsx';
import { useStore } from './store/useStore.ts';

function App() {
  const { error, view } = useStore();

  return (
    <div style={{ fontFamily: 'system-ui, sans-serif', maxWidth: 960, margin: '0 auto' }}>
      <Header />
      {error && (
        <div style={{ padding: '8px 16px', background: '#fee', color: '#c00', fontSize: 13 }}>
          {error}
        </div>
      )}
      <div style={{ display: 'flex', gap: 0 }}>
        <div style={{ width: 280, borderRight: '1px solid #ddd', minHeight: 400 }}>
          <ChainList />
          <Settings />
        </div>
        <div style={{ flex: 1 }}>
          <ChainDetail />
          {view === 'vendor' ? <VendorView /> : <ConsumerView />}
        </div>
      </div>
    </div>
  );
}

export default App;
