import { useState, useCallback, useEffect, useMemo } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import type { UtxoInfo } from '../api/client.ts';
import { signingKeyFromSeed, generateSigningKey } from '../core/sign.ts';
import { buildAuthorizationJson, buildAssignment } from '../core/assignment.ts';
import { recordingFee } from '../core/fees.ts';
import { toBytes } from '../core/dataitem.ts';
import { bytesToHex, hexToBytes } from '../core/hex.ts';
import type { Giver, Receiver, FeeRate } from '../core/assignment.ts';
import * as offlineQueue from '../core/offlineQueue.ts';
import { VendorMap, type VendorPin } from './VendorMap.tsx';
import { AttachmentPicker } from './AttachmentPicker.tsx';
import type { AttachedBlob } from '../core/blob.ts';

/** Scan chain for unspent UTXOs owned by the given pubkey hex. */
async function scanUtxos(
  client: RecorderClient,
  chainId: string,
  nextSeqId: number,
  pubkeyHex: string,
): Promise<UtxoInfo[]> {
  const found: UtxoInfo[] = [];
  for (let seq = 0; seq < nextSeqId; seq++) {
    try {
      const utxo = await client.getUtxo(chainId, seq);
      if (utxo.status === 'Unspent' && utxo.pubkey === pubkeyHex) {
        found.push(utxo);
      }
    } catch {
      // seq_id may not exist (genesis block uses seq 0 for shares_out tracking)
    }
  }
  return found;
}

