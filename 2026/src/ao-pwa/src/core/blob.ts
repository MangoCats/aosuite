/** Blob utilities — parse, build, and compress binary attachments with MIME metadata. */

/** Parse a DATA_BLOB payload: [MIME UTF-8 NUL-terminated][raw content] */
export function parseBlobPayload(data: Uint8Array): { mime: string; content: Uint8Array } | null {
  const nulIndex = data.indexOf(0);
  if (nulIndex < 1) return null;
  const mime = new TextDecoder().decode(data.subarray(0, nulIndex));
  const content = data.subarray(nulIndex + 1);
  return { mime, content };
}

/** Validate a MIME type string: must be non-empty, contain '/', and have no control characters. */
function validateMime(mime: string): void {
  if (!mime) throw new Error('MIME type must not be empty');
  if (!mime.includes('/')) throw new Error(`MIME type must contain '/': ${mime}`);
  // eslint-disable-next-line no-control-regex
  if (/[\x00-\x1f\x7f]/.test(mime)) throw new Error(`MIME type contains control characters: ${JSON.stringify(mime)}`);
}

/** Build a DATA_BLOB payload from MIME type and content */
export function buildBlobPayload(mime: string, content: Uint8Array): Uint8Array {
  validateMime(mime);
  const mimeBytes = new TextEncoder().encode(mime);
  const result = new Uint8Array(mimeBytes.length + 1 + content.length);
  result.set(mimeBytes, 0);
  result[mimeBytes.length] = 0; // NUL delimiter
  result.set(content, mimeBytes.length + 1);
  return result;
}

/** Compress an image file to max dimensions, returning WebP (or JPEG fallback) */
export async function compressImage(file: File, maxDim = 2048, quality = 0.85): Promise<{ data: Uint8Array; mime: string }> {
  const bitmap = await createImageBitmap(file);
  const { width, height } = bitmap;

  let targetW = width, targetH = height;
  if (width > maxDim || height > maxDim) {
    const scale = maxDim / Math.max(width, height);
    targetW = Math.round(width * scale);
    targetH = Math.round(height * scale);
  }

  const canvas = new OffscreenCanvas(targetW, targetH);
  const ctx = canvas.getContext('2d')!;
  ctx.drawImage(bitmap, 0, 0, targetW, targetH);
  bitmap.close();

  // Try WebP first, fallback to JPEG
  let blob = await canvas.convertToBlob({ type: 'image/webp', quality });
  let mime = 'image/webp';
  if (blob.size === 0) {
    blob = await canvas.convertToBlob({ type: 'image/jpeg', quality });
    mime = 'image/jpeg';
  }

  const arrayBuf = await blob.arrayBuffer();
  return { data: new Uint8Array(arrayBuf), mime };
}

export interface AttachedBlob {
  file: File;
  mime: string;
  payload: Uint8Array;  // full MIME+NUL+content
  previewUrl: string;   // URL.createObjectURL for display
}
