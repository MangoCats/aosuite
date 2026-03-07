import { useState } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import { signingKeyFromSeed } from '../core/sign.ts';
import { bytesToHex, hexToBytes } from '../core/hex.ts';
import * as tc from '../core/typecodes.ts';
import {
  containerItem, vbcItem, bytesItem, toJson, encodeDataItem,
} from '../core/dataitem.ts';
import { encodeBigint, encodeRational } from '../core/bigint.ts';
import { fromUnixSeconds, timestampToBytes, nowUnixSeconds } from '../core/timestamp.ts';
import { signDataItem } from '../core/sign.ts';
import { sha256 } from '../core/hash.ts';

export function VendorView() {
  const { recorderUrl } = useStore();
  const [symbol, setSymbol] = useState('');
  const [description, setDescription] = useState('');
  const [coins, setCoins] = useState('1000000000');
  const [shares, setShares] = useState('1099511627776'); // 2^40
  const [seedHex, setSeedHex] = useState('');
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);

  async function handleCreateChain() {
    setLoading(true);
    setStatus('Building genesis...');

    try {
      const client = new RecorderClient(recorderUrl);

      // Generate or use provided seed
      let seed: Uint8Array;
      if (seedHex) {
        seed = hexToBytes(seedHex);
      } else {
        seed = new Uint8Array(32);
        crypto.getRandomValues(seed);
      }
      const issuerKey = await signingKeyFromSeed(seed);

      const sharesVal = BigInt(shares);
      const coinsVal = BigInt(coins);
      const feeNum = 1n;
      const feeDen = coinsVal * 4000n;

      const sharesBytes = encodeBigint(sharesVal);
      const coinBytes = encodeBigint(coinsVal);
      const feeBytes = encodeRational({ num: feeNum, den: feeDen });
      const expiryPeriod = fromUnixSeconds(31_557_600n); // 1 year
      const nowSecs = nowUnixSeconds();
      const ts = fromUnixSeconds(nowSecs);

      const signableChildren = [
        vbcItem(tc.PROTOCOL_VER, 1n),
        bytesItem(tc.CHAIN_SYMBOL, new TextEncoder().encode(symbol)),
        bytesItem(tc.DESCRIPTION, new TextEncoder().encode(description)),
        bytesItem(tc.COIN_COUNT, coinBytes),
        bytesItem(tc.SHARES_OUT, sharesBytes),
        bytesItem(tc.FEE_RATE, feeBytes),
        bytesItem(tc.EXPIRY_PERIOD, timestampToBytes(expiryPeriod)),
        vbcItem(tc.EXPIRY_MODE, 1n),
        containerItem(tc.PARTICIPANT, [
          bytesItem(tc.ED25519_PUB, issuerKey.publicKey),
          bytesItem(tc.AMOUNT, sharesBytes),
        ]),
      ];
      const signable = containerItem(tc.GENESIS, signableChildren);

      setStatus('Signing genesis...');
      const sig = await signDataItem(issuerKey, signable, ts);

      // Build full genesis with AUTH_SIG and chain ID hash
      const allChildren = [
        ...signableChildren,
        containerItem(tc.AUTH_SIG, [
          bytesItem(tc.ED25519_SIG, sig),
          bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
        ]),
      ];

      // Compute chain ID hash: SHA256 of all children bytes concatenated
      const parts: Uint8Array[] = allChildren.map(c => encodeDataItem(c));
      let totalLen = 0;
      for (const p of parts) totalLen += p.length;
      const contentBytes = new Uint8Array(totalLen);
      let offset = 0;
      for (const p of parts) {
        contentBytes.set(p, offset);
        offset += p.length;
      }
      const chainHash = await sha256(contentBytes);
      allChildren.push(bytesItem(tc.SHA256, chainHash));

      const genesis = containerItem(tc.GENESIS, allChildren);
      const genesisJson = toJson(genesis);

      setStatus('Submitting genesis to recorder...');
      const info = await client.createChain(genesisJson);

      setStatus(
        `Chain created! ${info.symbol} (${info.chain_id.slice(0, 16)}...)\n` +
        `Issuer seed: ${bytesToHex(seed)}\n` +
        `Public key: ${bytesToHex(issuerKey.publicKey)}\n` +
        `Block height: ${info.block_height}, Shares: ${info.shares_out}`
      );
    } catch (e) {
      setStatus(`Error: ${e}`);
    }
    setLoading(false);
  }

  return (
    <div style={{ padding: 16 }}>
      <h3 style={{ fontSize: 15, marginBottom: 12 }}>AOS Vendor — Create Chain</h3>
      <div style={{ display: 'grid', gap: 8, maxWidth: 400 }}>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Symbol</span>
          <input
            value={symbol}
            onChange={e => setSymbol(e.target.value)}
            style={{ width: '100%', padding: '4px 6px' }}
            placeholder="e.g. BCG"
          />
        </label>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Description</span>
          <input
            value={description}
            onChange={e => setDescription(e.target.value)}
            style={{ width: '100%', padding: '4px 6px' }}
            placeholder="e.g. Bob's Curry Goat"
          />
        </label>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Coins</span>
          <input
            value={coins}
            onChange={e => setCoins(e.target.value)}
            style={{ width: '100%', padding: '4px 6px' }}
          />
        </label>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Shares</span>
          <input
            value={shares}
            onChange={e => setShares(e.target.value)}
            style={{ width: '100%', padding: '4px 6px' }}
          />
        </label>
        <label>
          <span style={{ fontSize: 13, fontWeight: 500 }}>Issuer seed (hex, leave blank to generate)</span>
          <input
            value={seedHex}
            onChange={e => setSeedHex(e.target.value)}
            style={{ width: '100%', padding: '4px 6px', fontFamily: 'monospace', fontSize: 12 }}
            placeholder="auto-generate"
          />
        </label>
        <button onClick={handleCreateChain} disabled={loading || !symbol}>
          {loading ? 'Creating...' : 'Create Chain'}
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
