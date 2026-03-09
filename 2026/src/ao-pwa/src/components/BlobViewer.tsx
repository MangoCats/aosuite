import { useState, useEffect, useRef } from 'react';
import { RecorderClient, BlobPrunedError } from '../api/client.ts';

interface BlobViewerProps {
  chainId: string;
  recorderUrl: string;
  hash: string;
}

export function BlobViewer({ chainId, recorderUrl, hash }: BlobViewerProps) {
  const [url, setUrl] = useState<string | null>(null);
  const [mime, setMime] = useState<string>('');
  const [error, setError] = useState<string | null>(null);
  const [pruned, setPruned] = useState(false);
  const objectUrlRef = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    if (objectUrlRef.current) {
      URL.revokeObjectURL(objectUrlRef.current);
      objectUrlRef.current = null;
    }
    setUrl(null);
    setError(null);
    setPruned(false);
    const client = new RecorderClient(recorderUrl);
    client.getBlob(chainId, hash)
      .then(({ mime: m, data }) => {
        if (cancelled) return;
        setMime(m);
        const newUrl = URL.createObjectURL(new Blob([data], { type: m }));
        objectUrlRef.current = newUrl;
        setUrl(newUrl);
      })
      .catch((e) => {
        if (cancelled) return;
        if (e instanceof BlobPrunedError) {
          setPruned(true);
        } else {
          setError(String(e));
        }
      });
    return () => {
      cancelled = true;
      if (objectUrlRef.current) {
        URL.revokeObjectURL(objectUrlRef.current);
        objectUrlRef.current = null;
      }
    };
  }, [chainId, recorderUrl, hash]);

  if (pruned) {
    return (
      <span style={{ color: '#888', fontSize: 12, fontStyle: 'italic' }}>
        Attachment expired (pruned per retention policy)
      </span>
    );
  }
  if (error) return <span style={{ color: '#c00', fontSize: 12 }}>{error}</span>;
  if (!url) return <span style={{ fontSize: 12, color: '#888' }}>Loading...</span>;

  if (mime.startsWith('image/')) {
    return <img src={url} alt="attachment" style={{ maxWidth: 200, maxHeight: 150, borderRadius: 4 }} />;
  }
  return <a href={url} download={`${hash.slice(0, 12)}.${mime.split('/')[1] || 'bin'}`} style={{ fontSize: 12 }}>Download ({mime})</a>;
}
