// Recorder identity display — TⒶ³ deliverable 3.
// Shows current recorder pubkey, reward rate, and chain status badges.

import { useStore } from '../store/useStore.ts';
import { formatRewardRate } from '../core/rewardRate.ts';

export function RecorderIdentity() {
  const { chainInfo } = useStore();

  if (!chainInfo) return null;

  const recorderPubkey = chainInfo.recorder_pubkey;
  const rewardDisplay = formatRewardRate(chainInfo.reward_rate_num, chainInfo.reward_rate_den);

  return (
    <div style={{ marginTop: 12 }}>
      <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Recorder</h3>
      <table style={{ borderCollapse: 'collapse', fontSize: 13 }}>
        <tbody>
          {recorderPubkey && (
            <tr>
              <td style={{ padding: '3px 10px 3px 0', color: '#666' }}>Key</td>
              <td style={{ padding: '3px 0', fontFamily: 'monospace', fontSize: 12 }}>
                {recorderPubkey.slice(0, 16)}…
              </td>
            </tr>
          )}
          <tr>
            <td style={{ padding: '3px 10px 3px 0', color: '#666' }}>Reward Rate</td>
            <td style={{ padding: '3px 0' }}>{rewardDisplay}</td>
          </tr>
          <tr>
            <td style={{ padding: '3px 10px 3px 0', color: '#666' }}>Owner Keys</td>
            <td style={{ padding: '3px 0' }}>{chainInfo.owner_key_count}</td>
          </tr>
        </tbody>
      </table>
      <StatusBadges frozen={chainInfo.frozen} hasPendingSwitch={!!chainInfo.pending_recorder_change} />
    </div>
  );
}

function StatusBadges({ frozen, hasPendingSwitch }: { frozen: boolean; hasPendingSwitch: boolean }) {
  if (!frozen && !hasPendingSwitch) return null;

  return (
    <div style={{ display: 'flex', gap: 6, marginTop: 6 }}>
      {frozen && (
        <span style={{
          fontSize: 11, padding: '2px 8px', borderRadius: 3,
          background: '#fdd', color: '#c33', fontWeight: 600,
        }}>
          FROZEN
        </span>
      )}
      {hasPendingSwitch && (
        <span style={{
          fontSize: 11, padding: '2px 8px', borderRadius: 3,
          background: '#fff3cd', color: '#856404', fontWeight: 600,
        }}>
          RECORDER SWITCH PENDING
        </span>
      )}
    </div>
  );
}
