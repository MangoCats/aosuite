import { useEffect, useRef } from 'react';
import { useStore } from './store';
import { connectWs, fetchChains, fetchScenario } from './api';
import { AgentTable } from './components/AgentTable';
import { ChainTable } from './components/ChainTable';
import { TransactionLog } from './components/TransactionLog';
import { AgentDetail } from './components/AgentDetail';
import { MapView } from './components/MapView';
import { TimeControls } from './components/TimeControls';
import { WelcomeOverlay } from './components/WelcomeOverlay';
import { TransactionToasts, useTransactionToasts } from './components/TransactionToasts';

export function App() {
  const { setAgents, addTransactions, setChains, setScenarioMeta,
    selectedAgent, tab, setTab, selectAgent, paused } = useStore();
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  // Fetch scenario metadata once
  useEffect(() => {
    fetchScenario().then(setScenarioMeta).catch(() => {});
  }, [setScenarioMeta]);

  useEffect(() => {
    const ws = connectWs((msg) => {
      if (pausedRef.current) return;
      setAgents(msg.agents);
      if (msg.transactions.length > 0) {
        addTransactions(msg.transactions);
      }
    });

    const chainPoll = setInterval(async () => {
      if (pausedRef.current) return;
      try {
        const chains = await fetchChains();
        setChains(chains);
      } catch { /* retry next interval */ }
    }, 3000);
    fetchChains().then(setChains).catch(() => {});

    return () => {
      ws.close();
      clearInterval(chainPoll);
    };
  }, [setAgents, addTransactions, setChains]);

  // Hook: generate toasts from new transactions
  useTransactionToasts();

  if (selectedAgent) {
    return (
      <div style={{ maxWidth: 1100, margin: '0 auto', padding: '16px 20px' }}>
        <Header />
        <button onClick={() => selectAgent(null)} style={backBtnStyle}>
          Back to Community View
        </button>
        <AgentDetail name={selectedAgent} />
      </div>
    );
  }

  return (
    <div style={{ maxWidth: 1100, margin: '0 auto', padding: '16px 20px' }}>
      <Header />
      <TimeControls />
      {tab === 'map' && <TransactionToasts />}
      <div style={{ display: 'flex', gap: 0, borderBottom: '2px solid #dee2e6', marginBottom: 16 }}>
        <TabBtn label="Map" active={tab === 'map'} onClick={() => setTab('map')} />
        <TabBtn label="Agents" active={tab === 'agents'} onClick={() => setTab('agents')} />
        <TabBtn label="Chains" active={tab === 'chains'} onClick={() => setTab('chains')} />
        <TabBtn label="Transactions" active={tab === 'transactions'} onClick={() => setTab('transactions')} />
      </div>
      {tab === 'map' && <MapView />}
      {tab === 'agents' && <AgentTable />}
      {tab === 'chains' && <ChainTable />}
      {tab === 'transactions' && <TransactionLog />}
      <WelcomeOverlay />
    </div>
  );
}

function Header() {
  const { agents, transactions, scenarioMeta, setShowWelcome } = useStore();
  const title = scenarioMeta?.title || scenarioMeta?.name || 'AO Sims Viewer';
  const hasOverlay = !!(scenarioMeta?.description || scenarioMeta?.title);

  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', marginBottom: 16 }}>
      <div style={{ display: 'flex', alignItems: 'baseline', gap: 8 }}>
        <h1 style={{ fontSize: 20, fontWeight: 600 }}>{title}</h1>
        {hasOverlay && (
          <button
            onClick={() => setShowWelcome(true)}
            title="About this simulation"
            style={{
              width: 22, height: 22, borderRadius: '50%', border: '1px solid #dee2e6',
              background: '#f8f9fa', cursor: 'pointer', fontSize: 13, fontWeight: 700,
              color: '#868e96', display: 'flex', alignItems: 'center', justifyContent: 'center',
              padding: 0, lineHeight: 1,
            }}
          >
            ?
          </button>
        )}
      </div>
      <span style={{ fontSize: 13, color: '#666' }}>
        {agents.length} agents, {transactions.length} transactions
      </span>
    </div>
  );
}

function TabBtn({ label, active, onClick }: { label: string; active: boolean; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      style={{
        padding: '8px 20px',
        border: 'none',
        borderBottom: active ? '2px solid #228be6' : '2px solid transparent',
        background: 'none',
        fontWeight: active ? 600 : 400,
        color: active ? '#228be6' : '#495057',
        cursor: 'pointer',
        fontSize: 14,
      }}
    >
      {label}
    </button>
  );
}

const backBtnStyle: React.CSSProperties = {
  padding: '6px 14px',
  border: '1px solid #dee2e6',
  borderRadius: 4,
  background: '#fff',
  cursor: 'pointer',
  fontSize: 13,
  marginBottom: 12,
};
