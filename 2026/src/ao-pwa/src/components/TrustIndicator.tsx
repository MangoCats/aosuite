import type { ValidatorEndorsement } from '../api/client.ts';

interface Props {
  validators: ValidatorEndorsement[];
  blockHeight: number;
}

export function TrustIndicator({ validators, blockHeight }: Props) {
  if (validators.length === 0) return null;

  return (
    <div style={{ marginTop: 12 }}>
      <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Validator Endorsements</h3>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        {validators.map((v) => (
          <ValidatorRow key={v.url} v={v} blockHeight={blockHeight} />
        ))}
      </div>
    </div>
  );
}

function ValidatorRow({ v, blockHeight }: { v: ValidatorEndorsement; blockHeight: number }) {
  const label = v.label || v.url;
  const lag = Math.max(0, blockHeight - v.validated_height);
  const isOk = v.status === 'ok';
  const isCurrent = isOk && lag <= 1;

  const color = isCurrent ? '#2a7' : isOk ? '#b80' : '#c33';
  const statusText = !isOk
    ? v.status
    : isCurrent
      ? 'verified'
      : `${lag} blocks behind`;

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      fontSize: 13, fontFamily: 'monospace',
    }}>
      <span style={{
        width: 8, height: 8, borderRadius: '50%',
        backgroundColor: color, flexShrink: 0,
      }} />
      <span style={{ color: '#444' }}>{label}</span>
      <span style={{ color, marginLeft: 'auto' }}>{statusText}</span>
    </div>
  );
}
