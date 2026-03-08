// Paired Devices management panel — device pairing via QR + relay status.
// Spec: specs/WalletSync.md §4.1, §7.3

import { useState, useEffect } from 'react';
import { QrCode } from './QrCode.tsx';
import { QrScanner } from './QrScanner.tsx';
import {
  generateX25519Pair, deriveRelayKey, isX25519Available,
  serializePairPayload, deserializePairPayload,
  type PairInitPayload, type PairAckPayload,
} from '../core/pairing.ts';
import {
  getDeviceId, getDeviceLabel, setDeviceLabel,
  addPeer, getPeers, removePeer,
  type PeerDevice,
} from '../core/walletDb.ts';

interface PairedDevicesProps {
  onClose: () => void;
}

export function PairedDevices({ onClose }: PairedDevicesProps) {
  const [mode, setMode] = useState<'list' | 'initiate' | 'scan' | 'confirm'>('list');
  const [peers, setPeers] = useState<PeerDevice[]>([]);
  const [x25519Available, setX25519Available] = useState(true);
  const [status, setStatus] = useState('');
  const [label, setLabel] = useState('');
  const [myDeviceId, setMyDeviceId] = useState('');

  // Pairing state
  const [pairQr, setPairQr] = useState('');
  const [pendingPrivateKey, setPendingPrivateKey] = useState<CryptoKey | null>(null);
  const [confirmQr, setConfirmQr] = useState('');

  useEffect(() => {
    async function init() {
      setX25519Available(await isX25519Available());
      setPeers(await getPeers());
      setLabel(await getDeviceLabel());
      setMyDeviceId(await getDeviceId());
    }
    init();
  }, []);

  // ── Initiate Pairing (Device A) ────────────────────────────────────

  async function handleInitiate() {
    try {
      const { privateKey, publicKeyHex } = await generateX25519Pair();
      setPendingPrivateKey(privateKey);

      const payload: PairInitPayload = {
        v: 1,
        type: 'pair',
        pub: publicKeyHex,
        deviceId: myDeviceId,
        label: await getDeviceLabel(),
      };
      setPairQr(serializePairPayload(payload));
      setMode('initiate');
      setStatus('Show this QR code to the other device.');
    } catch (e) {
      setStatus(`Pairing error: ${e}`);
    }
  }

  // ── Scan Peer's QR (Device B scans Device A's init QR) ────────────

  async function handleScanResult(data: string) {
    try {
      const parsed = deserializePairPayload(data);

      if (parsed.type === 'pair') {
        // We are Device B scanning Device A's init QR
        const { privateKey, publicKeyHex } = await generateX25519Pair();
        const relayKey = await deriveRelayKey(privateKey, parsed.pub);

        // Save peer
        await addPeer({
          deviceId: parsed.deviceId,
          label: parsed.label,
          relayKey,
          pairedAt: new Date().toISOString(),
        });

        // Show our ack QR for Device A to scan
        const ackPayload: PairAckPayload = {
          v: 1,
          type: 'pair_ack',
          pub: publicKeyHex,
          deviceId: myDeviceId,
          label: await getDeviceLabel(),
        };
        setConfirmQr(serializePairPayload(ackPayload));
        setMode('confirm');
        setStatus(`Paired with "${parsed.label}". Show the confirmation QR to their device.`);
        setPeers(await getPeers());

      } else if (parsed.type === 'pair_ack') {
        // We are Device A scanning Device B's ack QR
        if (!pendingPrivateKey) {
          setStatus('Error: no pending pairing. Start over.');
          setMode('list');
          return;
        }
        const relayKey = await deriveRelayKey(pendingPrivateKey, parsed.pub);

        await addPeer({
          deviceId: parsed.deviceId,
          label: parsed.label,
          relayKey,
          pairedAt: new Date().toISOString(),
        });

        setPendingPrivateKey(null);
        setPairQr('');
        setMode('list');
        setStatus(`Paired with "${parsed.label}".`);
        setPeers(await getPeers());
      }
    } catch (e) {
      setStatus(`Scan error: ${e}`);
      setMode('list');
    }
  }

  // ── Unpair ─────────────────────────────────────────────────────────

  async function handleUnpair(deviceId: string) {
    await removePeer(deviceId);
    setPeers(await getPeers());
    setStatus('Device unpaired.');
  }

  // ── Label ──────────────────────────────────────────────────────────

  async function handleSaveLabel() {
    await setDeviceLabel(label);
    setStatus('Device name saved.');
  }

  // ── Render ─────────────────────────────────────────────────────────

  if (!x25519Available) {
    return (
      <div style={{ padding: 16 }}>
        <h3 style={{ fontSize: 15, marginBottom: 8 }}>Paired Devices</h3>
        <div style={{ color: '#666', fontSize: 12 }}>
          Device pairing requires X25519 support (Chrome 113+, Firefox 129+, Safari 17+).
          Your browser does not support this feature. Use QR sync instead.
        </div>
        <button onClick={onClose} style={{ marginTop: 8 }}>Close</button>
      </div>
    );
  }

  return (
    <div style={{ padding: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <h3 style={{ margin: 0, fontSize: 15 }}>Paired Devices</h3>
        <button onClick={onClose} style={{ fontSize: 12 }}>Close</button>
      </div>

      {/* Device Name */}
      <div style={{ marginBottom: 12, padding: 8, background: '#f9f9f9', borderRadius: 4 }}>
        <label style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>This device name:</label>
        <div style={{ display: 'flex', gap: 4 }}>
          <input
            value={label}
            onChange={e => setLabel(e.target.value)}
            style={{ flex: 1, padding: '4px 6px', fontSize: 12 }}
          />
          <button onClick={handleSaveLabel} style={{ fontSize: 12 }}>Save</button>
        </div>
        <div style={{ fontSize: 10, color: '#999', marginTop: 2 }}>ID: {myDeviceId}</div>
      </div>

      {/* Peer List */}
      {mode === 'list' && (
        <>
          {peers.length === 0 ? (
            <div style={{ fontSize: 12, color: '#666', marginBottom: 12 }}>
              No paired devices. Tap "Add Device" to pair.
            </div>
          ) : (
            <div style={{ marginBottom: 12 }}>
              {peers.map(p => (
                <div key={p.deviceId} style={{
                  display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                  padding: 8, background: '#f5f5f5', borderRadius: 4, marginBottom: 4,
                }}>
                  <div>
                    <div style={{ fontSize: 13, fontWeight: 500 }}>{p.label}</div>
                    <div style={{ fontSize: 10, color: '#999' }}>
                      Paired {new Date(p.pairedAt).toLocaleDateString()}
                      {p.lastSeen && ` — last seen ${new Date(p.lastSeen).toLocaleTimeString()}`}
                    </div>
                  </div>
                  <button
                    onClick={() => handleUnpair(p.deviceId)}
                    style={{ fontSize: 11, color: '#c00' }}
                  >
                    Unpair
                  </button>
                </div>
              ))}
            </div>
          )}

          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={handleInitiate}>Add Device</button>
            <button onClick={() => setMode('scan')}>Scan Pair Code</button>
          </div>
        </>
      )}

      {/* Initiate Pairing — show QR, then scan ack */}
      {mode === 'initiate' && pairQr && (
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: 12, color: '#666', marginBottom: 8 }}>
            Show this QR code to the other device
          </div>
          <QrCode value={pairQr} size={240} />
          <div style={{ marginTop: 12, display: 'flex', gap: 8, justifyContent: 'center' }}>
            <button onClick={() => setMode('scan')} style={{ fontSize: 12 }}>
              Scan Confirmation QR
            </button>
            <button onClick={() => { setMode('list'); setPairQr(''); setPendingPrivateKey(null); }} style={{ fontSize: 12 }}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {/* Scan mode */}
      {mode === 'scan' && (
        <QrScanner
          onScan={handleScanResult}
          onClose={() => setMode('list')}
        />
      )}

      {/* Confirm — show ack QR for Device A */}
      {mode === 'confirm' && confirmQr && (
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: 12, color: '#666', marginBottom: 8 }}>
            Show this confirmation QR to the first device
          </div>
          <QrCode value={confirmQr} size={240} />
          <button onClick={() => { setMode('list'); setConfirmQr(''); }} style={{ marginTop: 12, fontSize: 12 }}>
            Done
          </button>
        </div>
      )}

      {status && (
        <pre style={{ marginTop: 12, padding: 8, background: '#f5f5f5', fontSize: 12, whiteSpace: 'pre-wrap' }}>
          {status}
        </pre>
      )}
    </div>
  );
}
