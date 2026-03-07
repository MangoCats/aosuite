import { describe, it, expect } from 'vitest';
import { hexToBytes, bytesToHex, concatBytes } from '../hex.ts';

describe('hex utilities', () => {
  it('hexToBytes', () => {
    expect(hexToBytes('')).toEqual(new Uint8Array([]));
    expect(hexToBytes('00')).toEqual(new Uint8Array([0]));
    expect(hexToBytes('ff')).toEqual(new Uint8Array([255]));
    expect(hexToBytes('0102ff')).toEqual(new Uint8Array([1, 2, 255]));
  });

  it('bytesToHex', () => {
    expect(bytesToHex(new Uint8Array([]))).toBe('');
    expect(bytesToHex(new Uint8Array([0]))).toBe('00');
    expect(bytesToHex(new Uint8Array([1, 2, 255]))).toBe('0102ff');
  });

  it('round-trip', () => {
    const hex = '0123456789abcdef';
    expect(bytesToHex(hexToBytes(hex))).toBe(hex);
  });

  it('rejects odd-length hex', () => {
    expect(() => hexToBytes('abc')).toThrow('odd-length');
  });

  it('concatBytes', () => {
    const a = new Uint8Array([1, 2]);
    const b = new Uint8Array([3]);
    const c = new Uint8Array([4, 5, 6]);
    const result = concatBytes(a, b, c);
    expect(result).toEqual(new Uint8Array([1, 2, 3, 4, 5, 6]));
  });
});
