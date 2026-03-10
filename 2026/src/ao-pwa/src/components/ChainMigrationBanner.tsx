// Chain migration banner — TⒶ³ deliverable 4.
// Displayed when a chain is frozen (migrated). Warns user that chain is read-only.

import { useStore } from '../store/useStore.ts';

export function ChainMigrationBanner() {
  const { chainInfo } = useStore();

  if (!chainInfo?.frozen) return null;

  return (
    <div style={{
      padding: '10px 14px', marginBottom: 12, borderRadius: 6,
      background: '#fdecea', border: '1px solid #f5c6cb',
      fontSize: 13,
    }}>
      <div style={{ fontWeight: 600, color: '#721c24', marginBottom: 4 }}>
        Chain Migrated
      </div>
      <div style={{ color: '#856404' }}>
        This chain has been frozen by a migration event. No new transactions can be recorded.
        Your keys and balances are preserved — use a new chain if you need to continue trading.
      </div>
    </div>
  );
}
