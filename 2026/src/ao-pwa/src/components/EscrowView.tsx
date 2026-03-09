// EscrowView — peer-to-peer atomic swap interface (CAA protocol).
// Provides: Propose Swap form, pending escrow list with status, timeout countdown.

import { useState, useEffect, useCallback } from 'react';
import { useStore, type EscrowEntry } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import {
  executeCaa, pollCaaStatus, overallCaaStatus,
  type CaaChainInput, type CaaProgressStep,
} from '../core/caaEscrow.ts';
import { signingKeyFromSeed, generateSigningKey } from '../core/sign.ts';
import { hexToBytes } from '../core/hex.ts';
import { recordingFee } from '../core/fees.ts';

// ── Propose Swap Form ────────────────────────────────────────────────

interface SwapFormState {
  // Source chain (user gives shares from here)
  srcRecorderUrl: string;
  srcChainId: string;
  srcSymbol: string;
  srcSeqId: string;
  srcAmount: string;
  // Destination chain (user receives shares here)
  dstRecorderUrl: string;
  dstChainId: string;
  dstSymbol: string;
  dstAmount: string;
  // Escrow
  escrowMinutes: string;
}

const defaultForm: SwapFormState = {
  srcRecorderUrl: '',
  srcChainId: '',
  srcSymbol: '',
  srcSeqId: '',
  srcAmount: '',
  dstRecorderUrl: '',
  dstChainId: '',
  dstSymbol: '',
  dstAmount: '',
  escrowMinutes: '5',
};

