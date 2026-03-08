import { useState, useEffect } from 'react';
import { RecorderClient } from '../api/client.ts';

interface BlobViewerProps {
  chainId: string;
  recorderUrl: string;
  hash: string;
}

export function BlobViewer({ chainId, recorderUrl, hash }: BlobViewerProps) {
  const [url, setUrl] = useState<string | null>(null);
  const [mime, setMime] = useState<string>('');
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let objectUrl: string | null = null;
    const client = new RecorderClient(recorderUrl);
    client.getBlob(chainId, hash)
      .then(({ mime: m, data }) => {
        if (cancelled) return;
        setMime(m);
        objectUrl = URL.createObjectURL(new Blob([data], { type: m }));
        setUrl(objectUrl);
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [chainId, recorderUrl, hash]);

  if (error) return <span style={{ color: '#c00', fontSize: 12 }}>{error}</span>;
  if (!url) return <span style={{ fontSize: 12, color: '#888' }}>Loading...</span>;

  if (mime.startsWith('image/')) {
    return <img src={url} alt="attachment" style={{ maxWidth: 200, maxHeight: 150, borderRadius: 4 }} />;
  }
  return <a href={url} download={`${hash.slice(0, 12)}.${mime.split('/')[1] || 'bin'}`} style={{ fontSize: 12 }}>Download ({mime})</a>;
}
