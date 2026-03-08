import { describe, it, expect } from 'vitest';
import { parseBlobPayload, buildBlobPayload } from '../blob.ts';

describe('blob utilities', () => {
  it('parseBlobPayload: valid image/jpeg', () => {
    const content = new Uint8Array([0xff, 0xd8, 0xff, 0xe0]);
    const payload = buildBlobPayload('image/jpeg', content);
    const result = parseBlobPayload(payload);
    expect(result).not.toBeNull();
    expect(result!.mime).toBe('image/jpeg');
    expect(result!.content).toEqual(content);
  });

  it('parseBlobPayload: no NUL returns null', () => {
    const data = new Uint8Array([0x61, 0x62, 0x63]); // "abc" with no NUL
    expect(parseBlobPayload(data)).toBeNull();
  });

  it('parseBlobPayload: empty MIME returns null', () => {
    const data = new Uint8Array([0x00, 0x01, 0x02]); // NUL at position 0
    expect(parseBlobPayload(data)).toBeNull();
  });

  it('buildBlobPayload: correct format', () => {
    const content = new Uint8Array([0xca, 0xfe]);
    const payload = buildBlobPayload('text/plain', content);

    // "text/plain" is 10 bytes, then NUL, then 2 bytes content
    expect(payload.length).toBe(10 + 1 + 2);
    expect(payload[10]).toBe(0); // NUL delimiter
    expect(payload[11]).toBe(0xca);
    expect(payload[12]).toBe(0xfe);

    const mimeStr = new TextDecoder().decode(payload.subarray(0, 10));
    expect(mimeStr).toBe('text/plain');
  });

  it('buildBlobPayload + parseBlobPayload roundtrip', () => {
    const mime = 'application/pdf';
    const content = new Uint8Array([0x25, 0x50, 0x44, 0x46]); // %PDF
    const payload = buildBlobPayload(mime, content);
    const parsed = parseBlobPayload(payload);
    expect(parsed).not.toBeNull();
    expect(parsed!.mime).toBe(mime);
    expect(parsed!.content).toEqual(content);
  });
});