export function EscrowView() {
  const {
    recorderUrl, seedHex, publicKeyHex,
    activeEscrows, addEscrow, updateEscrow, removeEscrow,
  } = useStore();

  const [form, setForm] = useState<SwapFormState>({
    ...defaultForm,
    srcRecorderUrl: recorderUrl,
    dstRecorderUrl: recorderUrl,
  });
  const [submitting, setSubmitting] = useState(false);
  const [progress, setProgress] = useState<CaaProgressStep | null>(null);
  const [error, setError] = useState<string | null>(null);

  const setField = useCallback(
    <K extends keyof SwapFormState>(key: K, value: SwapFormState[K]) =>
      setForm(f => ({ ...f, [key]: value })),
    [],
  );

  // Auto-populate source chain from store
  useEffect(() => {
    setField('srcRecorderUrl', recorderUrl);
  }, [recorderUrl, setField]);

  const handleSubmit = useCallback(async () => {
    if (!seedHex) { setError('Unlock wallet first'); return; }

    const srcAmount = BigInt(form.srcAmount || '0');
    const dstAmount = BigInt(form.dstAmount || '0');
    if (srcAmount <= 0n || dstAmount <= 0n) { setError('Enter valid amounts'); return; }

    const escrowSecs = Math.max(60, Math.min(600, parseInt(form.escrowMinutes || '5') * 60));

    setSubmitting(true);
    setError(null);
    setProgress({ phase: 'building' });

    const escrowId = crypto.randomUUID();
    try {
      const userKey = await signingKeyFromSeed(hexToBytes(seedHex));

      // Fetch chain info for fee calculation
      const srcClient = new RecorderClient(form.srcRecorderUrl);
      const dstClient = new RecorderClient(form.dstRecorderUrl);
      const [srcInfo, dstInfo] = await Promise.all([
        srcClient.chainInfo(form.srcChainId),
        dstClient.chainInfo(form.dstChainId),
      ]);

      const srcFeeRate = { num: BigInt(srcInfo.fee_rate_num), den: BigInt(srcInfo.fee_rate_den) };
      const dstFeeRate = { num: BigInt(dstInfo.fee_rate_num), den: BigInt(dstInfo.fee_rate_den) };

      // Estimate data size for fee calculation (~200 bytes per CAA component)
      const estBytes = 200n;
      const srcFee = recordingFee(estBytes, srcFeeRate.num, srcFeeRate.den, BigInt(srcInfo.shares_out));
      const dstFee = recordingFee(estBytes, dstFeeRate.num, dstFeeRate.den, BigInt(dstInfo.shares_out));

      // Balance equation per chain: giver_amount = receiver_amount + fee
      const srcRecvAmount = srcAmount - srcFee;
      const dstRecvAmount = dstAmount - dstFee;
      if (srcRecvAmount <= 0n) { setError(`Source amount too small for fee (${srcFee})`); return; }
      if (dstRecvAmount <= 0n) { setError(`Dest amount too small for fee (${dstFee})`); return; }

      // Source chain: user is giver, counterparty receives.
      // Note: In a real workflow, the counterparty provides their pubkey.
      // This simplified flow generates keys for both sides (demo/testing only).
      const receiverKey = await generateSigningKey();
      const srcChain: CaaChainInput = {
        chainId: form.srcChainId,
        recorderUrl: form.srcRecorderUrl,
        givers: [{ seqId: BigInt(form.srcSeqId || '0'), amount: srcAmount, key: userKey }],
        receivers: [{ pubkey: receiverKey.publicKey, amount: srcRecvAmount, key: receiverKey }],
        feeRate: srcFeeRate,
      };

      // Destination chain: counterparty is giver, user receives.
      const counterKey = await generateSigningKey();
      const userReceiverKey = await generateSigningKey();
      const dstChain: CaaChainInput = {
        chainId: form.dstChainId,
        recorderUrl: form.dstRecorderUrl,
        givers: [{ seqId: 0n, amount: dstAmount, key: counterKey }],
        receivers: [{ pubkey: userReceiverKey.publicKey, amount: dstRecvAmount, key: userReceiverKey }],
        feeRate: dstFeeRate,
      };

      const deadlineUnixSecs = Math.floor(Date.now() / 1000) + escrowSecs;

      // Track in store before executing
      addEscrow({
        id: escrowId,
        caaHash: '',
        status: 'submitting',
        chains: [
          { chainId: form.srcChainId, recorderUrl: form.srcRecorderUrl, symbol: form.srcSymbol || 'SRC' },
          { chainId: form.dstChainId, recorderUrl: form.dstRecorderUrl, symbol: form.dstSymbol || 'DST' },
        ],
        createdAt: Date.now(),
        deadlineUnixSecs: deadlineUnixSecs,
      });

      const result = await executeCaa(
        [srcChain, dstChain],
        escrowSecs,
        (step) => setProgress(step),
      );

      // Update the escrow entry with the actual CAA hash
      updateEscrow(escrowId, { caaHash: result.caaHash, status: 'finalized' });

      setForm(f => ({ ...defaultForm, srcRecorderUrl: f.srcRecorderUrl, dstRecorderUrl: f.dstRecorderUrl }));
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Swap failed';
      setError(msg);
      updateEscrow(escrowId, { status: 'failed', errorMessage: msg });
    } finally {
      setSubmitting(false);
      setProgress(null);
    }
  }, [form, seedHex, addEscrow, updateEscrow]);

  return (
    <div style={{ padding: 16 }}>
      <h3 style={{ margin: '0 0 12px', fontSize: 16 }}>Atomic Swap (CAA Escrow)</h3>

      {/* Propose Swap Form */}
      <div style={{ border: '1px solid #ddd', borderRadius: 6, padding: 12, marginBottom: 16 }}>
        <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>Propose Swap</div>

        <fieldset style={{ border: '1px solid #eee', borderRadius: 4, padding: 8, marginBottom: 8 }}>
          <legend style={{ fontSize: 11, fontWeight: 500 }}>You Give (Source Chain)</legend>
          <div style={rowStyle}>
            <label style={labelStyle}>Recorder URL</label>
            <input style={inputStyle} value={form.srcRecorderUrl}
              onChange={e => setField('srcRecorderUrl', e.target.value)} />
          </div>
          <div style={rowStyle}>
            <label style={labelStyle}>Chain ID</label>
            <input style={inputStyle} value={form.srcChainId}
              onChange={e => setField('srcChainId', e.target.value)} placeholder="hex" />
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <div style={{ flex: 1, ...rowStyle }}>
              <label style={labelStyle}>Symbol</label>
              <input style={inputStyle} value={form.srcSymbol}
                onChange={e => setField('srcSymbol', e.target.value)} placeholder="BCG" />
            </div>
            <div style={{ flex: 1, ...rowStyle }}>
              <label style={labelStyle}>Seq ID</label>
              <input style={inputStyle} value={form.srcSeqId}
                onChange={e => setField('srcSeqId', e.target.value)} placeholder="0" />
            </div>
            <div style={{ flex: 1, ...rowStyle }}>
              <label style={labelStyle}>Amount</label>
              <input style={inputStyle} value={form.srcAmount}
                onChange={e => setField('srcAmount', e.target.value)} placeholder="100" />
            </div>
          </div>
        </fieldset>

        <fieldset style={{ border: '1px solid #eee', borderRadius: 4, padding: 8, marginBottom: 8 }}>
          <legend style={{ fontSize: 11, fontWeight: 500 }}>You Receive (Destination Chain)</legend>
          <div style={rowStyle}>
            <label style={labelStyle}>Recorder URL</label>
            <input style={inputStyle} value={form.dstRecorderUrl}
              onChange={e => setField('dstRecorderUrl', e.target.value)} />
          </div>
          <div style={rowStyle}>
            <label style={labelStyle}>Chain ID</label>
            <input style={inputStyle} value={form.dstChainId}
              onChange={e => setField('dstChainId', e.target.value)} placeholder="hex" />
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <div style={{ flex: 1, ...rowStyle }}>
              <label style={labelStyle}>Symbol</label>
              <input style={inputStyle} value={form.dstSymbol}
                onChange={e => setField('dstSymbol', e.target.value)} placeholder="RMF" />
            </div>
            <div style={{ flex: 1, ...rowStyle }}>
              <label style={labelStyle}>Amount</label>
              <input style={inputStyle} value={form.dstAmount}
                onChange={e => setField('dstAmount', e.target.value)} placeholder="200" />
            </div>
          </div>
        </fieldset>

        <div style={rowStyle}>
          <label style={labelStyle}>Escrow Duration (minutes)</label>
          <input style={{ ...inputStyle, width: 60 }} type="number" min={1} max={10}
            value={form.escrowMinutes}
            onChange={e => setField('escrowMinutes', e.target.value)} />
        </div>

        {error && <div style={{ color: '#c00', fontSize: 12, marginBottom: 8 }}>{error}</div>}

        {progress && (
          <div style={{ fontSize: 11, color: '#666', marginBottom: 8 }}>
            {progressText(progress)}
          </div>
        )}

        <button
          onClick={handleSubmit}
          disabled={submitting || !seedHex}
          style={{ fontSize: 12, padding: '4px 16px' }}
        >
          {submitting ? 'Executing Swap...' : 'Propose Swap'}
        </button>

        {!seedHex && (
          <div style={{ fontSize: 11, color: '#999', marginTop: 4 }}>
            Unlock your wallet to propose swaps.
          </div>
        )}
      </div>

      {/* Active Escrows */}
      <EscrowList
        escrows={activeEscrows}
        onRemove={removeEscrow}
        onUpdate={updateEscrow}
      />
    </div>
  );
}

