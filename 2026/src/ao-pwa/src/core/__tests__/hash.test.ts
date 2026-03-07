import { describe, it, expect } from 'vitest';
import { sha256 } from '../hash.ts';
import { sha256Sync } from '../separable.ts';
import { hexToBytes, bytesToHex } from '../hex.ts';
import vectors from '../../../../../specs/conformance/vectors.json';

describe('SHA-256 (Web Crypto)', () => {
  for (const v of vectors.sha256.vectors) {
    it(v.note ?? `hash of ${v.input_hex}`, async () => {
      const input = hexToBytes(v.input_hex);
      const hash = await sha256(input);
      expect(bytesToHex(hash)).toBe(v.output_hex);
    });
  }
});

describe('SHA-256 (sync pure-JS)', () => {
  for (const v of vectors.sha256.vectors) {
    it(v.note ?? `hash of ${v.input_hex}`, () => {
      const input = hexToBytes(v.input_hex);
      const hash = sha256Sync(input);
      expect(bytesToHex(hash)).toBe(v.output_hex);
    });
  }

  it('sync and async produce same result for "abc"', async () => {
    const input = new TextEncoder().encode('abc');
    const syncHash = sha256Sync(input);
    const asyncHash = await sha256(input);
    expect(bytesToHex(syncHash)).toBe(bytesToHex(asyncHash));
  });
});
