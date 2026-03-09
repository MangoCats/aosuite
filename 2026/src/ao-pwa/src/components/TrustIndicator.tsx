import { useState, useEffect } from 'react';
import type { ValidatorEndorsement } from '../api/client.ts';
import {
  verifyCredential,
  type CredentialRefInfo,
  type CredentialVerifyResult,
} from '../core/credentialIssue.ts';

interface Props {
  validators: ValidatorEndorsement[];
  blockHeight: number;
  credentials?: CredentialRefInfo[];
}

export function TrustIndicator({ validators, blockHeight, credentials }: Props) {
  const hasValidators = validators.length > 0;
  const hasCredentials = (credentials?.length ?? 0) > 0;

  if (!hasValidators && !hasCredentials) return null;

  return (
    <div style={{ marginTop: 12 }}>
      {hasValidators && (
        <>
          <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Validator Endorsements</h3>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {validators.map((v) => (
              <ValidatorRow key={v.url} v={v} blockHeight={blockHeight} />
            ))}
          </div>
        </>
      )}
      {hasCredentials && (
        <div style={{ marginTop: hasValidators ? 12 : 0 }}>
          <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Credentials</h3>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {credentials!.map((c) => (
              <CredentialRow key={c.url} cred={c} />
            ))}
          </div>
        </div>
      )}
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

function CredentialRow({ cred }: { cred: CredentialRefInfo }) {
  const [result, setResult] = useState<CredentialVerifyResult | 'checking'>('checking');
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    verifyCredential(cred).then(r => {
      if (!cancelled) setResult(r);
    });
    return () => { cancelled = true; };
  }, [cred.url, cred.contentHash]);

  const color = result === 'verified' ? '#2a7'
    : result === 'mismatch' ? '#c33'
    : result === 'unreachable' ? '#999'
    : '#b80';

  const statusText = result === 'verified' ? 'verified'
    : result === 'mismatch' ? 'hash mismatch'
    : result === 'unreachable' ? 'unreachable'
    : 'checking...';

  // Show a shortened URL label
  let label: string;
  try {
    const u = new URL(cred.url);
    label = u.hostname + u.pathname;
    if (label.length > 40) label = label.slice(0, 37) + '...';
  } catch {
    label = cred.url.length > 40 ? cred.url.slice(0, 37) + '...' : cred.url;
  }

  return (
    <div>
      <div
        onClick={() => setExpanded(!expanded)}
        style={{
          display: 'flex', alignItems: 'center', gap: 8,
          fontSize: 13, fontFamily: 'monospace', cursor: 'pointer',
        }}
      >
        <span style={{
          width: 8, height: 8, borderRadius: '50%',
          backgroundColor: color, flexShrink: 0,
        }} />
        <span style={{ color: '#444' }}>{label}</span>
        <span style={{ color, marginLeft: 'auto' }}>{statusText}</span>
      </div>
      {expanded && (
        <div style={{
          marginLeft: 16, marginTop: 4, padding: 8,
          background: '#f9f9f9', borderRadius: 4, fontSize: 11,
          fontFamily: 'monospace', wordBreak: 'break-all',
        }}>
          <div>URL: {cred.url}</div>
          <div>Hash: {cred.contentHash}</div>
          {cred.blockHeight != null && <div>Block: {cred.blockHeight}</div>}
        </div>
      )}
    </div>
  );
}
