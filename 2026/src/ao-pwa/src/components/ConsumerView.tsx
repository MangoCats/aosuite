import { useState, useCallback, useEffect, useMemo } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import type { UtxoInfo } from '../api/client.ts';
import { signingKeyFromSeed, generateSigningKey } from '../core/sign.ts';
import type { SigningKey } from '../core/sign.ts';
import { buildAuthorizationJson, buildAssignment } from '../core/assignment.ts';
import { recordingFee } from '../core/fees.ts';
import { toBytes, bytesItem } from '../core/dataitem.ts';
import type { DataItem } from '../core/dataitem.ts';
import { DATA_BLOB } from '../core/typecodes.ts';
import { bytesToHex, hexToBytes } from '../core/hex.ts';
import type { Giver, Receiver, FeeRate } from '../core/assignment.ts';
import * as offlineQueue from '../core/offlineQueue.ts';
import { VendorMap, type VendorPin } from './VendorMap.tsx';
import { AttachmentPicker } from './AttachmentPicker.tsx';
import type { AttachedBlob } from '../core/blob.ts';
import { TransactionHistory } from './TransactionHistory.tsx';
import * as walletDb from '../core/walletDb.ts';
import { validateKeysOnChain } from '../core/walletSync.ts';

/** State held between build (preview) and confirm (submit) phases. */
interface PendingTransfer {
  givers: Giver[];
  receivers: Receiver[];
  feeRate: FeeRate;
  recipientKey: SigningKey | null;
  changeKey: SigningKey;
  needsChange: boolean;
  sendAmount: bigint;
  fee: bigint;
  changeAmount: bigint;
}

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
    seedHex: storedSeedHex, publicKeyHex, walletPassphrase,
    setWallet, clearWallet,
    setUnsyncedKeyCount, setWalletKeyCount,
    cachedBalance, lastValidatedAt: lastValidatedAtMap,
    setCachedBalance, setLastValidatedAt,
  } = useStore();

  const lastValidatedAt = selectedChainId ? lastValidatedAtMap[selectedChainId] ?? null : null;

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
  const [pendingTransfer, setPendingTransfer] = useState<PendingTransfer | null>(null);

  // Validation alerts
  const [validationAlert, setValidationAlert] = useState('');

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

  // Load cached balance from IndexedDB immediately (N14)
  useEffect(() => {
    if (!selectedChainId) return;
    walletDb.chainBalance(selectedChainId).then(bal => {
      setCachedBalance(bal.toString());
    });
  }, [selectedChainId, setCachedBalance]);

  // Validate held keys against recorder when chain is selected (WalletSync §2)
  // Also subscribe to SSE block events for real-time monitoring.
  useEffect(() => {
    if (!selectedChainId || !recorderUrl) return;
    const client = new RecorderClient(recorderUrl);

    function runValidation() {
      validateKeysOnChain(client, selectedChainId!).then(async result => {
        if (result.unknownSpends.length > 0) {
          const keys = result.unknownSpends.map(k =>
            `seq #${k.seqId} (${k.amount ?? '?'} shares)`
          ).join(', ');
          setValidationAlert(
            `Warning: ${result.unknownSpends.length} key(s) spent by unknown device: ${keys}. ` +
            'If you did not authorize this, your key may be compromised. ' +
            'Consider transferring remaining coins to fresh keys.'
          );
        } else {
          setValidationAlert('');
        }
        if (result.newlySpent > 0) {
          setStatus(prev => prev
            ? `${prev}\nValidation: ${result.newlySpent} key(s) found spent since last check.`
            : `Validation: ${result.newlySpent} key(s) found spent since last check.`
          );
        }
        // Update cached balance and validation timestamp (N14)
        const bal = await walletDb.chainBalance(selectedChainId!);
        setCachedBalance(bal.toString());
        setLastValidatedAt(selectedChainId!, Date.now());
      }).catch(() => {
        // Validation failure is non-fatal — balance may be stale
      });
    }

    // Initial validation
    runValidation();

    // Subscribe to SSE block events — revalidate when new blocks arrive
    let es: EventSource | null = null;
    try {
      es = client.subscribeBlocks(selectedChainId, (_blockInfo) => {
        runValidation();
      });
    } catch {
      // SSE unavailable — fall back to initial validation only
    }

    return () => {
      if (es) es.close();
    };
  }, [selectedChainId, recorderUrl]);

  // Helper: refresh wallet key counts in global store
  async function refreshKeyCounts() {
    const all = await walletDb.getKeys();
    setWalletKeyCount(all.length);
    const unsynced = await walletDb.getUnsyncedKeys();
    setUnsyncedKeyCount(unsynced.length);
  }

  // ── Wallet Actions ────────────────────────────────────────────────
  async function handleGenerate() {
    const key = await generateSigningKey();
    const hex = bytesToHex(key.seed);
    const pubHex = bytesToHex(key.publicKey);
    setWallet('Wallet', pubHex, hex);

    // Store in IndexedDB — encrypt seed if passphrase is set
    const deviceId = await walletDb.getDeviceId();
    const storedSeed = walletPassphrase
      ? await walletDb.encryptSeedHex(hex, walletPassphrase)
      : hex;
    await walletDb.importKeyIfNew({
      chainId: selectedChainId ?? '',
      publicKey: pubHex,
      seedHex: storedSeed,
      seedEncrypted: !!walletPassphrase,
      seqId: null,
      amount: null,
      status: 'unconfirmed',
      acquiredAt: new Date().toISOString(),
      acquiredBy: deviceId,
      synced: false,
    });
    await refreshKeyCounts();

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
      const pubHex = bytesToHex(key.publicKey);
      setWallet('Wallet', pubHex, importSeed);

      // Store in IndexedDB — encrypt seed if passphrase is set
      const deviceId = await walletDb.getDeviceId();
      const storedSeed = walletPassphrase
        ? await walletDb.encryptSeedHex(importSeed, walletPassphrase)
        : importSeed;
      await walletDb.importKeyIfNew({
        chainId: selectedChainId ?? '',
        publicKey: pubHex,
        seedHex: storedSeed,
        seedEncrypted: !!walletPassphrase,
        seqId: null,
        amount: null,
        status: 'unconfirmed',
        acquiredAt: new Date().toISOString(),
        acquiredBy: deviceId,
        synced: false,
      });
      await refreshKeyCounts();

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
    setValidationAlert('');
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

  // ── Transfer: Build (preview) ───────────────────────────────────
  async function handleBuild() {
    if (!chainInfo || !selectedChainId || !storedSeedHex || !selectedUtxo) return;
    if (recipientPubkey && !/^[0-9a-fA-F]{64}$/.test(recipientPubkey)) {
      setStatus('Error: recipient pubkey must be 64 hex characters');
      return;
    }
    setLoading(true);
    setStatus('Building transfer...');

    try {
      const giverKey = await signingKeyFromSeed(hexToBytes(storedSeedHex));
      const giverAmt = BigInt(selectedUtxo.amount);
      const sendAmt = BigInt(transferAmount || selectedUtxo.amount);

      const feeRate: FeeRate = {
        num: BigInt(chainInfo.fee_rate_num),
        den: BigInt(chainInfo.fee_rate_den),
      };
      const sharesOut = BigInt(chainInfo.shares_out);

      const recipientKey = recipientPubkey ? null : await generateSigningKey();
      const changeKey = await generateSigningKey();

      const givers: Giver[] = [{
        seqId: BigInt(selectedUtxo.seq_id),
        amount: giverAmt,
        key: giverKey,
      }];

      const needsChange = sendAmt < giverAmt;
      const receivers: Receiver[] = [{
        pubkey: recipientPubkey ? hexToBytes(recipientPubkey) : recipientKey!.publicKey,
        amount: sendAmt,
        key: recipientPubkey ? null : recipientKey!,
      }];

      if (needsChange) {
        receivers.push({
          pubkey: changeKey.publicKey,
          amount: 0n,
          key: changeKey,
        });
      }

      // Build DATA_BLOB DataItems from attachments for on-chain linking.
      // These are included in the assignment for pre-substitution fee calculation,
      // then replaced with SHA256 hashes before signing.
      const blobItems: DataItem[] = attachments.map(a =>
        bytesItem(DATA_BLOB, a.payload),
      );
      const separableItems = blobItems.length > 0 ? blobItems : undefined;

      // Iterative fee convergence (3 rounds).
      // Fee is computed on pre-substitution size (includes full blob payloads).
      let fee = 0n;
      for (let i = 0; i < 3; i++) {
        const assignment = buildAssignment(givers, receivers, feeRate, separableItems);
        const pageBytes = BigInt(toBytes(assignment).length + 200);
        fee = recordingFee(pageBytes, feeRate.num, feeRate.den, sharesOut);

        if (needsChange) {
          receivers[receivers.length - 1].amount = giverAmt - sendAmt - fee;
        } else {
          receivers[0].amount = giverAmt - fee;
        }
      }

      const lastAmt = receivers[receivers.length - 1].amount;
      if (lastAmt <= 0n) {
        setStatus('Error: insufficient funds after fee deduction');
        setLoading(false);
        return;
      }

      const changeAmount = needsChange ? receivers[receivers.length - 1].amount : 0n;

      setPendingTransfer({
        givers, receivers, feeRate, recipientKey, changeKey,
        needsChange, sendAmount: receivers[0].amount, fee, changeAmount,
      });
      setStatus('');
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
    setLoading(false);
  }

  // ── Transfer: Confirm (sign + submit) ─────────────────────────────
  async function handleConfirm() {
    if (!pendingTransfer || !selectedChainId) return;
    const { givers, receivers, feeRate, recipientKey, changeKey, needsChange } = pendingTransfer;

    setLoading(true);
    setStatus('Signing...');

    try {
      const client = new RecorderClient(recorderUrl);

      // Build DATA_BLOB items for on-chain linking (same as in handleBuild).
      const blobItems: DataItem[] = attachments.map(a =>
        bytesItem(DATA_BLOB, a.payload),
      );
      const separableItems = blobItems.length > 0 ? blobItems : undefined;

      const authJson = await buildAuthorizationJson(givers, receivers, feeRate, separableItems);

      // Upload blobs to recorder before submitting the assignment.
      // Blobs must be present on the recorder so it can validate pre-sub fees.
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

        // Save new keys to IndexedDB for multi-device sync
        const deviceId = await walletDb.getDeviceId();
        const now = new Date().toISOString();

        // Mark spent key
        await walletDb.markKeySpent(publicKeyHex!, deviceId);

        // Save receiver key (if we generated it)
        if (!recipientPubkey && recipientKey) {
          const recvSeedHex = bytesToHex(recipientKey.seed);
          const recvStored = walletPassphrase
            ? await walletDb.encryptSeedHex(recvSeedHex, walletPassphrase)
            : recvSeedHex;
          await walletDb.importKeyIfNew({
            chainId: selectedChainId,
            publicKey: bytesToHex(recipientKey.publicKey),
            seedHex: recvStored,
            seedEncrypted: !!walletPassphrase,
            seqId: null,
            amount: receivers[0].amount.toString(),
            status: 'unconfirmed',
            acquiredAt: now,
            acquiredBy: deviceId,
            synced: false,
          });
        }

        // Save change key
        if (needsChange) {
          const changeSeedHex = bytesToHex(changeKey.seed);
          const changeStored = walletPassphrase
            ? await walletDb.encryptSeedHex(changeSeedHex, walletPassphrase)
            : changeSeedHex;
          await walletDb.importKeyIfNew({
            chainId: selectedChainId,
            publicKey: bytesToHex(changeKey.publicKey),
            seedHex: changeStored,
            seedEncrypted: !!walletPassphrase,
            seqId: null,
            amount: receivers[receivers.length - 1].amount.toString(),
            status: 'unconfirmed',
            acquiredAt: now,
            acquiredBy: deviceId,
            synced: false,
          });
        }

        await refreshKeyCounts();

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
      setPendingTransfer(null);

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

      {/* Unknown Spend Alert (WalletSync §7.1) */}
      {validationAlert && (
        <div style={{ marginBottom: 12, padding: 8, background: '#fee', border: '1px solid #c00', borderRadius: 4, fontSize: 12, color: '#900' }}>
          {validationAlert}
          <button
            onClick={() => setValidationAlert('')}
            style={{ marginLeft: 8, fontSize: 11, color: '#666' }}
          >
            Dismiss
          </button>
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
            {cachedBalance !== null && (
              <div style={{ fontSize: 12, marginBottom: 4, display: 'flex', alignItems: 'center', gap: 6 }}>
                <span>Cached balance: {cachedBalance} shares</span>
                {lastValidatedAt ? (
                  <span style={{ color: '#666' }}>
                    (verified {new Date(lastValidatedAt).toLocaleTimeString()})
                  </span>
                ) : (
                  <span style={{ background: '#fff3cd', padding: '1px 6px', borderRadius: 8, fontSize: 11 }}>
                    unverified
                  </span>
                )}
                {lastValidatedAt && Date.now() - lastValidatedAt > 3600000 && (
                  <span style={{ background: '#fff3cd', padding: '1px 6px', borderRadius: 8, fontSize: 11 }}>
                    stale
                  </span>
                )}
              </div>
            )}
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
      {storedSeedHex && selectedUtxo && !pendingTransfer && (
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
          <button onClick={handleBuild} disabled={loading}>
            {loading ? 'Building...' : 'Review Transfer'}
          </button>
        </div>
      )}

      {/* Confirmation Screen (N12) */}
      {pendingTransfer && (
        <div style={{ maxWidth: 400, padding: 12, background: '#f0f7ff', border: '1px solid #cce0ff', borderRadius: 6 }}>
          <div style={{ fontSize: 14, fontWeight: 600, marginBottom: 8 }}>Confirm Transfer</div>
          <table style={{ fontSize: 12, width: '100%', borderCollapse: 'collapse' }}>
            <tbody>
              <tr>
                <td style={{ padding: '4px 8px 4px 0', color: '#666' }}>Sending</td>
                <td style={{ padding: '4px 0', fontWeight: 500 }}>{pendingTransfer.sendAmount.toString()} shares</td>
              </tr>
              <tr>
                <td style={{ padding: '4px 8px 4px 0', color: '#666' }}>Fee</td>
                <td style={{ padding: '4px 0' }}>{pendingTransfer.fee.toString()} shares</td>
              </tr>
              {pendingTransfer.needsChange && (
                <tr>
                  <td style={{ padding: '4px 8px 4px 0', color: '#666' }}>Change returned</td>
                  <td style={{ padding: '4px 0' }}>{pendingTransfer.changeAmount.toString()} shares</td>
                </tr>
              )}
              <tr>
                <td style={{ padding: '4px 8px 4px 0', color: '#666' }}>Recipient</td>
                <td style={{ padding: '4px 0', fontFamily: 'monospace', fontSize: 11, wordBreak: 'break-all' }}>
                  {recipientPubkey
                    ? `${recipientPubkey.slice(0, 8)}...${recipientPubkey.slice(-8)}`
                    : 'New key (generated)'}
                </td>
              </tr>
              {attachments.length > 0 && (
                <tr>
                  <td style={{ padding: '4px 8px 4px 0', color: '#666' }}>Attachments</td>
                  <td style={{ padding: '4px 0' }}>{attachments.length} file{attachments.length > 1 ? 's' : ''}</td>
                </tr>
              )}
            </tbody>
          </table>
          <div style={{ display: 'flex', gap: 8, marginTop: 12 }}>
            <button
              onClick={() => setPendingTransfer(null)}
              disabled={loading}
              style={{ flex: 1 }}
            >
              Edit
            </button>
            <button
              onClick={handleConfirm}
              disabled={loading}
              style={{ flex: 1, background: '#0066cc', color: '#fff', border: 'none', borderRadius: 4, padding: '6px 12px', cursor: loading ? 'wait' : 'pointer' }}
            >
              {loading ? 'Sending...' : 'Confirm & Send'}
            </button>
          </div>
        </div>
      )}

      {status && (
        <pre style={{ marginTop: 12, padding: 8, background: '#f5f5f5', fontSize: 12, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
          {status}
        </pre>
      )}

      {/* Transaction History (N15) */}
      {storedSeedHex && <TransactionHistory />}
    </div>
  );
}
