import { useState, useRef } from 'react';
import type { AttachedBlob } from '../core/blob.ts';
import { buildBlobPayload, compressImage } from '../core/blob.ts';

interface AttachmentPickerProps {
  onAttach: (blobs: AttachedBlob[]) => void;
  attachments: AttachedBlob[];
  maxFiles?: number;
}

export function AttachmentPicker({ onAttach, attachments, maxFiles = 5 }: AttachmentPickerProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [processing, setProcessing] = useState(false);

  async function handleFiles(files: FileList | null) {
    if (!files || files.length === 0) return;
    setProcessing(true);
    try {
      const newBlobs: AttachedBlob[] = [];
      for (const file of Array.from(files)) {
        if (attachments.length + newBlobs.length >= maxFiles) break;

        let mime: string;
        let content: Uint8Array;

        if (file.type.startsWith('image/') && file.size > 1_048_576) {
          // Compress large images
          const compressed = await compressImage(file);
          mime = compressed.mime;
          content = compressed.data;
        } else {
          mime = file.type || 'application/octet-stream';
          const buf = await file.arrayBuffer();
          content = new Uint8Array(buf);
        }

        const payload = buildBlobPayload(mime, content);
        const previewUrl = file.type.startsWith('image/')
          ? URL.createObjectURL(new Blob([content], { type: mime }))
          : '';

        newBlobs.push({ file, mime, payload, previewUrl });
      }
      onAttach([...attachments, ...newBlobs]);
    } finally {
      setProcessing(false);
      if (inputRef.current) inputRef.current.value = '';
    }
  }

  function removeAttachment(index: number) {
    const updated = attachments.filter((_, i) => i !== index);
    if (attachments[index].previewUrl) {
      URL.revokeObjectURL(attachments[index].previewUrl);
    }
    onAttach(updated);
  }

  return (
    <div style={{ marginTop: 8 }}>
      <input
        ref={inputRef}
        type="file"
        accept="image/*,application/pdf"
        multiple
        onChange={(e) => handleFiles(e.target.files)}
        style={{ display: 'none' }}
      />
      <button
        type="button"
        onClick={() => inputRef.current?.click()}
        disabled={processing || attachments.length >= maxFiles}
        style={{ fontSize: 13 }}
      >
        {processing ? 'Processing...' : 'Attach File'}
      </button>
      {attachments.length > 0 && (
        <div style={{ display: 'flex', gap: 8, marginTop: 8, flexWrap: 'wrap' }}>
          {attachments.map((blob, i) => (
            <div key={i} style={{ position: 'relative', border: '1px solid #ccc', borderRadius: 4, padding: 4 }}>
              {blob.previewUrl ? (
                <img src={blob.previewUrl} alt={blob.file.name} style={{ width: 64, height: 64, objectFit: 'cover', borderRadius: 2 }} />
              ) : (
                <div style={{ width: 64, height: 64, display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 10, textAlign: 'center', background: '#f5f5f5' }}>
                  {blob.file.name.slice(0, 12)}
                </div>
              )}
              <button
                onClick={() => removeAttachment(i)}
                style={{ position: 'absolute', top: -6, right: -6, width: 18, height: 18, borderRadius: '50%', border: '1px solid #ccc', background: '#fff', cursor: 'pointer', fontSize: 10, lineHeight: '16px', padding: 0 }}
              >
                x
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
