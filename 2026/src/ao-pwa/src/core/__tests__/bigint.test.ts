import { describe, it, expect } from 'vitest';
import { encodeBigint, decodeBigint, encodeRational, decodeRational } from '../bigint.ts';
import { hexToBytes, bytesToHex } from '../hex.ts';
import vectors from '../../../../../specs/conformance/vectors.json';

describe('BigInt encoding', () => {
  for (const v of vectors.bigint.vectors) {
    it(`encodes ${v.value} → ${v.full_hex}`, () => {
      const encoded = encodeBigint(BigInt(v.value));
      expect(bytesToHex(encoded)).toBe(v.full_hex);
    });

    it(`decodes ${v.full_hex} → ${v.value}`, () => {
      const [value, consumed] = decodeBigint(hexToBytes(v.full_hex), 0);
      expect(value).toBe(BigInt(v.value));
      expect(consumed).toBe(v.full_hex.length / 2);
    });
  }

  it('round-trips large values', () => {
    const values = [0n, 1n, -1n, 255n, -256n, 1000000n, -999999999999999999999999999n];
    for (const v of values) {
      const encoded = encodeBigint(v);
      const [decoded, consumed] = decodeBigint(encoded, 0);
      expect(decoded).toBe(v);
      expect(consumed).toBe(encoded.length);
    }
  });
});

describe('Rational encoding', () => {
  for (const v of vectors.rational.vectors) {
    it(`encodes ${v.numerator}/${v.denominator} → ${v.full_hex}`, () => {
      const encoded = encodeRational({
        num: BigInt(v.numerator),
        den: BigInt(v.denominator),
      });
      expect(bytesToHex(encoded)).toBe(v.full_hex);
    });

    it(`decodes ${v.full_hex} → ${v.numerator}/${v.denominator}`, () => {
      const [value, consumed] = decodeRational(hexToBytes(v.full_hex), 0);
      expect(value.num).toBe(BigInt(v.numerator));
      expect(value.den).toBe(BigInt(v.denominator));
      expect(consumed).toBe(v.full_hex.length / 2);
    });
  }
});
