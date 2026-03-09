// Credential issuance UI — N28.
// Vendor attaches credential references (URL + SHA256) to their chain.
// Consumer views credentials via TrustIndicator (in ChainDetail).

import { useState, useEffect } from 'react';
import { RecorderClient } from '../api/client.ts';
import {
  fetchDocumentHash,
  parseCredentialRefs,
  deduplicateCredentials,
  type CredentialRefInfo,
} from '../core/credentialIssue.ts';
import { bytesToHex } from '../core/hex.ts';

interface Props {
  recorderUrl: string;
  chainId: string;
}

/** Vendor-facing component to add credential references to the chain. */
export function CredentialIssue({ recorderUrl, chainId }: Props) {
  const [url, setUrl] = useState('');
  const [status, setStatus] = useState('');
  const [loading, setLoading] = useState(false);
  const [credentials, setCredentials] = useState<CredentialRefInfo[]>([]);
  const [loadingCreds, setLoadingCreds] = useState(false);

  // Load existing credentials from chain blocks
  useEffect(() => {
    let cancelled = false;
    setLoadingCreds(true);

    async function load() {
      try {
        const client = new RecorderClient(recorderUrl);
        const blocks = await client.getBlocks(chainId);
        const refs = parseCredentialRefs(blocks);
        const deduped = deduplicateCredentials(refs);
        if (!cancelled) setCredentials(deduped);
      } catch {
        // Non-fatal — credentials are optional
      }
      if (!cancelled) setLoadingCreds(false);
    }

    load();
    return () => { cancelled = true; };
  }, [recorderUrl, chainId]);

  async function handleAddCredential() {
    if (!url.trim()) {
      setStatus('Enter a credential document URL');
      return;
    }

    // Basic URL validation
    try {
      new URL(url);
    } catch {
      setStatus('Invalid URL format');
      return;
    }

    setLoading(true);
    setStatus('Fetching document...');

    try {
      const hash = await fetchDocumentHash(url);

      setStatus(`Hash: ${bytesToHex(hash).slice(0, 16)}... — credential reference built. ` +
        'Include this in your next assignment to record it on-chain.');

      // Add to local display
      setCredentials(prev => deduplicateCredentials([
        ...prev,
        { url, contentHash: bytesToHex(hash) },
      ]));
      setUrl('');
    } catch (e) {
      setStatus(`Error: ${e instanceof Error ? e.message : e}`);
    }
    setLoading(false);
  }

  return (
    <div style={{ marginTop: 16, padding: 12, background: '#f9f9f9', borderRadius: 4 }}>
      <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 8 }}>Credentials</div>

      {/* Existing credentials */}
      {loadingCreds ? (
        <div style={{ fontSize: 12, color: '#666' }}>Loading credentials...</div>
      ) : credentials.length > 0 ? (
        <div style={{ marginBottom: 12 }}>
          {credentials.map(c => (
            <div key={c.url} style={{
              fontSize: 12, padding: '4px 0', borderBottom: '1px solid #eee',
              fontFamily: 'monospace', wordBreak: 'break-all',
            }}>
              <div style={{ color: '#444' }}>{c.url}</div>
              <div style={{ color: '#888', fontSize: 11 }}>
                SHA256: {c.contentHash.slice(0, 16)}...
                {c.blockHeight != null && ` (block ${c.blockHeight})`}
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div style={{ fontSize: 12, color: '#666', marginBottom: 8 }}>No credentials attached.</div>
      )}

      {/* Add credential form */}
      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <input
          value={url}
          onChange={e => setUrl(e.target.value)}
          placeholder="https://example.com/cert/food-safety.json"
          style={{ flex: 1, padding: '4px 6px', fontSize: 12 }}
          disabled={loading}
        />
        <button
          onClick={handleAddCredential}
          disabled={loading || !url.trim()}
          style={{ fontSize: 12, whiteSpace: 'nowrap' }}
        >
          {loading ? 'Fetching...' : 'Add Credential'}
        </button>
      </div>

      {status && (
        <div style={{
          marginTop: 8, fontSize: 11, padding: 6,
          background: status.startsWith('Error') ? '#fee' : '#f0f7ff',
          borderRadius: 4, wordBreak: 'break-all',
        }}>
          {status}
        </div>
      )}
    </div>
  );
}
