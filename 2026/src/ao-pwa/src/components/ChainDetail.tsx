import { useEffect, useState } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient, type BlobPolicyResponse } from '../api/client.ts';
import { TrustIndicator } from './TrustIndicator.tsx';
import { QrCode } from './QrCode.tsx';
import {
  parseCredentialRefs, deduplicateCredentials,
  type CredentialRefInfo,
} from '../core/credentialIssue.ts';
import { RecorderIdentity } from './RecorderIdentity.tsx';
import { OwnerKeyManager } from './OwnerKeyManager.tsx';
import { RecorderSwitch } from './RecorderSwitch.tsx';
import { ChainMigrationBanner } from './ChainMigrationBanner.tsx';

export function ChainDetail() {
  const { recorderUrl, selectedChainId, chainInfo, setChainInfo } = useStore();
  const [showQr, setShowQr] = useState(false);
  const [credentials, setCredentials] = useState<CredentialRefInfo[]>([]);
  const [blobPolicy, setBlobPolicy] = useState<BlobPolicyResponse | null>(null);

  useEffect(() => {
    if (!selectedChainId) return;
    const client = new RecorderClient(recorderUrl);
    let cancelled = false;

    async function load() {
      try {
        const info = await client.chainInfo(selectedChainId!);
        if (!cancelled) setChainInfo(info);
      } catch {
        // ignore refresh errors
      }
    }

    load();
    const interval = setInterval(load, 5000);
    return () => { cancelled = true; clearInterval(interval); };
  }, [recorderUrl, selectedChainId, setChainInfo]);

  // Scan blocks for credential references
  useEffect(() => {
    if (!selectedChainId) return;
    const client = new RecorderClient(recorderUrl);
    let cancelled = false;

    async function loadCreds() {
      try {
        const blocks = await client.getBlocks(selectedChainId!);
        const refs = parseCredentialRefs(blocks);
        const deduped = deduplicateCredentials(refs);
        if (!cancelled) setCredentials(deduped);
      } catch {
        // Non-fatal
      }
    }

    loadCreds();
    return () => { cancelled = true; };
  }, [recorderUrl, selectedChainId]);

  // Fetch blob policy from genesis
  useEffect(() => {
    if (!selectedChainId) return;
    const client = new RecorderClient(recorderUrl);
    let cancelled = false;

    client.getBlobPolicy(selectedChainId).then(policy => {
      if (!cancelled) setBlobPolicy(policy);
    }).catch(() => {
      if (!cancelled) setBlobPolicy(null);
    });

    return () => { cancelled = true; };
  }, [recorderUrl, selectedChainId]);

  if (!selectedChainId) {
    return <div style={{ padding: 16, color: '#666' }}>Select a chain to view details.</div>;
  }

  if (!chainInfo) {
    return <div style={{ padding: 16 }}>Loading...</div>;
  }

  const hasValidators = chainInfo.validators && chainInfo.validators.length > 0;
  const hasCredentials = credentials.length > 0;

  return (
    <div style={{ padding: 16 }}>
      <ChainMigrationBanner />
      <h2 style={{ fontSize: 16 }}>{chainInfo.symbol}</h2>
      <table style={{ borderCollapse: 'collapse', fontSize: 14 }}>
        <tbody>
          <Row label="Chain ID" value={chainInfo.chain_id} />
          <Row label="Block Height" value={String(chainInfo.block_height)} />
          <Row label="Shares Out" value={chainInfo.shares_out} />
          <Row label="Coins" value={chainInfo.coin_count} />
          <Row label="Fee Rate" value={`${chainInfo.fee_rate_num} / ${chainInfo.fee_rate_den}`} />
          <Row label="Expiry Period" value={`${chainInfo.expiry_period}s`} />
          <Row label="Expiry Mode" value={String(chainInfo.expiry_mode)} />
          <Row label="Next Seq ID" value={String(chainInfo.next_seq_id)} />
        </tbody>
      </table>
      <button
        onClick={() => setShowQr(!showQr)}
        style={{ marginTop: 8, fontSize: 12 }}
      >
        {showQr ? 'Hide QR' : 'Show QR Code'}
      </button>
      {showQr && (
        <div style={{ marginTop: 8 }}>
          <QrCode value={`${recorderUrl}/chain/${chainInfo.chain_id}/info`} size={180} />
          <div style={{ fontSize: 11, color: '#666', marginTop: 4 }}>
            Scan to connect to this chain
          </div>
        </div>
      )}
      <BlobPolicyDisplay policy={blobPolicy} />
      <RecorderIdentity />
      <OwnerKeyManager />
      <RecorderSwitch />
      {(hasValidators || hasCredentials) && (
        <TrustIndicator
          validators={chainInfo.validators ?? []}
          blockHeight={chainInfo.block_height}
          credentials={hasCredentials ? credentials : undefined}
        />
      )}
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <tr>
      <td style={{ padding: '4px 12px 4px 0', fontWeight: 500, color: '#444' }}>{label}</td>
      <td style={{ padding: '4px 0', fontFamily: 'monospace', wordBreak: 'break-all' }}>{value}</td>
    </tr>
  );
}

function BlobPolicyDisplay({ policy }: { policy: BlobPolicyResponse | null }) {
  if (!policy) {
    return (
      <div style={{ marginTop: 12, fontSize: 13, color: '#999' }}>
        Blob storage: best-effort, no retention guarantee
      </div>
    );
  }

  return (
    <div style={{ marginTop: 12 }}>
      <h3 style={{ fontSize: 14, margin: '0 0 8px' }}>Blob Retention Policy</h3>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {policy.rules.map((rule, i) => (
          <div key={i} style={{ fontSize: 13, fontFamily: 'monospace' }}>
            <span style={{ color: '#444' }}>{rule.mime_pattern}</span>
            {rule.max_blob_size != null && (
              <span style={{ color: '#666', marginLeft: 8 }}>
                max {formatBytes(rule.max_blob_size)}
              </span>
            )}
            {rule.retention_secs != null && (
              <span style={{ color: '#2a7', marginLeft: 8 }}>
                {formatDuration(rule.retention_secs)}
              </span>
            )}
          </div>
        ))}
      </div>
      {policy.capacity_limit != null && (
        <div style={{ fontSize: 12, color: '#666', marginTop: 4 }}>
          Capacity: {formatBytes(policy.capacity_limit)}
        </div>
      )}
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes >= 1_073_741_824) return `${(bytes / 1_073_741_824).toFixed(1)} GB`;
  if (bytes >= 1_048_576) return `${(bytes / 1_048_576).toFixed(1)} MB`;
  if (bytes >= 1024) return `${(bytes / 1024).toFixed(0)} KB`;
  return `${bytes} B`;
}

function formatDuration(seconds: number): string {
  const days = seconds / 86400;
  if (days >= 365) return `${(days / 365).toFixed(1)} years`;
  if (days >= 30) return `${(days / 30).toFixed(0)} months`;
  if (days >= 1) return `${days.toFixed(0)} days`;
  return `${seconds}s`;
}
