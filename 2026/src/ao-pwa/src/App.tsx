import { Header } from './components/Header.tsx';
import { ChainList } from './components/ChainList.tsx';
import { ChainDetail } from './components/ChainDetail.tsx';
import { Settings } from './components/Settings.tsx';
import { ConsumerView } from './components/ConsumerView.tsx';
import { VendorView } from './components/VendorView.tsx';
import { InvestorView } from './components/InvestorView.tsx';
import { useStore } from './store/useStore.ts';

function ViewContent() {
  const { view } = useStore();
  if (view === 'investor') return <InvestorView />;
  if (view === 'vendor') return <VendorView />;
  return <ConsumerView />;
}

function App() {
  const { error, view } = useStore();

  const isInvestor = view === 'investor';

  return (
    <div style={{ fontFamily: 'system-ui, sans-serif', maxWidth: 960, margin: '0 auto' }}>
      <Header />
      {error && (
        <div style={{ padding: '8px 16px', background: '#fee', color: '#c00', fontSize: 13 }}>
          {error}
        </div>
      )}
      {isInvestor ? (
        <ViewContent />
      ) : (
        <div style={{ display: 'flex', gap: 0 }}>
          <div style={{ width: 280, borderRight: '1px solid #ddd', minHeight: 400 }}>
            <ChainList />
            <Settings />
          </div>
          <div style={{ flex: 1 }}>
            <ChainDetail />
            <ViewContent />
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
