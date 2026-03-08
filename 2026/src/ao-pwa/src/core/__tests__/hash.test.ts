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

describe('SHA-256 subarray correctness', () => {
  it('hashes only the subarray content, not the full underlying buffer', async () => {
    // Create a larger buffer and take a subarray with non-zero byteOffset
    const full = new Uint8Array([0xff, 0xff, 0x61, 0x62, 0x63, 0xff, 0xff]);
    const sub = full.subarray(2, 5); // bytes for "abc"
    expect(sub.byteOffset).toBe(2); // confirm non-zero offset

    const abc = new TextEncoder().encode('abc');
    const expectedHash = await sha256(abc);
    const subHash = await sha256(sub);

    // Must match hash of "abc", NOT hash of the full 7-byte buffer
    expect(bytesToHex(subHash)).toBe(bytesToHex(expectedHash));
    // Known SHA-256("abc")
    expect(bytesToHex(subHash)).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
    );
  });
});
