// Passphrase prompt — shown on app start when wallet has keys but no
// passphrase is set for the session. Also used to set a new passphrase.
// Spec: specs/WalletSync.md §6 (seed encryption at rest)

import { useState } from 'react';
import { useStore } from '../store/useStore.ts';
import { encryptAllSeeds } from '../core/walletDb.ts';

interface PassphrasePromptProps {
  /** True if this is first-time setup (no encrypted seeds exist yet). */
  isSetup: boolean;
  onComplete: () => void;
}

export function PassphrasePrompt({ isSetup, onComplete }: PassphrasePromptProps) {
  const { setWalletPassphrase } = useStore();
  const [passphrase, setPassphrase] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState('');
  const [encrypting, setEncrypting] = useState(false);

  async function handleSubmit() {
    if (isSetup) {
      if (passphrase.length < 8) {
        setError('Passphrase must be at least 8 characters.');
        return;
      }
      if (passphrase.length > 1024) {
        setError('Passphrase must be at most 1024 characters.');
        return;
      }
      if (passphrase !== confirm) {
        setError('Passphrases do not match.');
        return;
      }
      setEncrypting(true);
      try {
        await encryptAllSeeds(passphrase);
        setWalletPassphrase(passphrase);
        onComplete();
      } catch (e) {
        setError(`Encryption failed: ${e}`);
        setEncrypting(false);
      }
    } else {
      // Unlock — just store the passphrase in session
      setWalletPassphrase(passphrase);
      onComplete();
    }
  }

  function handleSkip() {
    // Allow skipping — seeds remain plaintext until passphrase is set
    setWalletPassphrase(null);
    onComplete();
  }

  return (
    <div style={{
      position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.5)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 1000,
    }}>
      <div style={{
        background: '#fff', borderRadius: 8, padding: 24, maxWidth: 400, width: '90%',
      }}>
        <h3 style={{ margin: '0 0 12px', fontSize: 16 }}>
          {isSetup ? 'Set Wallet Passphrase' : 'Unlock Wallet'}
        </h3>

        {isSetup && (
          <p style={{ fontSize: 12, color: '#666', marginBottom: 12 }}>
            Your passphrase encrypts key seeds stored on this device.
            Choose a strong passphrase — it is never sent to any server.
          </p>
        )}

        <input
          type="password"
          value={passphrase}
          onChange={e => { setPassphrase(e.target.value); setError(''); }}
          placeholder={isSetup ? 'New passphrase (8+ chars)' : 'Enter passphrase'}
          style={{ width: '100%', padding: '8px', marginBottom: 8, boxSizing: 'border-box' }}
          autoFocus
          onKeyDown={e => { if (e.key === 'Enter' && !isSetup) handleSubmit(); }}
        />

        {isSetup && (
          <input
            type="password"
            value={confirm}
            onChange={e => { setConfirm(e.target.value); setError(''); }}
            placeholder="Confirm passphrase"
            style={{ width: '100%', padding: '8px', marginBottom: 8, boxSizing: 'border-box' }}
            onKeyDown={e => { if (e.key === 'Enter') handleSubmit(); }}
          />
        )}

        {error && (
          <div style={{ color: '#c00', fontSize: 12, marginBottom: 8 }}>{error}</div>
        )}

        <div style={{ display: 'flex', gap: 8, justifyContent: 'flex-end' }}>
          {isSetup && (
            <button onClick={handleSkip} style={{ fontSize: 12, color: '#666' }}>
              Skip for now
            </button>
          )}
          <button onClick={handleSubmit} disabled={encrypting || !passphrase}>
            {encrypting ? 'Encrypting...' : isSetup ? 'Set Passphrase' : 'Unlock'}
          </button>
        </div>
      </div>
    </div>
  );
}
