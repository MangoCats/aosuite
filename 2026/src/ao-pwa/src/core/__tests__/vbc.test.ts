import { describe, it, expect } from 'vitest';
import { encodeUnsigned, decodeUnsigned, encodeSigned, decodeSigned } from '../vbc.ts';
import { hexToBytes, bytesToHex } from '../hex.ts';
import vectors from '../../../../../specs/conformance/vectors.json';

describe('VBC unsigned', () => {
  for (const v of vectors.vbc_unsigned.vectors) {
    it(`encodes ${v.value} → ${v.hex}`, () => {
      const encoded = encodeUnsigned(BigInt(v.value));
      expect(bytesToHex(encoded)).toBe(v.hex);
    });

    it(`decodes ${v.hex} → ${v.value}`, () => {
      const [value, consumed] = decodeUnsigned(hexToBytes(v.hex), 0);
      expect(value).toBe(BigInt(v.value));
      expect(consumed).toBe(v.hex.length / 2);
    });
  }
});

describe('VBC signed', () => {
  for (const v of vectors.vbc_signed.vectors) {
    it(`encodes ${v.value} → ${v.hex}`, () => {
      const encoded = encodeSigned(BigInt(v.value));
      expect(bytesToHex(encoded)).toBe(v.hex);
    });

    it(`decodes ${v.hex} → ${v.value}`, () => {
      const [value, consumed] = decodeSigned(hexToBytes(v.hex), 0);
      expect(value).toBe(BigInt(v.value));
      expect(consumed).toBe(v.hex.length / 2);
    });
  }

  it('rejects negative zero (wire value 1)', () => {
    expect(() => decodeSigned(hexToBytes('01'), 0)).toThrow('negative zero');
  });
});

describe('VBC edge cases', () => {
  it('rejects empty input', () => {
    expect(() => decodeUnsigned(new Uint8Array(0), 0)).toThrow();
    expect(() => decodeSigned(new Uint8Array(0), 0)).toThrow();
  });

  it('rejects truncated continuation', () => {
    expect(() => decodeUnsigned(new Uint8Array([0x80]), 0)).toThrow();
  });

  it('decodes at offset', () => {
    const buf1 = encodeSigned(42n);
    const buf2 = encodeSigned(-99n);
    const combined = new Uint8Array([...buf1, ...buf2]);
    const [v1, c1] = decodeSigned(combined, 0);
    expect(v1).toBe(42n);
    const [v2] = decodeSigned(combined, c1);
    expect(v2).toBe(-99n);
  });
});
