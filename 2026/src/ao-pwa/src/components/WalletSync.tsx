// WalletSync component — QR-based key transfer between devices.
// Spec: specs/WalletSync.md §3

import { useState, useEffect, useRef } from 'react';
import { QrScanner } from './QrScanner.tsx';
import { AnimatedQrCode, createCollector, collectFrame, parseFrame, type FrameCollector } from './AnimatedQrCode.tsx';
import {
  buildSyncPayload, buildFullExportPayload,
  importSyncPayload, serializePayload, deserializePayload,
  type SyncPayload,
} from '../core/walletSync.ts';
import { getUnsyncedKeys, markAllSynced } from '../core/walletDb.ts';
import { useStore } from '../store/useStore.ts';

interface WalletSyncProps {
  onClose: () => void;
}

export function WalletSync({ onClose }: WalletSyncProps) {
  const { walletPassphrase } = useStore();
  const [mode, setMode] = useState<'menu' | 'export' | 'export-full' | 'scan' | 'import-file'>('menu');
  const [payload, setPayload] = useState<string>('');
  const [unsyncedCount, setUnsyncedCount] = useState(0);
  const [status, setStatus] = useState('');
  const [importText, setImportText] = useState('');
  const collectorRef = useRef<FrameCollector>(createCollector());
  const [framesCollected, setFramesCollected] = useState(0);

  useEffect(() => {
    getUnsyncedKeys().then(keys => setUnsyncedCount(keys.length));
  }, []);

  async function handleExport() {
    const p = await buildSyncPayload(walletPassphrase);
    setPayload(serializePayload(p));
    setMode('export');
  }

  async function handleExportFull() {
    const p = await buildFullExportPayload(walletPassphrase);
    setPayload(serializePayload(p));
    setMode('export-full');
  }

  async function handleScanResult(data: string) {
    // Check if this is an animated QR frame
    const frame = parseFrame(data);
    if (frame) {
      const assembled = collectFrame(collectorRef.current, data);
      setFramesCollected(collectorRef.current.frames.size);
      if (!assembled) {
        // Still collecting frames — stay in scan mode
        setStatus(`Collecting frames: ${collectorRef.current.frames.size}/${frame.n}`);
        return;
      }
      // All frames collected — import the reassembled payload
      data = assembled;
      collectorRef.current = createCollector();
      setFramesCollected(0);
    }

    setMode('menu');
    try {
      const p = deserializePayload(data);
      const result = await importSyncPayload(p, walletPassphrase);
      setStatus(`Imported ${result.imported} key${result.imported !== 1 ? 's' : ''}, ${result.spentMarked} spent notification${result.spentMarked !== 1 ? 's' : ''}.`);
      setUnsyncedCount((await getUnsyncedKeys()).length);
    } catch (e) {
      setStatus(`Import error: ${e}`);
    }
  }

  async function handleMarkSynced() {
    await markAllSynced();
    setUnsyncedCount(0);
    setStatus('All keys marked as synced.');
  }

  async function handleImportText() {
    try {
      const p = deserializePayload(importText);
      const result = await importSyncPayload(p, walletPassphrase);
      setStatus(`Imported ${result.imported} key${result.imported !== 1 ? 's' : ''}, ${result.spentMarked} spent notification${result.spentMarked !== 1 ? 's' : ''}.`);
      setImportText('');
      setMode('menu');
      setUnsyncedCount((await getUnsyncedKeys()).length);
    } catch (e) {
      setStatus(`Import error: ${e}`);
    }
  }

  async function handleFileImport(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const p = deserializePayload(text);
      const result = await importSyncPayload(p, walletPassphrase);
      setStatus(`Imported ${result.imported} key${result.imported !== 1 ? 's' : ''} from file.`);
      setMode('menu');
      setUnsyncedCount((await getUnsyncedKeys()).length);
    } catch (err) {
      setStatus(`File import error: ${err}`);
    }
  }

  function handleFileSave() {
    const blob = new Blob([payload], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `ao-wallet-sync-${new Date().toISOString().slice(0, 10)}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }

  return (
    <div style={{ padding: 16 }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <h3 style={{ margin: 0, fontSize: 15 }}>Wallet Sync</h3>
        <button onClick={onClose} style={{ fontSize: 12 }}>Close</button>
      </div>

      {unsyncedCount > 0 && mode === 'menu' && (
        <div style={{ marginBottom: 12, padding: 8, background: '#fff3cd', borderRadius: 4, fontSize: 12 }}>
          {unsyncedCount} key{unsyncedCount !== 1 ? 's' : ''} not yet synced to other devices.
        </div>
      )}

      {mode === 'menu' && (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
          <button onClick={handleExport} disabled={unsyncedCount === 0}>
            Show Sync QR ({unsyncedCount} key{unsyncedCount !== 1 ? 's' : ''})
          </button>
          <button onClick={handleExportFull}>
            Full Wallet Export (QR)
          </button>
          <button onClick={() => { collectorRef.current = createCollector(); setFramesCollected(0); setMode('scan'); }}>
            Scan Sync QR from Another Device
          </button>
          <button onClick={() => setMode('import-file')}>
            Import from File
          </button>
          {unsyncedCount > 0 && (
            <button onClick={handleMarkSynced} style={{ fontSize: 12, color: '#666' }}>
              Mark all as synced
            </button>
          )}
        </div>
      )}

      {(mode === 'export' || mode === 'export-full') && payload && (
        <div style={{ textAlign: 'center' }}>
          <div style={{ fontSize: 12, marginBottom: 8, color: '#666' }}>
            {mode === 'export-full' ? 'Full wallet' : `${unsyncedCount} unsynced key${unsyncedCount !== 1 ? 's' : ''}`}
            {' — scan from another device'}
          </div>
          <AnimatedQrCode value={payload} size={280} />
          <div style={{ marginTop: 12, display: 'flex', gap: 8, justifyContent: 'center' }}>
            <button onClick={handleFileSave} style={{ fontSize: 12 }}>Save as File</button>
            <button onClick={async () => { await handleMarkSynced(); setMode('menu'); }} style={{ fontSize: 12 }}>
              Done
            </button>
          </div>
        </div>
      )}

      {mode === 'scan' && (
        <>
        {framesCollected > 0 && (
          <div style={{ fontSize: 12, color: '#666', marginBottom: 8, textAlign: 'center' }}>
            Collecting animated QR frames: {framesCollected}/{collectorRef.current.total ?? '?'}
          </div>
        )}
        <QrScanner
          onScan={handleScanResult}
          onClose={() => { setMode('menu'); collectorRef.current = createCollector(); setFramesCollected(0); }}
        />
        </>
      )}

      {mode === 'import-file' && (
        <div>
          <div style={{ marginBottom: 8 }}>
            <label style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>Load sync file:</label>
            <input type="file" accept=".json" onChange={handleFileImport} />
          </div>
          <div style={{ marginTop: 12 }}>
            <label style={{ fontSize: 12, display: 'block', marginBottom: 4 }}>Or paste sync JSON:</label>
            <textarea
              value={importText}
              onChange={e => setImportText(e.target.value)}
              style={{ width: '100%', height: 120, fontFamily: 'monospace', fontSize: 11 }}
              placeholder='{"v":1,"type":"key_sync",...}'
            />
            <button onClick={handleImportText} disabled={!importText} style={{ marginTop: 4 }}>
              Import
            </button>
          </div>
          <button onClick={() => setMode('menu')} style={{ marginTop: 8, fontSize: 12 }}>Back</button>
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
