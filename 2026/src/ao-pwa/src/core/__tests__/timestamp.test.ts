import { describe, it, expect } from 'vitest';
import { fromUnixSeconds, timestampToBytes, timestampFromBytes, AO_MULTIPLIER } from '../timestamp.ts';
import { hexToBytes, bytesToHex } from '../hex.ts';
import vectors from '../../../../../specs/conformance/vectors.json';

describe('Timestamp', () => {
  for (const v of vectors.timestamp.vectors) {
    const unixSecs = BigInt(v.unix_seconds);
    const aoTs = BigInt(v.ao_timestamp);

    it(`${v.description}: unix ${v.unix_seconds} → ao ${v.ao_timestamp}`, () => {
      expect(fromUnixSeconds(unixSecs)).toBe(aoTs);
    });

    it(`${v.description}: encodes to ${v.hex}`, () => {
      const bytes = timestampToBytes(aoTs);
      expect(bytesToHex(bytes)).toBe(v.hex);
    });

    it(`${v.description}: decodes from ${v.hex}`, () => {
      const [decoded] = timestampFromBytes(hexToBytes(v.hex), 0);
      expect(decoded).toBe(aoTs);
    });
  }

  it('multiplier is 189000000', () => {
    expect(AO_MULTIPLIER).toBe(189000000n);
  });
});
