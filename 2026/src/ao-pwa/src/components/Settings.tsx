import { useState } from 'react';
import { useStore } from '../store/useStore.ts';

export function Settings() {
  const { recorderUrl, setRecorderUrl } = useStore();
  const [url, setUrl] = useState(recorderUrl);

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
    </div>
  );
}
