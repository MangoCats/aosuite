import { useState } from 'react';
import { useStore } from '../store/useStore.ts';
import { QrScanner } from './QrScanner.tsx';

/** Parse a chain info URL into recorder base URL and chain ID.
 *  Accepts: http://host:port/chain/{id}/info or http://host:port/chain/{id}
 *  Returns null if the URL doesn't match the expected pattern. */
function parseChainUrl(url: string): { recorderUrl: string; chainId: string } | null {
  const match = url.match(/^(https?:\/\/[^/]+)\/chain\/([^/]+)(\/info)?$/);
  if (!match) return null;
  return { recorderUrl: match[1], chainId: match[2] };
}

export function Settings() {
  const { recorderUrl, setRecorderUrl, selectChain } = useStore();
  const [url, setUrl] = useState(recorderUrl);
  const [scanning, setScanning] = useState(false);

  function handleScanResult(data: string) {
    setScanning(false);
    const parsed = parseChainUrl(data);
    if (parsed) {
      setUrl(parsed.recorderUrl);
      setRecorderUrl(parsed.recorderUrl);
      // Auto-select the chain after a short delay for chain list to load
      setTimeout(() => selectChain(parsed.chainId), 500);
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
    </div>
  );
}
