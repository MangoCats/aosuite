import type { AgentState, TransactionEvent } from '../api';
import { sharesToCoins } from '../utils';

interface Props {
  agent: AgentState;
  transactions: TransactionEvent[];
}

export function AppScreenPanel({ agent, transactions }: Props) {
  switch (agent.role) {
    case 'vendor': return <VendorScreen agent={agent} transactions={transactions} />;
    case 'exchange': return <ExchangeScreen agent={agent} />;
    case 'consumer': return <ConsumerScreen agent={agent} transactions={transactions} />;
    case 'validator': return <ValidatorScreen agent={agent} />;
    case 'attacker': return <AttackerScreen agent={agent} />;
    default: return null;
  }
}

function VendorScreen({ agent, transactions }: { agent: AgentState; transactions: TransactionEvent[] }) {
  const chain = agent.chains[0];
  if (!chain) return null;

  const recentSales = transactions
    .filter((t) => t.to_agent === agent.name && t.description.includes('redeemed'))
    .length;
  const coins = chain ? sharesToCoins(chain.shares, chain.total_shares, chain.coin_count) : '-';

  return (
    <div style={screenStyle}>
      <div style={screenHeader}>
        <span style={appLabel}>AOS</span> Vendor View
      </div>
      <div style={screenBody}>
        <div style={rowStyle}>
          <span style={labelStyle}>Chain</span>
          <strong>{chain.symbol}</strong>
        </div>
        <div style={rowStyle}>
          <span style={labelStyle}>Balance</span>
          <span style={{ fontWeight: 600, fontSize: 16 }}>{coins} coins</span>
        </div>
        <div style={rowStyle}>
          <span style={labelStyle}>UTXOs</span>
          {chain.unspent_utxos}
        </div>
        <div style={rowStyle}>
          <span style={labelStyle}>Redemptions</span>
          {recentSales}
        </div>
        <div style={{ ...rowStyle, borderBottom: 'none' }}>
          <span style={labelStyle}>Status</span>
          <span style={{ color: '#2b8a3e', fontWeight: 600 }}>Accepting Orders</span>
        </div>
      </div>
    </div>
  );
}

