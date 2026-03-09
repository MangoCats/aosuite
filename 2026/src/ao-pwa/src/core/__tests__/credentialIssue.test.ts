import { describe, it, expect, vi, beforeEach } from 'vitest';
import {
  buildCredentialRef,
  verifyCredential,
  parseCredentialRefs,
  deduplicateCredentials,
  type CredentialRefInfo,
} from '../credentialIssue.ts';
import { children, findChild, asBytes } from '../dataitem.ts';
import * as tc from '../typecodes.ts';
import { bytesToHex, hexToBytes } from '../hex.ts';
import { sha256 } from '../hash.ts';

/** Encode a UTF-8 string to hex (matching JSON wire format for variable-size bytes). */
function utf8ToHex(s: string): string {
  return bytesToHex(new TextEncoder().encode(s));
}

describe('buildCredentialRef', () => {
  it('builds a CREDENTIAL_REF container with URL and SHA256', () => {
    const hash = new Uint8Array(32);
    hash[0] = 0xab;
    hash[31] = 0xcd;

    const ref = buildCredentialRef('https://example.com/cert.json', hash);

    expect(ref.typeCode).toBe(tc.CREDENTIAL_REF);
    expect(ref.value.kind).toBe('container');

    const urlChild = findChild(ref, tc.CREDENTIAL_URL);
    expect(urlChild).toBeDefined();
    const urlBytes = asBytes(urlChild!);
    expect(new TextDecoder().decode(urlBytes!)).toBe('https://example.com/cert.json');

    const hashChild = findChild(ref, tc.SHA256);
    expect(hashChild).toBeDefined();
    const hashBytes = asBytes(hashChild!);
    expect(hashBytes!.length).toBe(32);
    expect(hashBytes![0]).toBe(0xab);
    expect(hashBytes![31]).toBe(0xcd);
  });

  it('rejects non-32-byte hash', () => {
    expect(() => buildCredentialRef('https://x.com', new Uint8Array(16)))
      .toThrow('32 bytes');
  });
});

describe('verifyCredential', () => {
  const mockFetch = vi.fn();
  beforeEach(() => {
    mockFetch.mockReset();
    vi.stubGlobal('fetch', mockFetch);
  });

  it('returns verified when hash matches', async () => {
    const docBytes = new TextEncoder().encode('{"type":"food-safety"}');
    const hash = await sha256(docBytes);

    mockFetch.mockResolvedValueOnce({
      ok: true,
      arrayBuffer: () => Promise.resolve(docBytes.buffer),
    });

    const result = await verifyCredential({
      url: 'https://example.com/cert.json',
      contentHash: bytesToHex(hash),
    });

    expect(result).toBe('verified');
  });

  it('returns mismatch when hash differs', async () => {
    const docBytes = new TextEncoder().encode('{"type":"food-safety"}');
    const wrongHash = new Uint8Array(32); // all zeros

    mockFetch.mockResolvedValueOnce({
      ok: true,
      arrayBuffer: () => Promise.resolve(docBytes.buffer),
    });

    const result = await verifyCredential({
      url: 'https://example.com/cert.json',
      contentHash: bytesToHex(wrongHash),
    });

    expect(result).toBe('mismatch');
  });

  it('returns unreachable on network error', async () => {
    mockFetch.mockRejectedValueOnce(new Error('network error'));

    const result = await verifyCredential({
      url: 'https://example.com/cert.json',
      contentHash: '00'.repeat(32),
    });

    expect(result).toBe('unreachable');
  });

  it('returns unreachable on HTTP error', async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 404,
    });

    const result = await verifyCredential({
      url: 'https://example.com/gone.json',
      contentHash: '00'.repeat(32),
    });

    expect(result).toBe('unreachable');
  });
});

describe('parseCredentialRefs', () => {
  it('extracts CREDENTIAL_REF from block JSON', () => {
    const blocks = [{
      type: 'BLOCK',
      code: Number(tc.BLOCK),
      items: [{
        type: 'BLOCK_SIGNED',
        code: Number(tc.BLOCK_SIGNED),
        items: [{
          type: 'BLOCK_CONTENTS',
          code: Number(tc.BLOCK_CONTENTS),
          items: [
            { type: 'FIRST_SEQ', code: Number(tc.FIRST_SEQ), value: 5 },
            {
              type: 'PAGE',
              code: Number(tc.PAGE),
              items: [{
                type: 'CREDENTIAL_REF',
                code: Number(tc.CREDENTIAL_REF),
                items: [
                  { type: 'CREDENTIAL_URL', code: Number(tc.CREDENTIAL_URL), value: utf8ToHex('https://x.com/cert') },
                  { type: 'SHA256', code: Number(tc.SHA256), value: 'ab'.repeat(32) },
                ],
              }],
            },
          ],
        }],
      }],
    }];

    const refs = parseCredentialRefs(blocks);
    expect(refs).toHaveLength(1);
    expect(refs[0].url).toBe('https://x.com/cert');
    expect(refs[0].contentHash).toBe('ab'.repeat(32));
    expect(refs[0].blockHeight).toBe(5);
  });

  it('returns empty for blocks without credentials', () => {
    const blocks = [{
      type: 'BLOCK',
      code: Number(tc.BLOCK),
      items: [{
        type: 'BLOCK_SIGNED',
        code: Number(tc.BLOCK_SIGNED),
        items: [{
          type: 'BLOCK_CONTENTS',
          code: Number(tc.BLOCK_CONTENTS),
          items: [
            { type: 'FIRST_SEQ', code: Number(tc.FIRST_SEQ), value: 0 },
          ],
        }],
      }],
    }];

    const refs = parseCredentialRefs(blocks);
    expect(refs).toHaveLength(0);
  });
});

describe('deduplicateCredentials', () => {
  it('keeps latest block height per URL', () => {
    const refs: CredentialRefInfo[] = [
      { url: 'https://x.com/cert', contentHash: 'aa'.repeat(32), blockHeight: 1 },
      { url: 'https://x.com/cert', contentHash: 'bb'.repeat(32), blockHeight: 5 },
      { url: 'https://y.com/cert', contentHash: 'cc'.repeat(32), blockHeight: 3 },
    ];

    const result = deduplicateCredentials(refs);
    expect(result).toHaveLength(2);

    const x = result.find(r => r.url === 'https://x.com/cert');
    expect(x!.contentHash).toBe('bb'.repeat(32));
    expect(x!.blockHeight).toBe(5);
  });

  it('handles entries without blockHeight', () => {
    const refs: CredentialRefInfo[] = [
      { url: 'https://x.com/cert', contentHash: 'aa'.repeat(32) },
      { url: 'https://x.com/cert', contentHash: 'bb'.repeat(32) },
    ];

    const result = deduplicateCredentials(refs);
    expect(result).toHaveLength(1);
  });
});
