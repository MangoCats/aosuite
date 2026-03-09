import { useState, useEffect, useRef } from 'react';
import { Header } from './components/Header.tsx';
import { ChainList } from './components/ChainList.tsx';
import { ChainDetail } from './components/ChainDetail.tsx';
import { Settings } from './components/Settings.tsx';
import { ConsumerView } from './components/ConsumerView.tsx';
import { VendorView } from './components/VendorView.tsx';
import { InvestorView } from './components/InvestorView.tsx';
import { CooperativeView } from './components/CooperativeView.tsx';
import { WalletSync } from './components/WalletSync.tsx';
import { PassphrasePrompt } from './components/PassphrasePrompt.tsx';
import { useStore } from './store/useStore.ts';
import { migrateFromLocalStorage, getUnsyncedKeys, getKeys, getUnspentKeys, getPeers, getSeedHex } from './core/walletDb.ts';
import { RelayClient } from './core/relayClient.ts';

function ViewContent() {
  const { view } = useStore();
  if (view === 'investor') return <InvestorView />;
  if (view === 'vendor') return <VendorView />;
  if (view === 'cooperative') return <CooperativeView />;
  return <ConsumerView />;
}

function App() {
  const { error, view, relayUrl, walletPassphrase, setUnsyncedKeyCount, setWalletKeyCount, setRelayConnected, setWallet } = useStore();
  const [showSync, setShowSync] = useState(false);
  const [showPassphrase, setShowPassphrase] = useState(false);
  const [hasEncryptedKeys, setHasEncryptedKeys] = useState(false);
  const relayRef = useRef<RelayClient | null>(null);

  const isInvestor = view === 'investor';

  // One-time migration from localStorage to IndexedDB + refresh key counts
  useEffect(() => {
    async function init() {
      await migrateFromLocalStorage();
      const allKeys = await getKeys();
      setWalletKeyCount(allKeys.length);
      const unsynced = await getUnsyncedKeys();
      setUnsyncedKeyCount(unsynced.length);

      // Check if any keys exist and whether they're encrypted
      if (allKeys.length > 0) {
        const anyEncrypted = allKeys.some(k => k.seedEncrypted);
        setHasEncryptedKeys(anyEncrypted);
        if (anyEncrypted && !walletPassphrase) {
          setShowPassphrase(true);
        }

        // Restore the most recent unspent key as the active key
        const unspentKeys = allKeys.filter(k => k.status === 'unspent' || k.status === 'unconfirmed');
        if (unspentKeys.length > 0) {
          const latest = unspentKeys[unspentKeys.length - 1];
          try {
            const seed = await getSeedHex(latest, walletPassphrase);
            setWallet('Wallet', latest.publicKey, seed);
          } catch {
            // Can't decrypt yet — passphrase prompt will handle it
            setWallet('Wallet', latest.publicKey, '');
          }
        }
      }
    }
    init();
  }, [setWalletKeyCount, setUnsyncedKeyCount, walletPassphrase]);

  // Connect RelayClient when paired devices exist
  useEffect(() => {
    let cancelled = false;
    async function connectRelay() {
      const peers = await getPeers();
      if (peers.length === 0 || cancelled) return;

      // Use first peer's relayKey (all peers in a wallet group share the same key)
      const relayKeyHex = peers[0].relayKey;

      // Disconnect previous client if any
      if (relayRef.current) {
        relayRef.current.disconnect();
      }

      const client = new RelayClient({
        relayUrl: relayUrl,
        relayKeyHex,
        getPassphrase: () => useStore.getState().walletPassphrase,
        onKeysReceived: async (imported, spentMarked, _fromDevice) => {
          if (imported > 0 || spentMarked > 0) {
            const allKeys = await getKeys();
            setWalletKeyCount(allKeys.length);
            const unsynced = await getUnsyncedKeys();
            setUnsyncedKeyCount(unsynced.length);
          }
        },
        onConnectionChange: (connected) => {
          setRelayConnected(connected);
        },
      });

      relayRef.current = client;
      client.connect();
    }
    connectRelay();
    return () => {
      cancelled = true;
      if (relayRef.current) {
        relayRef.current.disconnect();
        relayRef.current = null;
      }
    };
  }, [relayUrl, setWalletKeyCount, setUnsyncedKeyCount, setRelayConnected]);

  return (
    <div className="app-root">
      {showPassphrase && (
        <PassphrasePrompt
          isSetup={!hasEncryptedKeys}
          onComplete={() => setShowPassphrase(false)}
        />
      )}
      <Header onSyncClick={() => setShowSync(true)} />
      {error && (
        <div style={{ padding: '8px 16px', background: '#fee', color: '#c00', fontSize: 13 }}>
          {error}
        </div>
      )}
      {showSync && (
        <WalletSync onClose={async () => {
          setShowSync(false);
          const unsynced = await getUnsyncedKeys();
          setUnsyncedKeyCount(unsynced.length);
        }} />
      )}
      {isInvestor ? (
        <ViewContent />
      ) : (
        <div className="app-layout">
          <div className="app-sidebar">
            <ChainList />
            <Settings />
          </div>
          <div className="app-main">
            <ChainDetail />
            <ViewContent />
          </div>
        </div>
      )}
    </div>
  );
}

export default App;