function ExchangeScreen({ agent }: { agent: AgentState }) {
  return (
    <div style={screenStyle}>
      <div style={screenHeader}>
        <span style={appLabel}>AOI</span> Exchange View
      </div>
      <div style={screenBody}>
        <div style={{ fontSize: 11, color: '#868e96', textTransform: 'uppercase', fontWeight: 600, marginBottom: 8 }}>
          Inventory
        </div>
        {agent.chains.map((c) => {
          const coins = sharesToCoins(c.shares, c.total_shares, c.coin_count);
          return (
            <div key={c.chain_id} style={{ ...rowStyle, display: 'flex', alignItems: 'center', gap: 8 }}>
              <strong style={{ minWidth: 40 }}>{c.symbol}</strong>
              <div style={{ flex: 1, background: '#e9ecef', borderRadius: 4, height: 16, overflow: 'hidden' }}>
                <div style={{
                  height: '100%', borderRadius: 4,
                  background: '#228be6',
                  width: `${Math.min(100, Math.max(5, c.unspent_utxos * 20))}%`,
                }} />
              </div>
              <span style={{ fontSize: 12, color: '#495057', minWidth: 60, textAlign: 'right' }}>
                {coins}
              </span>
            </div>
          );
        })}
        {agent.chains.length === 0 && (
          <div style={{ color: '#999', fontSize: 13 }}>No positions</div>
        )}
        {agent.trading_rates.length > 0 && (
          <>
            <div style={{ fontSize: 11, color: '#868e96', textTransform: 'uppercase', fontWeight: 600, marginTop: 12, marginBottom: 8 }}>
              Trading Rates
            </div>
            {agent.trading_rates.map((tr) => (
              <div key={`${tr.sell}-${tr.buy}`} style={{ ...rowStyle, display: 'flex', justifyContent: 'space-between' }}>
                <span>{tr.sell} / {tr.buy}</span>
                <strong>{tr.rate.toFixed(4)}</strong>
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}

function ConsumerScreen({ agent, transactions }: { agent: AgentState; transactions: TransactionEvent[] }) {
  const filtered = transactions
    .filter((t) => t.from_agent === agent.name || t.to_agent === agent.name);
  const lastPurchase = filtered[filtered.length - 1];

  return (
    <div style={screenStyle}>
      <div style={screenHeader}>
        <span style={appLabel}>AOE</span> Consumer View
      </div>
      <div style={screenBody}>
        <div style={{ fontSize: 11, color: '#868e96', textTransform: 'uppercase', fontWeight: 600, marginBottom: 8 }}>
          Wallet
        </div>
        {agent.chains.map((c) => {
          const coins = sharesToCoins(c.shares, c.total_shares, c.coin_count);
          return (
            <div key={c.chain_id} style={rowStyle}>
              <strong>{c.symbol}</strong>
              <span style={{ float: 'right', fontWeight: 600 }}>{coins} coins</span>
            </div>
          );
        })}
        {agent.chains.length === 0 && (
          <div style={{ ...rowStyle, color: '#999' }}>No holdings</div>
        )}
        <div style={{ marginTop: 12, fontSize: 11, color: '#868e96', textTransform: 'uppercase', fontWeight: 600, marginBottom: 4 }}>
          Last Purchase
        </div>
        <div style={{ fontSize: 13, color: '#495057' }}>
          {lastPurchase
            ? `${lastPurchase.symbol} — ${lastPurchase.description}`
            : 'None yet'}
        </div>
      </div>
    </div>
  );
}

function ValidatorScreen({ agent }: { agent: AgentState }) {
  const vs = agent.validator_status;
  if (!vs) return null;

  return (
    <div style={screenStyle}>
      <div style={screenHeader}>
        <span style={appLabel}>AOV</span> Validator View
      </div>
      <div style={screenBody}>
        <div style={rowStyle}>
          <span style={labelStyle}>Chains</span>
          <strong>{vs.monitored_chains.length}</strong>
        </div>
        <div style={rowStyle}>
          <span style={labelStyle}>Blocks Verified</span>
          <strong>{vs.total_blocks_verified}</strong>
        </div>
        <div style={{ ...rowStyle, borderBottom: 'none' }}>
          <span style={labelStyle}>Alerts</span>
          <span style={{ color: vs.alerts.length > 0 ? '#e03131' : '#2b8a3e', fontWeight: 600 }}>
            {vs.alerts.length}
          </span>
        </div>
        {vs.monitored_chains.length > 0 && (
          <>
            <div style={{ fontSize: 11, color: '#868e96', textTransform: 'uppercase', fontWeight: 600, marginTop: 12, marginBottom: 8 }}>
              Monitored Chains
            </div>
            {vs.monitored_chains.map((mc) => {
              const pct = mc.chain_height > 0 ? Math.round((mc.validated_height / mc.chain_height) * 100) : 0;
              const statusColor = mc.status === 'ok' ? '#2b8a3e' : mc.status === 'alert' ? '#e03131' : '#868e96';
              return (
                <div key={mc.chain_id} style={{ ...rowStyle, display: 'flex', alignItems: 'center', gap: 8 }}>
                  <strong style={{ minWidth: 40 }}>{mc.symbol}</strong>
                  <div style={{ flex: 1, background: '#e9ecef', borderRadius: 4, height: 16, overflow: 'hidden' }}>
                    <div style={{
                      height: '100%', borderRadius: 4,
                      background: statusColor,
                      width: `${pct}%`,
                    }} />
                  </div>
                  <span style={{ fontSize: 12, color: statusColor, minWidth: 40, textAlign: 'right' }}>
                    {pct}%
                  </span>
                </div>
              );
            })}
          </>
        )}
        {vs.alerts.length > 0 && (
          <>
            <div style={{ fontSize: 11, color: '#868e96', textTransform: 'uppercase', fontWeight: 600, marginTop: 12, marginBottom: 8 }}>
              Recent Alerts
            </div>
            {vs.alerts.slice(-5).reverse().map((a, i) => (
              <div key={i} style={{ ...rowStyle, color: '#e03131', fontSize: 12 }}>
                [{a.alert_type}] {a.message}
              </div>
            ))}
          </>
        )}
      </div>
    </div>
  );
}

function AttackerScreen({ agent }: { agent: AgentState }) {
  const atk = agent.attacker_status;
  if (!atk) return null;

  const hasUnexpected = atk.unexpected_accepts > 0;

  return (
    <div style={screenStyle}>
      <div style={{ ...screenHeader, background: '#c92a2a' }}>
        <span style={{ ...appLabel, background: '#e03131' }}>ATK</span> {atk.attack_type}
      </div>
      <div style={screenBody}>
        <div style={rowStyle}>
          <span style={labelStyle}>Attempts</span>
          <strong>{atk.attempts}</strong>
        </div>
        <div style={rowStyle}>
          <span style={labelStyle}>Rejected</span>
          <span style={{ color: '#2b8a3e', fontWeight: 600 }}>{atk.rejections}</span>
        </div>
        <div style={rowStyle}>
          <span style={labelStyle}>Unexpected</span>
          <span style={{ color: hasUnexpected ? '#e03131' : '#2b8a3e', fontWeight: 600 }}>
            {atk.unexpected_accepts}{hasUnexpected ? ' !!!' : ''}
          </span>
        </div>
        <div style={{ ...rowStyle, borderBottom: 'none' }}>
          <span style={labelStyle}>Last Result</span>
          <span style={{ fontSize: 12, color: '#495057' }}>{atk.last_result}</span>
        </div>
      </div>
    </div>
  );
}

const screenStyle: React.CSSProperties = {
  border: '1px solid #dee2e6', borderRadius: 8, overflow: 'hidden', background: '#fff',
};

const screenHeader: React.CSSProperties = {
  padding: '8px 12px', background: '#343a40', color: '#fff', fontSize: 13, fontWeight: 600,
};

const appLabel: React.CSSProperties = {
  display: 'inline-block', padding: '1px 6px', borderRadius: 3,
  background: '#228be6', fontSize: 11, marginRight: 6, fontWeight: 700,
};

const screenBody: React.CSSProperties = {
  padding: 12,
};

const rowStyle: React.CSSProperties = {
  padding: '6px 0', borderBottom: '1px solid #f1f3f5', fontSize: 13,
};

const labelStyle: React.CSSProperties = {
  display: 'inline-block', width: 100, color: '#868e96', fontSize: 12,
};
