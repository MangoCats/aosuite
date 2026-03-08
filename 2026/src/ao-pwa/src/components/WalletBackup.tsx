import { useState, useEffect } from 'react';
import { useStore } from '../store/useStore.ts';
import { buildFullExportPayload, importSyncPayload, serializePayload, deserializePayload } from '../core/walletSync.ts';
import { getLastBackupAt, setLastBackupAt, getKeys, getUnsyncedKeys } from '../core/walletDb.ts';
import { encryptBackup, decryptBackup, type BackupFile } from '../core/backup.ts';

export function WalletBackup() {
  const { walletPassphrase, setWalletKeyCount, setUnsyncedKeyCount } = useStore();

  const [mode, setMode] = useState<'idle' | 'export' | 'import'>('idle');
  const [backupPassword, setBackupPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);
  const [importFile, setImportFile] = useState<File | null>(null);

  // Backup age warning
  const [lastBackup, setLastBackup] = useState<string | null>(null);
  useEffect(() => {
    getLastBackupAt().then(setLastBackup);
  }, []);

  const daysSinceBackup = lastBackup
    ? Math.floor((Date.now() - new Date(lastBackup).getTime()) / 86_400_000)
    : null;
  const needsBackup = daysSinceBackup === null || daysSinceBackup > 30;

  async function handleExport() {
    if (backupPassword.length < 8) {
      setStatus('Backup password must be at least 8 characters.');
      return;
    }
    if (backupPassword !== confirmPassword) {
      setStatus('Passwords do not match.');
      return;
    }
    setLoading(true);
    setStatus('Building backup...');
    try {
      const payload = await buildFullExportPayload(walletPassphrase);
      const json = serializePayload(payload);
      const encrypted = await encryptBackup(json, backupPassword);
      const backup: BackupFile = {
        v: 1,
        type: 'ao_wallet_backup',
        created: new Date().toISOString(),
        encrypted,
      };
      const blob = new Blob([JSON.stringify(backup, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `ao-wallet-backup-${new Date().toISOString().slice(0, 10)}.json`;
      a.click();
      URL.revokeObjectURL(url);

      await setLastBackupAt(new Date().toISOString());
      setLastBackup(new Date().toISOString());
      setStatus(`Backup exported (${payload.keys.length} key${payload.keys.length !== 1 ? 's' : ''}).`);
      setMode('idle');
      setBackupPassword('');
      setConfirmPassword('');
    } catch (e) {
      setStatus(`Export failed: ${e}`);
    }
    setLoading(false);
  }

  async function handleImport() {
    if (!importFile || backupPassword.length < 1) {
      setStatus('Select a file and enter the backup password.');
      return;
    }
    setLoading(true);
    setStatus('Decrypting backup...');
    try {
      const text = await importFile.text();
      const backup = JSON.parse(text) as BackupFile;
      if (backup.type !== 'ao_wallet_backup' || backup.v !== 1) {
        setStatus('Not a valid AO wallet backup file.');
        setLoading(false);
        return;
      }
      const decrypted = await decryptBackup(backup.encrypted, backupPassword);
      const payload = deserializePayload(decrypted);
      const result = await importSyncPayload(payload, walletPassphrase);

      // Refresh counts
      const allKeys = await getKeys();
      setWalletKeyCount(allKeys.length);
      const unsynced = await getUnsyncedKeys();
      setUnsyncedKeyCount(unsynced.length);

      setStatus(`Imported ${result.imported} key${result.imported !== 1 ? 's' : ''}, ${result.spentMarked} spent.`);
      setMode('idle');
      setBackupPassword('');
      setImportFile(null);
    } catch (e) {
      if (e instanceof DOMException && e.name === 'OperationError') {
        setStatus('Wrong password or corrupted backup file.');
      } else {
        setStatus(`Import failed: ${e}`);
      }
    }
    setLoading(false);
  }

  return (
    <div style={{ marginTop: 12, borderTop: '1px solid #eee', paddingTop: 12 }}>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 6 }}>Wallet Backup</div>

      {needsBackup && mode === 'idle' && (
        <div style={{ fontSize: 11, color: '#996600', background: '#fff8e0', padding: '4px 8px', borderRadius: 4, marginBottom: 8 }}>
          {daysSinceBackup === null
            ? 'No backup exists. Back up your wallet now.'
            : `Last backup: ${daysSinceBackup} days ago. Consider backing up.`}
        </div>
      )}

      {mode === 'idle' && (
        <div style={{ display: 'flex', gap: 8 }}>
          <button onClick={() => { setMode('export'); setStatus(''); }} style={{ fontSize: 12 }}>
            Export Backup
          </button>
          <button onClick={() => { setMode('import'); setStatus(''); }} style={{ fontSize: 12 }}>
            Import Backup
          </button>
        </div>
      )}

      {mode === 'export' && (
        <div style={{ display: 'grid', gap: 6, maxWidth: 300 }}>
          <input
            type="password"
            value={backupPassword}
            onChange={e => setBackupPassword(e.target.value)}
            placeholder="Backup password (8+ chars)"
            style={{ padding: '4px 6px', fontSize: 12 }}
          />
          <input
            type="password"
            value={confirmPassword}
            onChange={e => setConfirmPassword(e.target.value)}
            placeholder="Confirm password"
            style={{ padding: '4px 6px', fontSize: 12 }}
          />
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={() => { setMode('idle'); setStatus(''); }} style={{ fontSize: 12 }}>
              Cancel
            </button>
            <button onClick={handleExport} disabled={loading} style={{ fontSize: 12 }}>
              {loading ? 'Encrypting...' : 'Download Backup'}
            </button>
          </div>
        </div>
      )}

      {mode === 'import' && (
        <div style={{ display: 'grid', gap: 6, maxWidth: 300 }}>
          <input
            type="file"
            accept=".json"
            onChange={e => setImportFile(e.target.files?.[0] ?? null)}
            style={{ fontSize: 12 }}
          />
          <input
            type="password"
            value={backupPassword}
            onChange={e => setBackupPassword(e.target.value)}
            placeholder="Backup password"
            style={{ padding: '4px 6px', fontSize: 12 }}
          />
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={() => { setMode('idle'); setStatus(''); }} style={{ fontSize: 12 }}>
              Cancel
            </button>
            <button onClick={handleImport} disabled={loading || !importFile} style={{ fontSize: 12 }}>
              {loading ? 'Decrypting...' : 'Restore from Backup'}
            </button>
          </div>
        </div>
      )}

      {status && (
        <div style={{ fontSize: 11, marginTop: 6, color: status.startsWith('Export') || status.startsWith('Import') ? '#090' : '#c00' }}>
          {status}
        </div>
      )}
    </div>
  );
}
