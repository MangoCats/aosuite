import { describe, it, expect } from 'vitest';
import { recordingFee } from '../fees.ts';
import vectors from '../../../../../specs/conformance/vectors.json';

describe('Recording fee', () => {
  for (const v of vectors.fee_calculation.vectors) {
    it(v.description, () => {
      const fee = recordingFee(
        BigInt(v.data_bytes),
        BigInt(v.fee_rate_num),
        BigInt(v.fee_rate_den),
        BigInt(v.shares_out),
      );
      expect(fee.toString()).toBe(v.expected_fee);
    });

    it(`${v.description} — numerator`, () => {
      const numerator = BigInt(v.data_bytes) * BigInt(v.fee_rate_num) * BigInt(v.shares_out);
      expect(numerator.toString()).toBe(v.numerator);
    });
  }

  it('exact division does not round up', () => {
    // 10 / 5 = 2
    expect(recordingFee(10n, 1n, 5n, 1n)).toBe(2n);
  });

  it('non-exact division rounds up', () => {
    // 11 / 5 = 2.2 → ceil = 3
    expect(recordingFee(11n, 1n, 5n, 1n)).toBe(3n);
  });

  it('throws on negative numerator (regression: ceilDiv negative input)', () => {
    // A negative numerator would produce incorrect ceiling — must throw.
    // This happens if fee_rate_num * dataBytes * sharesOut overflows into negative
    // due to a bug, or if inputs are malformed.
    expect(() => recordingFee(-1n, 1n, 1n, 1n)).toThrow('negative numerator');
  });

  it('zero dataBytes yields zero fee', () => {
    expect(recordingFee(0n, 1n, 1000000n, 1000n)).toBe(0n);
  });
});
