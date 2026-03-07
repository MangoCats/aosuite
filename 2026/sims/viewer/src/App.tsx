import { useEffect, useRef } from 'react';
import { useStore } from './store';
import { connectWs, fetchChains } from './api';
import { AgentTable } from './components/AgentTable';
import { ChainTable } from './components/ChainTable';
import { TransactionLog } from './components/TransactionLog';
import { AgentDetail } from './components/AgentDetail';
import { MapView } from './components/MapView';
import { TimeControls } from './components/TimeControls';

export function App() {
  const { setAgents, addTransactions, setChains, selectedAgent, tab, setTab, selectAgent, paused } = useStore();
  const pausedRef = useRef(paused);
  pausedRef.current = paused;

  useEffect(() => {
    const ws = connectWs((msg) => {
      if (pausedRef.current) return;
      setAgents(msg.agents);
      if (msg.transactions.length > 0) {
        addTransactions(msg.transactions);
      }
    });

    // Also fetch chains periodically (derived from agent state on server)
    const chainPoll = setInterval(async () => {
      if (pausedRef.current) return;
      try {
        const chains = await fetchChains();
        setChains(chains);
      } catch { /* retry next interval */ }
    }, 3000);
    // Initial fetch
    fetchChains().then(setChains).catch(() => {});

    return () => {
      ws.close();
      clearInterval(chainPoll);
    };
  }, [setAgents, addTransactions, setChains]);

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
      <div style={{ display: 'flex', gap: 0, borderBottom: '2px solid #dee2e6', marginBottom: 16 }}>
        <TabBtn label="Agents" active={tab === 'agents'} onClick={() => setTab('agents')} />
        <TabBtn label="Chains" active={tab === 'chains'} onClick={() => setTab('chains')} />
        <TabBtn label="Transactions" active={tab === 'transactions'} onClick={() => setTab('transactions')} />
        <TabBtn label="Map" active={tab === 'map'} onClick={() => setTab('map')} />
      </div>
      {tab === 'agents' && <AgentTable />}
      {tab === 'chains' && <ChainTable />}
      {tab === 'transactions' && <TransactionLog />}
      {tab === 'map' && <MapView />}
    </div>
  );
}

function Header() {
  const { agents, transactions } = useStore();
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline', marginBottom: 16 }}>
      <h1 style={{ fontSize: 20, fontWeight: 600 }}>AO Sims Viewer</h1>
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