// ── Escrow List ──────────────────────────────────────────────────────

function EscrowList({ escrows, onRemove, onUpdate }: {
  escrows: EscrowEntry[];
  onRemove: (id: string) => void;
  onUpdate: (id: string, patch: Partial<EscrowEntry>) => void;
}) {
  if (escrows.length === 0) {
    return <div style={{ fontSize: 12, color: '#999' }}>No active escrows.</div>;
  }

  return (
    <div>
      <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 8 }}>Active Escrows</div>
      {escrows.map(e => (
        <EscrowCard key={e.id} entry={e} onRemove={onRemove} onUpdate={onUpdate} />
      ))}
    </div>
  );
}

function EscrowCard({ entry, onRemove, onUpdate }: {
  entry: EscrowEntry;
  onRemove: (id: string) => void;
  onUpdate: (id: string, patch: Partial<EscrowEntry>) => void;
}) {
  const [polling, setPolling] = useState(false);
  const secondsLeft = Math.max(0, entry.deadlineUnixSecs - Math.floor(Date.now() / 1000));
  const [countdown, setCountdown] = useState(secondsLeft);

  // Countdown timer
  useEffect(() => {
    if (entry.status === 'finalized' || entry.status === 'expired' || entry.status === 'failed') return;
    const interval = setInterval(() => {
      const left = Math.max(0, entry.deadlineUnixSecs - Math.floor(Date.now() / 1000));
      setCountdown(left);
      if (left === 0 && entry.status !== 'expired') {
        onUpdate(entry.id, { status: 'expired' });
      }
    }, 1000);
    return () => clearInterval(interval);
  }, [entry.deadlineUnixSecs, entry.status, entry.id, onUpdate]);

  // Poll status
  const handleRefresh = useCallback(async () => {
    if (!entry.caaHash) return;
    setPolling(true);
    try {
      const statuses = await pollCaaStatus(entry.caaHash, entry.chains);
      const overall = overallCaaStatus(statuses);
      if (overall !== 'unknown') {
        onUpdate(entry.id, { status: overall as EscrowEntry['status'] });
      }
    } catch {
      // ignore polling errors
    } finally {
      setPolling(false);
    }
  }, [entry.caaHash, entry.chains, onUpdate]);

  const statusColor = {
    submitting: '#999',
    escrowed: '#e65100',
    binding: '#1565c0',
    finalized: '#2e7d32',
    expired: '#666',
    failed: '#c00',
  }[entry.status] ?? '#999';

  return (
    <div style={{
      border: '1px solid #ddd', borderRadius: 4, padding: 8, marginBottom: 8,
      background: entry.status === 'finalized' ? '#f0f8f0' : entry.status === 'failed' ? '#fff5f5' : '#fff',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 4 }}>
        <span style={{ fontSize: 12, fontWeight: 600, color: statusColor }}>
          {entry.status.toUpperCase()}
        </span>
        <span style={{ fontSize: 11, color: '#999' }}>
          {entry.chains.map(c => c.symbol).join(' / ')}
        </span>
      </div>

      {entry.caaHash && (
        <div style={{ fontSize: 10, fontFamily: 'monospace', color: '#666', marginBottom: 4 }}>
          CAA: {entry.caaHash.slice(0, 16)}...
        </div>
      )}

      {/* Countdown */}
      {entry.status !== 'finalized' && entry.status !== 'expired' && entry.status !== 'failed' && (
        <div style={{ fontSize: 11, marginBottom: 4 }}>
          <span style={{ color: countdown < 60 ? '#c00' : '#666' }}>
            {formatCountdown(countdown)} remaining
          </span>
        </div>
      )}

      {entry.errorMessage && (
        <div style={{ fontSize: 11, color: '#c00', marginBottom: 4 }}>{entry.errorMessage}</div>
      )}

      <div style={{ display: 'flex', gap: 8 }}>
        {entry.caaHash && entry.status !== 'finalized' && entry.status !== 'failed' && (
          <button onClick={handleRefresh} disabled={polling} style={{ fontSize: 10 }}>
            {polling ? 'Checking...' : 'Refresh Status'}
          </button>
        )}
        {(entry.status === 'finalized' || entry.status === 'expired' || entry.status === 'failed') && (
          <button onClick={() => onRemove(entry.id)} style={{ fontSize: 10 }}>
            Dismiss
          </button>
        )}
      </div>
    </div>
  );
}

// ── Helpers ──────────────────────────────────────────────────────────

function progressText(step: CaaProgressStep): string {
  switch (step.phase) {
    case 'building': return 'Building CAA...';
    case 'submitting': return `Submitting to chain ${step.chainIndex + 1}/${step.totalChains}...`;
    case 'binding': return `Binding chain ${step.chainIndex + 1}/${step.totalChains}...`;
    case 'done': return 'Swap complete!';
  }
}

function formatCountdown(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = secs % 60;
  return `${m}:${String(s).padStart(2, '0')}`;
}

const rowStyle: React.CSSProperties = { marginBottom: 4 };
const labelStyle: React.CSSProperties = { display: 'block', fontSize: 10, color: '#666', marginBottom: 1 };
const inputStyle: React.CSSProperties = { width: '100%', fontSize: 12, padding: '2px 4px', boxSizing: 'border-box' };
