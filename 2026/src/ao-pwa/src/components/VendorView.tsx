import { useState, useEffect, useRef } from 'react';
import { useStore } from '../store/useStore.ts';
import { RecorderClient } from '../api/client.ts';
import type { BlockInfo } from '../api/client.ts';
import { playChime, isInQuietHours, isQuickMuted } from '../core/chime.ts';
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
  const { recorderUrl, selectedChainId, chainInfo } = useStore();

  return (
    <div style={{ padding: 16 }}>
      <h3 style={{ fontSize: 15, marginBottom: 12 }}>AOS Vendor</h3>
      {selectedChainId && chainInfo ? (
        <>
          <VendorProfileEditor recorderUrl={recorderUrl} chainId={selectedChainId} />
          <IncomingMonitor
            recorderUrl={recorderUrl}
            chainId={selectedChainId}
            symbol={chainInfo.symbol}
          />
        </>
      ) : (
        <div style={{ marginBottom: 16, color: '#666', fontSize: 13 }}>
          Select a chain to monitor incoming payments.
        </div>
      )}
      <GenesisCreator recorderUrl={recorderUrl} />
    </div>
  );
}

// ── Vendor Profile Editor ──────────────────────────────────────────

function VendorProfileEditor({ recorderUrl, chainId }: { recorderUrl: string; chainId: string }) {
  const [name, setName] = useState('');
  const [desc, setDesc] = useState('');
  const [lat, setLat] = useState('');
  const [lon, setLon] = useState('');
  const [saved, setSaved] = useState(false);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    const client = new RecorderClient(recorderUrl);
    client.getProfile(chainId).then(p => {
      if (p.name) setName(p.name);
      if (p.description) setDesc(p.description);
      if (p.lat !== undefined) setLat(String(p.lat));
      if (p.lon !== undefined) setLon(String(p.lon));
      setLoaded(true);
    }).catch(() => setLoaded(true));
  }, [recorderUrl, chainId]);

  async function handleSave() {
    const client = new RecorderClient(recorderUrl);
    await client.setProfile(chainId, {
      name: name || undefined,
      description: desc || undefined,
      lat: lat ? parseFloat(lat) : undefined,
      lon: lon ? parseFloat(lon) : undefined,
    });
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  }

  function handleGeolocate() {
    if (!navigator.geolocation) return;
    navigator.geolocation.getCurrentPosition(
      (pos) => {
        setLat(pos.coords.latitude.toFixed(6));
        setLon(pos.coords.longitude.toFixed(6));
      },
      () => { /* permission denied — ignore */ }
    );
  }

  if (!loaded) return null;

  return (
    <div style={{ marginBottom: 16, padding: 12, background: '#f9f9f9', borderRadius: 4 }}>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>Vendor Profile</div>
      <div style={{ display: 'grid', gap: 6, maxWidth: 400 }}>
        <input value={name} onChange={e => setName(e.target.value)}
          placeholder="Business name" style={{ padding: '4px 6px' }} />
        <input value={desc} onChange={e => setDesc(e.target.value)}
          placeholder="Description / menu" style={{ padding: '4px 6px' }} />
        <div style={{ display: 'flex', gap: 6 }}>
          <input value={lat} onChange={e => setLat(e.target.value)}
            placeholder="Latitude" style={{ flex: 1, padding: '4px 6px' }} />
          <input value={lon} onChange={e => setLon(e.target.value)}
            placeholder="Longitude" style={{ flex: 1, padding: '4px 6px' }} />
          <button onClick={handleGeolocate} style={{ fontSize: 12 }}>GPS</button>
        </div>
        <button onClick={handleSave}>
          {saved ? 'Saved!' : 'Save Profile'}
        </button>
      </div>
    </div>
  );
}

// ── SSE Incoming Payment Monitor ──────────────────────────────────
interface MonitorProps {
  recorderUrl: string;
  chainId: string;
  symbol: string;
}

function IncomingMonitor({ recorderUrl, chainId, symbol }: MonitorProps) {
  const [events, setEvents] = useState<BlockInfo[]>([]);
  const [connected, setConnected] = useState(false);
  const esRef = useRef<EventSource | null>(null);
  const notification = useStore(s => s.notification);
  // Ref avoids SSE reconnection when notification settings change
  const notifRef = useRef(notification);
  notifRef.current = notification;

  useEffect(() => {
    const client = new RecorderClient(recorderUrl);
    const es = client.subscribeBlocks(chainId, (block) => {
      setEvents(prev => [block, ...prev].slice(0, 50));
      const n = notifRef.current;
      if (block.seq_count > 0
        && n.enabled
        && !isInQuietHours(n.quietStart, n.quietEnd)
        && !isQuickMuted(n.quickMuteUntil)
      ) {
        playChime(n.chimeStyle, n.volume);
      }
    });

    es.onopen = () => setConnected(true);
    es.onerror = () => setConnected(false);
    esRef.current = es;

    return () => {
      es.close();
      esRef.current = null;
    };
  }, [recorderUrl, chainId]);

  return (
    <div style={{ marginBottom: 16, padding: 12, background: '#f9f9f9', borderRadius: 4 }}>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>
        Incoming Payments — {symbol}
        <span style={{
          marginLeft: 8,
          fontSize: 11,
          color: connected ? '#090' : '#c00',
        }}>
          {connected ? 'SSE connected' : 'connecting...'}
        </span>
      </div>
      {events.length === 0 ? (
        <div style={{ fontSize: 12, color: '#999' }}>Waiting for new blocks...</div>
      ) : (
        <div style={{ maxHeight: 200, overflow: 'auto' }}>
          {events.map((block, i) => (
            <div key={`${block.height}-${i}`} style={{
              fontSize: 12, fontFamily: 'monospace', padding: '4px 0',
              borderBottom: '1px solid #eee',
            }}>
              Block #{block.height} — {block.seq_count} assignment{block.seq_count !== 1 ? 's' : ''}
              {' '}(seq {block.first_seq}–{block.first_seq + block.seq_count - 1})
              {' '}— {block.hash.slice(0, 12)}...
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Genesis Chain Creator ─────────────────────────────────────────
function GenesisCreator({ recorderUrl }: { recorderUrl: string }) {
  const [symbol, setSymbol] = useState('');
  const [description, setDescription] = useState('');
  const [coins, setCoins] = useState('1000000000');
  const [shares, setShares] = useState('1099511627776'); // 2^40
  const [seedHex, setSeedHex] = useState('');
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);
  const [collapsed, setCollapsed] = useState(true);

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
    <div>
      <div
        onClick={() => setCollapsed(!collapsed)}
        style={{ fontSize: 14, fontWeight: 500, cursor: 'pointer', marginBottom: 8 }}
      >
        {collapsed ? '+ Create New Chain' : '- Create New Chain'}
      </div>
      {!collapsed && (
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
      )}
      {status && (
        <pre style={{ marginTop: 12, padding: 8, background: '#f5f5f5', fontSize: 12, whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
          {status}
        </pre>
      )}
    </div>
  );
}
