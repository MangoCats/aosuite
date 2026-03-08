import { useEffect, useRef } from 'react';
import { useStore } from '../store';
import type { TransactionEvent } from '../api';

const ROLE_COLORS: Record<string, string> = {
  vendor: '#2b8a3e',
  exchange: '#e67700',
  consumer: '#1971c2',
  recorder: '#862e9c',
  validator: '#862e9c',
  attacker: '#e03131',
};

/** Watches for new transactions and generates human-readable toast messages. */
export function useTransactionToasts() {
  const { transactions, addToast, toastsMuted, paused, scenarioMeta } = useStore();
  const lastSeenId = useRef(0);

  // Build blurb map once
  const blurbMap = useRef(new Map<string, string>());
  useEffect(() => {
    blurbMap.current.clear();
    if (scenarioMeta) {
      for (const a of scenarioMeta.agents) {
        if (a.blurb) blurbMap.current.set(a.name, a.blurb);
      }
    }
  }, [scenarioMeta]);

  useEffect(() => {
    if (toastsMuted || paused) return;

    const newTxns = transactions.filter((t) => t.id > lastSeenId.current);
    if (newTxns.length > 0) {
      lastSeenId.current = newTxns[newTxns.length - 1].id;
    }

    // Only toast the most recent 2 to avoid flooding
    for (const tx of newTxns.slice(-2)) {
      const text = formatToastText(tx);
      addToast(text);
    }
  }, [transactions, addToast, toastsMuted, paused]);
}

function formatToastText(tx: TransactionEvent): string {
  const desc = tx.description.toLowerCase();
  const isCaa = desc.includes('caa');

  if (desc.includes('genesis') || desc.includes('created')) {
    return `${tx.from_agent} created ${tx.symbol} chain`;
  }
  if (desc.includes('redeemed')) {
    return `${tx.from_agent} redeemed ${tx.symbol} at ${tx.to_agent}`;
  }
  if (isCaa) {
    return `${tx.from_agent} atomic swap: ${tx.symbol} via ${tx.to_agent}`;
  }
  if (desc.includes('bought') || desc.includes('purchase') || desc.includes('sold')) {
    return `${tx.from_agent} bought ${tx.symbol} from ${tx.to_agent}`;
  }
  if (desc.includes('funded') || desc.includes('initial')) {
    return `${tx.from_agent} funded ${tx.to_agent} with ${tx.symbol}`;
  }
  // Fallback: use description directly
  return `${tx.symbol}: ${tx.from_agent} → ${tx.to_agent} (${tx.description})`;
}

/** Renders toast notifications, positioned above the main content. */
export function TransactionToasts() {
  const { toasts, removeToast, toastsMuted, toggleToastsMuted, tab } = useStore();

  // Auto-dismiss after 5 seconds
  useEffect(() => {
    if (toasts.length === 0) return;
    const timers = toasts.map((t) =>
      setTimeout(() => removeToast(t.id), 5000)
    );
    return () => timers.forEach(clearTimeout);
  }, [toasts, removeToast]);

  // Only show on map tab
  if (tab !== 'map') return null;

  return (
    <>
      {/* Mute toggle */}
      <div style={{
        display: 'flex', justifyContent: 'flex-end', marginBottom: 4,
      }}>
        <button
          onClick={toggleToastsMuted}
          style={{
            padding: '2px 8px', fontSize: 11, fontWeight: 500,
            border: '1px solid #dee2e6', borderRadius: 4, cursor: 'pointer',
            background: toastsMuted ? '#f8f9fa' : '#fff',
            color: toastsMuted ? '#868e96' : '#495057',
          }}
        >
          {toastsMuted ? 'Unmute activity' : 'Mute activity'}
        </button>
      </div>

      {/* Toast stack */}
      {!toastsMuted && toasts.length > 0 && (
        <div style={{
          display: 'flex', flexDirection: 'column', gap: 4, marginBottom: 8,
        }}>
          {toasts.map((t) => (
            <div
              key={t.id}
              style={{
                padding: '6px 12px', borderRadius: 6, fontSize: 13,
                background: '#f1f3f5', border: '1px solid #dee2e6',
                color: '#495057', animation: 'fadeIn 0.3s ease-out',
              }}
            >
              {t.text}
            </div>
          ))}
        </div>
      )}
    </>
  );
}
