import { describe, it, expect } from 'vitest';
import * as tc from '../typecodes.ts';

describe('Type codes', () => {
  const ALL_CODES = [
    tc.ED25519_PUB, tc.ED25519_SIG, tc.SHA256, tc.BLAKE3, tc.TIMESTAMP,
    tc.AMOUNT, tc.SEQ_ID, tc.ASSIGNMENT, tc.AUTHORIZATION, tc.PARTICIPANT,
    tc.BLOCK, tc.BLOCK_SIGNED, tc.BLOCK_CONTENTS, tc.PAGE, tc.GENESIS,
    tc.RECORDING_BID, tc.DEADLINE, tc.COIN_COUNT, tc.FEE_RATE, tc.EXPIRY_PERIOD,
    tc.CHAIN_SYMBOL, tc.PROTOCOL_VER, tc.SHARES_OUT, tc.PREV_HASH,
    tc.FIRST_SEQ, tc.SEQ_COUNT, tc.LIST_SIZE, tc.REFUTATION, tc.PAGE_INDEX,
    tc.AUTH_SIG, tc.EXPIRY_MODE, tc.TAX_PARAMS,
    tc.NOTE, tc.DATA_BLOB, tc.DESCRIPTION, tc.ICON, tc.VENDOR_PROFILE,
  ];

  it('all codes have size categories', () => {
    for (const code of ALL_CODES) {
      expect(tc.sizeCategory(code)).toBeDefined();
    }
  });

  it('all codes have type names', () => {
    for (const code of ALL_CODES) {
      expect(tc.typeName(code)).toBeDefined();
    }
  });

  it('separability: inseparable |code| 1-31', () => {
    expect(tc.isSeparable(1n)).toBe(false);
    expect(tc.isSeparable(8n)).toBe(false);
    expect(tc.isSeparable(31n)).toBe(false);
    expect(tc.isSeparable(-1n)).toBe(false);
    expect(tc.isSeparable(-2n)).toBe(false);
  });

  it('separability: separable |code| 32-63', () => {
    expect(tc.isSeparable(32n)).toBe(true);
    expect(tc.isSeparable(33n)).toBe(true);
    expect(tc.isSeparable(34n)).toBe(true);
    expect(tc.isSeparable(36n)).toBe(true);
    expect(tc.isSeparable(63n)).toBe(true);
  });

  it('separability: next inseparable band 64-95', () => {
    expect(tc.isSeparable(64n)).toBe(false);
    expect(tc.isSeparable(95n)).toBe(false);
  });

  it('separability: next separable band 96-127', () => {
    expect(tc.isSeparable(96n)).toBe(true);
    expect(tc.isSeparable(127n)).toBe(true);
  });
});