export function ConsumerView() {
  const {
    recorderUrl, selectedChainId, chainInfo, chains,
    seedHex: storedSeedHex, publicKeyHex,
    setWallet, clearWallet,
  } = useStore();

  // Wallet management
  const [importSeed, setImportSeed] = useState('');

  // UTXO state
  const [utxos, setUtxos] = useState<UtxoInfo[]>([]);
  const [selectedUtxo, setSelectedUtxo] = useState<UtxoInfo | null>(null);
  const [scanning, setScanning] = useState(false);

  // Transfer state
  const [transferAmount, setTransferAmount] = useState('');
  const [recipientPubkey, setRecipientPubkey] = useState('');
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);
  const [queuedCount, setQueuedCount] = useState(0);
  const [attachments, setAttachments] = useState<AttachedBlob[]>([]);

  // Auto-flush offline queue when online
  useEffect(() => {
    async function checkQueue() {
      const count = await offlineQueue.pendingCount();
      setQueuedCount(count);
      if (count > 0 && navigator.onLine) {
        const submitted = await offlineQueue.flushPending();
        if (submitted > 0) {
          setStatus(`Submitted ${submitted} queued assignment${submitted > 1 ? 's' : ''}`);
        }
        setQueuedCount(await offlineQueue.pendingCount());
      }
    }
    checkQueue();
    window.addEventListener('online', checkQueue);
    const interval = setInterval(checkQueue, 30_000);
    return () => {
      window.removeEventListener('online', checkQueue);
      clearInterval(interval);
    };
  }, []);

  // ── Wallet Actions ────────────────────────────────────────────────
  async function handleGenerate() {
    const key = await generateSigningKey();
    const hex = bytesToHex(key.seed);
    const pubHex = bytesToHex(key.publicKey);
    setWallet('Wallet', pubHex, hex);
    setUtxos([]);
    setSelectedUtxo(null);
  }

  async function handleImport() {
    if (importSeed.length !== 64) return;
    if (!/^[0-9a-fA-F]{64}$/.test(importSeed)) {
      setStatus('Import error: seed must be 64 hex characters');
      return;
    }
    try {
      const key = await signingKeyFromSeed(hexToBytes(importSeed));
      setWallet('Wallet', bytesToHex(key.publicKey), importSeed);
      setImportSeed('');
      setUtxos([]);
      setSelectedUtxo(null);
    } catch (e) {
      setStatus(`Import error: ${e}`);
    }
  }

  function handleClear() {
    clearWallet();
    setUtxos([]);
    setSelectedUtxo(null);
    setStatus('');
  }

  // ── UTXO Scanning ────────────────────────────────────────────────
  const handleScan = useCallback(async () => {
    if (!selectedChainId || !chainInfo || !publicKeyHex) return;
    setScanning(true);
    try {
      const client = new RecorderClient(recorderUrl);
      const found = await scanUtxos(client, selectedChainId, chainInfo.next_seq_id, publicKeyHex);
      setUtxos(found);
      if (found.length > 0) setSelectedUtxo(found[0]);
      else setSelectedUtxo(null);
    } catch (e) {
      setStatus(`Scan error: ${e}`);
    }
    setScanning(false);
  }, [recorderUrl, selectedChainId, chainInfo, publicKeyHex]);

  // ── Transfer ──────────────────────────────────────────────────────
  async function handleTransfer() {
    if (!chainInfo || !selectedChainId || !storedSeedHex || !selectedUtxo) return;
    if (recipientPubkey && !/^[0-9a-fA-F]{64}$/.test(recipientPubkey)) {
      setStatus('Error: recipient pubkey must be 64 hex characters');
      return;
    }
    setLoading(true);
    setStatus('Building transfer...');

    try {
      const client = new RecorderClient(recorderUrl);
      const giverKey = await signingKeyFromSeed(hexToBytes(storedSeedHex));
      const giverAmt = BigInt(selectedUtxo.amount);
      const sendAmt = BigInt(transferAmount || selectedUtxo.amount);

      const feeRate: FeeRate = {
        num: BigInt(chainInfo.fee_rate_num),
        den: BigInt(chainInfo.fee_rate_den),
      };
      const sharesOut = BigInt(chainInfo.shares_out);

      // Generate receiver key (only needed when no external recipient specified)
      const recipientKey = recipientPubkey ? null : await generateSigningKey();

      // Generate change key (back to consumer)
      const changeKey = await generateSigningKey();

      const givers: Giver[] = [{
        seqId: BigInt(selectedUtxo.seq_id),
        amount: giverAmt,
        key: giverKey,
      }];

      // If sending full amount: single receiver. Otherwise: receiver + change.
      const needsChange = sendAmt < giverAmt;
      const receivers: Receiver[] = [{
        pubkey: recipientPubkey ? hexToBytes(recipientPubkey) : recipientKey!.publicKey,
        amount: sendAmt,
        key: recipientPubkey ? undefined : recipientKey!,
      }];

      if (needsChange) {
        receivers.push({
          pubkey: changeKey.publicKey,
          amount: 0n, // adjusted by fee convergence
          key: changeKey,
        });
      }

      // Iterative fee convergence (3 rounds) — last receiver absorbs fee
      for (let i = 0; i < 3; i++) {
        const assignment = buildAssignment(givers, receivers, feeRate);
        const pageBytes = BigInt(toBytes(assignment).length + 200);
        const fee = recordingFee(pageBytes, feeRate.num, feeRate.den, sharesOut);

        if (needsChange) {
          // Change receiver (last) gets remainder after fee
          receivers[receivers.length - 1].amount = giverAmt - sendAmt - fee;
        } else {
          // Single receiver absorbs fee
          receivers[0].amount = giverAmt - fee;
        }
      }

      const lastAmt = receivers[receivers.length - 1].amount;
      if (lastAmt <= 0n) {
        setStatus('Error: insufficient funds after fee deduction');
        setLoading(false);
        return;
      }

      setStatus(`Signing (sending ${receivers[0].amount} shares)...`);
      const authJson = await buildAuthorizationJson(givers, receivers, feeRate);

      // Upload attached blobs to recorder (associated content for this transfer).
      // TODO: Reference blob hashes in the assignment DataItem once buildAssignment
      // supports DATA_BLOB children. For now, blobs are uploaded alongside but not
      // linked on-chain.
      if (attachments.length > 0) {
        setStatus('Uploading attachments...');
        try {
          for (const blob of attachments) {
            await client.uploadBlob(selectedChainId, blob.payload);
          }
        } catch (blobErr) {
          setStatus(`Blob upload failed: ${blobErr}. Transfer aborted — no shares were moved.`);
          setLoading(false);
          return;
        }
      }

      setStatus('Submitting to recorder...');
      try {
        const result = await client.submit(selectedChainId, authJson);

        let msg = `Block ${result.height} recorded! Hash: ${result.hash.slice(0, 16)}...\n`;
        msg += `Sent: ${receivers[0].amount} shares\n`;
        if (!recipientPubkey && recipientKey) {
          msg += `Receiver pubkey: ${bytesToHex(recipientKey.publicKey)}\n`;
          msg += `Receiver seed: ${bytesToHex(recipientKey.seed)}\n`;
        }
        if (needsChange) {
          msg += `Change: ${receivers[receivers.length - 1].amount} shares\n`;
          msg += `Change pubkey: ${bytesToHex(changeKey.publicKey)}\n`;
          msg += `Change seed: ${bytesToHex(changeKey.seed)}`;
        }
        setStatus(msg);
      } catch (submitErr) {
        // Network failure — queue for offline retry
        if (!navigator.onLine || (submitErr instanceof TypeError)) {
          await offlineQueue.enqueue({
            chainId: selectedChainId,
            recorderUrl,
            authorization: authJson,
            queuedAt: Date.now(),
            status: 'pending',
          });
          setQueuedCount(await offlineQueue.pendingCount());
          setStatus('Offline — assignment queued for submission when connectivity returns.');
        } else {
          throw submitErr;
        }
      }

      // Clean up attachment preview URLs and clear
      for (const blob of attachments) {
        if (blob.previewUrl) URL.revokeObjectURL(blob.previewUrl);
      }
      setAttachments([]);

      // Refresh UTXOs after transfer
      setSelectedUtxo(null);
      setUtxos([]);
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
    setLoading(false);
  }

  // ── Render ────────────────────────────────────────────────────────
  if (!selectedChainId || !chainInfo) {
    return <div style={{ padding: 16, color: '#666' }}>Select a chain first.</div>;
  }

  const totalBalance = utxos.reduce((sum, u) => sum + BigInt(u.amount), 0n);

  // Build vendor pins from chain listings with profiles
  const vendorPins: VendorPin[] = useMemo(() =>
    chains
      .filter(c => c.vendor_profile?.lat != null && c.vendor_profile?.lon != null)
      .map(c => ({
        symbol: c.symbol,
        name: c.vendor_profile?.name ?? c.symbol,
        lat: c.vendor_profile!.lat!,
        lon: c.vendor_profile!.lon!,
        chainId: c.chain_id,
      })),
    [chains]
  );

  return (
    <div style={{ padding: 16 }}>
      <h3 style={{ fontSize: 15, marginBottom: 12 }}>AOE Consumer</h3>

      {/* Nearby Vendors Map */}
      {vendorPins.length > 0 && (
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>Nearby Vendors</div>
          <VendorMap vendors={vendorPins} height={220} />
        </div>
      )}

      {/* Offline Queue Indicator */}
      {queuedCount > 0 && (
        <div style={{ marginBottom: 12, padding: 8, background: '#fff3cd', borderRadius: 4, fontSize: 12 }}>
          {queuedCount} assignment{queuedCount > 1 ? 's' : ''} queued offline
          {navigator.onLine ? ' — retrying...' : ' — waiting for connectivity'}
        </div>
      )}

      {/* Wallet Section */}
      <div style={{ marginBottom: 16, padding: 12, background: '#f9f9f9', borderRadius: 4 }}>
        <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>Wallet</div>
        {storedSeedHex ? (
          <div>
            <div style={{ fontSize: 12, fontFamily: 'monospace', wordBreak: 'break-all', marginBottom: 4 }}>
              Pubkey: {publicKeyHex}
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button onClick={handleScan} disabled={scanning}>
                {scanning ? 'Scanning...' : 'Scan UTXOs'}
              </button>
              <button onClick={handleClear} style={{ color: '#c00' }}>Clear Wallet</button>
            </div>
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
            <button onClick={handleGenerate}>Generate New Wallet</button>
            <div style={{ display: 'flex', gap: 4 }}>
              <input
                value={importSeed}
                onChange={e => setImportSeed(e.target.value)}
                style={{ flex: 1, padding: '4px 6px', fontFamily: 'monospace', fontSize: 11 }}
                placeholder="Import seed hex (64 chars)..."
              />
              <button onClick={handleImport} disabled={importSeed.length !== 64}>Import</button>
            </div>
          </div>
        )}
      </div>

      {/* Balance & UTXOs */}
      {storedSeedHex && utxos.length > 0 && (
        <div style={{ marginBottom: 16 }}>
          <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>
            Balance: {totalBalance.toString()} shares ({utxos.length} UTXO{utxos.length > 1 ? 's' : ''})
          </div>
          {utxos.length > 1 && (
            <select
              value={selectedUtxo?.seq_id ?? ''}
              onChange={e => {
                const u = utxos.find(u => u.seq_id === Number(e.target.value));
                if (u) setSelectedUtxo(u);
              }}
              style={{ fontSize: 12, padding: 4, marginBottom: 8 }}
            >
              {utxos.map(u => (
                <option key={u.seq_id} value={u.seq_id}>
                  Seq #{u.seq_id} — {u.amount} shares
                </option>
              ))}
            </select>
          )}
        </div>
      )}

      {/* Transfer Form */}
      {storedSeedHex && selectedUtxo && (
        <div style={{ display: 'grid', gap: 8, maxWidth: 400 }}>
          <div style={{ fontSize: 13, fontWeight: 500 }}>
            Transfer from UTXO #{selectedUtxo.seq_id} ({selectedUtxo.amount} shares)
          </div>
          <label>
            <span style={{ fontSize: 12 }}>Amount (blank = full UTXO minus fee)</span>
            <input
              value={transferAmount}
              onChange={e => setTransferAmount(e.target.value)}
              style={{ width: '100%', padding: '4px 6px' }}
              placeholder={`max ${selectedUtxo.amount}`}
            />
          </label>
          <label>
            <span style={{ fontSize: 12 }}>Recipient pubkey (blank = generate new)</span>
            <input
              value={recipientPubkey}
              onChange={e => setRecipientPubkey(e.target.value)}
              style={{ width: '100%', padding: '4px 6px', fontFamily: 'monospace', fontSize: 11 }}
              placeholder="hex pubkey or leave blank..."
            />
          </label>
          <AttachmentPicker attachments={attachments} onAttach={setAttachments} />
          <button onClick={handleTransfer} disabled={loading}>
            {loading ? 'Processing...' : 'Transfer'}
          </button>
        </div>
      )}

      {status && (
        <pre style={{ marginTop: 12, padding: 8, background: '#f5f5f5', fontSize: 12, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
          {status}
        </pre>
      )}
    </div>
  );
}
