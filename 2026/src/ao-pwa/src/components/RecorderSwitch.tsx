// Recorder switch flow UI — TⒶ³ deliverable 2.
// Shows pending recorder change status with drain progress indicator.

import { useStore } from '../store/useStore.ts';
import { recorderSwitchPhase, type RecorderSwitchPhase } from '../core/recorderSwitch.ts';

export function RecorderSwitch() {
  const { chainInfo } = useStore();

  if (!chainInfo) return null;

  const hasPending = !!chainInfo.pending_recorder_change;
  // activeCaaCount would come from a dedicated API or be tracked locally.
  // For now we show the phase based on pending status alone.
  const phase = recorderSwitchPhase(hasPending, 0, chainInfo.frozen);

  if (phase === 'idle') return null;

  return (
    <div style={{
      marginTop: 12, padding: 12, borderRadius: 6,
      border: '1px solid #e0c050', background: '#fffde7',
    }}>
      <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 6 }}>
        Recorder Switch
      </div>
      <PhaseIndicator phase={phase} />
      {chainInfo.pending_recorder_change && (
        <div style={{ marginTop: 8, fontSize: 13 }}>
          <div style={{ color: '#666' }}>New recorder:</div>
          <div style={{ fontFamily: 'monospace', fontSize: 12, wordBreak: 'break-all' }}>
            {chainInfo.pending_recorder_change.new_recorder_pubkey.slice(0, 16)}…
          </div>
          <div style={{ fontSize: 12, color: '#2a7', marginTop: 2 }}>
            {chainInfo.pending_recorder_change.new_recorder_url}
          </div>
        </div>
      )}
    </div>
  );
}

function PhaseIndicator({ phase }: { phase: RecorderSwitchPhase }) {
  const phases: { key: RecorderSwitchPhase; label: string }[] = [
    { key: 'pending', label: 'Pending' },
    { key: 'draining', label: 'Draining CAAs' },
    { key: 'ready', label: 'Ready' },
    { key: 'completed', label: 'Complete' },
  ];

  if (phase === 'failed') {
    return (
      <div style={{ fontSize: 13, color: '#c33' }}>
        Switch unavailable — chain is frozen
      </div>
    );
  }

  const currentIdx = phases.findIndex(p => p.key === phase);

  return (
    <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
      {phases.map((p, i) => (
        <div key={p.key} style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <div style={{
            width: 8, height: 8, borderRadius: '50%',
            background: i <= currentIdx ? '#2a7' : '#ddd',
            transition: 'background 0.3s',
          }} />
          <span style={{
            fontSize: 11,
            color: i <= currentIdx ? '#333' : '#999',
            fontWeight: i === currentIdx ? 600 : 400,
          }}>
            {p.label}
          </span>
          {i < phases.length - 1 && (
            <div style={{
              width: 16, height: 1,
              background: i < currentIdx ? '#2a7' : '#ddd',
            }} />
          )}
        </div>
      ))}
    </div>
  );
}
