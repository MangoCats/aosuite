import { useState } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import { signingKeyFromSeed, generateSigningKey } from '../core/sign.ts';
import { buildAuthorizationJson } from '../core/assignment.ts';
import { recordingFee } from '../core/fees.ts';
import { toBytes } from '../core/dataitem.ts';
import { buildAssignment } from '../core/assignment.ts';
import { bytesToHex, hexToBytes } from '../core/hex.ts';
import type { Giver, Receiver, FeeRate } from '../core/assignment.ts';

export function ConsumerView() {
  const { recorderUrl, selectedChainId, chainInfo } = useStore();
  const [seedHex, setSeedHex] = useState('');
  const [giverSeqId, setGiverSeqId] = useState('');
  const [giverAmount, setGiverAmount] = useState('');
  const [receiverAmount, setReceiverAmount] = useState('');
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);

  if (!selectedChainId || !chainInfo) {
    return <div style={{ padding: 16, color: '#666' }}>Select a chain first.</div>;
  }

  async function handleTransfer() {
    if (!chainInfo || !selectedChainId) return;
    setLoading(true);
    setStatus('Building transfer...');

    try {
      const client = new RecorderClient(recorderUrl);

      // Create giver key from seed
      const giverSeed = hexToBytes(seedHex);
      const giverKey = await signingKeyFromSeed(giverSeed);

      // Generate fresh receiver key
      const receiverKey = await generateSigningKey();

      const feeRate: FeeRate = {
        num: BigInt(chainInfo.fee_rate_num),
        den: BigInt(chainInfo.fee_rate_den),
      };

      const giverAmt = BigInt(giverAmount);
      const sharesOut = BigInt(chainInfo.shares_out);

      // Build assignment to estimate fee
      const givers: Giver[] = [{
        seqId: BigInt(giverSeqId),
        amount: giverAmt,
        key: giverKey,
      }];
      const receivers: Receiver[] = [{
        pubkey: receiverKey.publicKey,
        amount: BigInt(receiverAmount || '0'),
        key: receiverKey,
      }];

      // Iterative fee convergence (3 rounds)
      for (let i = 0; i < 3; i++) {
        const assignment = buildAssignment(givers, receivers, feeRate);
        const pageBytes = BigInt(toBytes(assignment).length + 200); // estimate with sigs
        const fee = recordingFee(pageBytes, feeRate.num, feeRate.den, sharesOut);
        receivers[0].amount = giverAmt - fee;
      }

      if (receivers[0].amount <= 0n) {
        setStatus('Error: insufficient funds after fee');
        setLoading(false);
        return;
      }

      setStatus(`Signing (receiver gets ${receivers[0].amount} shares)...`);
      const authJson = await buildAuthorizationJson(givers, receivers, feeRate);

      setStatus('Submitting to recorder...');
      const result = await client.submit(selectedChainId, authJson);

      setStatus(
        `Block ${result.height} recorded! Hash: ${result.hash.slice(0, 16)}... ` +
        `Receiver key: ${bytesToHex(receiverKey.publicKey).slice(0, 16)}... ` +
        `New seed: ${bytesToHex(receiverKey.seed)}`
      );
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
    setLoading(false);
  }

  return (
    <div style={{ padding: 16 }}>
      <h3 style={{ fontSize: 15, marginBottom: 12 }}>AOE Consumer — Transfer</h3>
      <div style={{ display: 'grid', gap: 8, maxWidth: 400 }}>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Your seed (hex, 64 chars)</span>
          <input
            value={seedHex}
            onChange={e => setSeedHex(e.target.value)}
            style={{ width: '100%', padding: '4px 6px', fontFamily: 'monospace', fontSize: 12 }}
            placeholder="ed25519 seed hex..."
          />
        </label>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>UTXO Seq ID to spend</span>
          <input
            value={giverSeqId}
            onChange={e => setGiverSeqId(e.target.value)}
            style={{ width: '100%', padding: '4px 6px' }}
            placeholder="e.g. 1"
          />
        </label>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Amount (shares)</span>
          <input
            value={giverAmount}
            onChange={e => setGiverAmount(e.target.value)}
            style={{ width: '100%', padding: '4px 6px' }}
            placeholder="e.g. 1000000"
          />
        </label>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Receiver amount (auto if blank)</span>
          <input
            value={receiverAmount}
            onChange={e => setReceiverAmount(e.target.value)}
            style={{ width: '100%', padding: '4px 6px' }}
            placeholder="auto (giver - fee)"
          />
        </label>
        <button onClick={handleTransfer} disabled={loading || !seedHex || !giverSeqId || !giverAmount}>
          {loading ? 'Processing...' : 'Transfer'}
        </button>
      </div>
      {status && (
        <pre style={{ marginTop: 12, padding: 8, background: '#f5f5f5', fontSize: 12, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
          {status}
        </pre>
      )}
    </div>
  );
}
