import { useState } from 'react';
import { useStore } from '../store/useStore.ts';
import { QrScanner } from './QrScanner.tsx';
import { PairedDevices } from './PairedDevices.tsx';
import { WalletBackup } from './WalletBackup.tsx';
import { NotificationSettings } from './NotificationSettings.tsx';

/** Parse a chain info URL into recorder base URL and chain ID.
 *  Accepts: http://host:port/chain/{id}/info or http://host:port/chain/{id}
 *  Returns null if the URL doesn't match the expected pattern. */
function parseChainUrl(url: string): { recorderUrl: string; chainId: string } | null {
  const match = url.match(/^(https?:\/\/[^/]+)\/chain\/([^/]+)(\/info)?$/);
  if (!match) return null;
  return { recorderUrl: match[1], chainId: match[2] };
}

function PowerUserSettings() {
  const { showRefutation, setShowRefutation } = useStore();
  return (
    <div style={{ marginTop: 12, borderTop: '1px solid #eee', paddingTop: 12 }}>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 6 }}>Power User</div>
      <label style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13 }}>
        <input
          type="checkbox"
          checked={showRefutation}
          onChange={(e) => setShowRefutation(e.target.checked)}
        />
        Enable Refutation UI
      </label>
      <div style={{ fontSize: 11, color: '#999', marginTop: 2, marginLeft: 22 }}>
        Adds dispute buttons to transaction history. Refuting voids stale agreements you no longer intend to honor.
      </div>
    </div>
  );
}

export function Settings() {
  const { recorderUrl, setRecorderUrl, selectChain } = useStore();
  const [url, setUrl] = useState(recorderUrl);
  const [scanning, setScanning] = useState(false);
  const [showPairing, setShowPairing] = useState(false);

  function handleScanResult(data: string) {
    setScanning(false);
    const parsed = parseChainUrl(data);
    if (parsed) {
      setUrl(parsed.recorderUrl);
      setRecorderUrl(parsed.recorderUrl);
      // Select chain immediately — the store handles selecting a chain
      // that hasn't loaded yet (chainInfo resets to null until fetched).
      selectChain(parsed.chainId);
    } else if (data.startsWith('http')) {
      // Plain recorder URL without chain path
      setUrl(data);
      setRecorderUrl(data);
    }
  }

  return (
    <div style={{ padding: 16 }}>
      <h2 style={{ fontSize: 16, marginBottom: 8 }}>Settings</h2>
      <label style={{ display: 'block', marginBottom: 4, fontWeight: 500 }}>Recorder URL</label>
      <div style={{ display: 'flex', gap: 8 }}>
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          style={{ flex: 1, padding: '6px 8px', border: '1px solid #ccc', borderRadius: 4 }}
        />
        <button onClick={() => setRecorderUrl(url)}>Connect</button>
      </div>
      <button
        onClick={() => setScanning(true)}
        style={{ marginTop: 8, fontSize: 13 }}
      >
        Scan QR Code
      </button>
      {scanning && (
        <QrScanner
          onScan={handleScanResult}
          onClose={() => setScanning(false)}
        />
      )}

      {/* Payment Notifications */}
      <NotificationSettings />

      {/* Wallet Backup */}
      <WalletBackup />

      {/* Power User Features */}
      <PowerUserSettings />

      {/* Paired Devices */}
      <div style={{ marginTop: 12, borderTop: '1px solid #eee', paddingTop: 12 }}>
        <button onClick={() => setShowPairing(!showPairing)} style={{ fontSize: 13 }}>
          {showPairing ? 'Hide' : 'Paired Devices'}
        </button>
        {showPairing && <PairedDevices onClose={() => setShowPairing(false)} />}
      </div>
    </div>
  );
}
