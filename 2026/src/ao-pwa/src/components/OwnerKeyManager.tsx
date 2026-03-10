// Owner key management UI — displays valid keys, rotation, revocation.
// TⒶ³ deliverable 1: key lifecycle with rate limit visibility.

import { useState, useEffect } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient, type OwnerKeyInfo } from '../api/client.ts';
import { keyStatus, rotationCooldown, revocationCooldown } from '../core/ownerKeys.ts';

export function OwnerKeyManager() {
  const { recorderUrl, selectedChainId, chainInfo } = useStore();
  const [ownerKeys, setOwnerKeys] = useState<OwnerKeyInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nowSecs = Math.floor(Date.now() / 1000);

  useEffect(() => {
    if (!selectedChainId) return;
    const client = new RecorderClient(recorderUrl);
    let cancelled = false;
    setLoading(true);

    client.getOwnerKeys(selectedChainId).then(keys => {
      if (!cancelled) { setOwnerKeys(keys); setLoading(false); }
    }).catch(err => {
      if (!cancelled) { setError(String(err)); setLoading(false); }
    });

    return () => { cancelled = true; };
  }, [recorderUrl, selectedChainId]);

  if (!chainInfo || !selectedChainId) return null;

  const validKeys = ownerKeys.filter(k => {
    const s = keyStatus(k, nowSecs);
    return s === 'valid' || s === 'expiring_soon';
  });
  const heldKeys = ownerKeys.filter(k => keyStatus(k, nowSecs) === 'held');
  const inactiveKeys = ownerKeys.filter(k => {
    const s = keyStatus(k, nowSecs);
    return s === 'expired' || s === 'revoked';
  });

  // Rate limit info
  const lastRotationTs = ownerKeys.length > 0
    ? Math.max(...ownerKeys.map(k => k.added_timestamp))
    : null;
  const rotCooldown = rotationCooldown(lastRotationTs, chainInfo.key_rotation_rate, nowSecs);

  return (
    <div style={{ marginTop: 16 }}>
      <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Owner Keys</h3>
      {loading && <div style={{ fontSize: 13, color: '#666' }}>Loading keys...</div>}
      {error && <div style={{ fontSize: 13, color: '#c33' }}>{error}</div>}

      {validKeys.length > 0 && (
        <div style={{ marginBottom: 8 }}>
          <div style={{ fontSize: 12, color: '#666', marginBottom: 4 }}>
            Active ({validKeys.length})
          </div>
          {validKeys.map(k => (
            <KeyRow key={k.pubkey} keyInfo={k} nowSecs={nowSecs} />
          ))}
        </div>
      )}

      {heldKeys.length > 0 && (
        <div style={{ marginBottom: 8, padding: '6px 8px', background: '#eef', borderRadius: 4 }}>
          <div style={{ fontSize: 12, color: '#66c', fontWeight: 600, marginBottom: 4 }}>
            Held — under review ({heldKeys.length})
          </div>
          {heldKeys.map(k => (
            <KeyRow key={k.pubkey} keyInfo={k} nowSecs={nowSecs} />
          ))}
        </div>
      )}

      {inactiveKeys.length > 0 && (
        <div style={{ marginBottom: 8 }}>
          <div style={{ fontSize: 12, color: '#999', marginBottom: 4 }}>
            Inactive ({inactiveKeys.length})
          </div>
          {inactiveKeys.map(k => (
            <KeyRow key={k.pubkey} keyInfo={k} nowSecs={nowSecs} />
          ))}
        </div>
      )}

      {rotCooldown > 0 && (
        <div style={{ fontSize: 12, color: '#996' }}>
          Next rotation available in {formatCooldown(rotCooldown)}
        </div>
      )}
    </div>
  );
}

function KeyRow({ keyInfo, nowSecs }: { keyInfo: OwnerKeyInfo; nowSecs: number }) {
  const status = keyStatus(keyInfo, nowSecs);
  const truncatedKey = keyInfo.pubkey.slice(0, 12) + '…';
  const statusColors: Record<string, string> = {
    valid: '#22aa77',
    expiring_soon: '#cc9900',
    expired: '#999999',
    revoked: '#cc3333',
    held: '#6666cc',
  };

  return (
    <div style={{
      display: 'flex', alignItems: 'center', gap: 8,
      padding: '4px 0', fontSize: 13,
    }}>
      <span style={{
        fontFamily: 'monospace',
        color: status === 'valid' || status === 'expiring_soon' ? '#333' : '#999',
      }}>
        {truncatedKey}
      </span>
      <span style={{
        fontSize: 11, padding: '1px 6px', borderRadius: 3,
        background: statusColors[status] + '20',
        color: statusColors[status],
      }}>
        {status.replace('_', ' ')}
      </span>
      {keyInfo.expires_at != null && status !== 'expired' && status !== 'revoked' && (
        <span style={{ fontSize: 11, color: '#666' }}>
          expires {formatExpiry(keyInfo.expires_at, nowSecs)}
        </span>
      )}
    </div>
  );
}

function formatExpiry(expiresAtAo: number, nowSecs: number): string {
  const expSecs = expiresAtAo / 189_000_000;
  const diff = expSecs - nowSecs;
  if (diff <= 0) return 'now';
  return 'in ' + formatCooldown(diff);
}

function formatCooldown(seconds: number): string {
  if (seconds >= 86400) return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h`;
  if (seconds >= 3600) return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  if (seconds >= 60) return `${Math.floor(seconds / 60)}m`;
  return `${Math.floor(seconds)}s`;
}
