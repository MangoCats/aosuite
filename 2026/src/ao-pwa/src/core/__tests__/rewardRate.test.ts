import { describe, it, expect } from 'vitest';
import {
  buildRewardRateChange, signRewardRateChange, formatRewardRate,
} from '../rewardRate.ts';
import { generateSigningKey } from '../sign.ts';
import { children, findChild } from '../dataitem.ts';
import * as tc from '../typecodes.ts';

describe('buildRewardRateChange', () => {
  it('builds REWARD_RATE_CHANGE with encoded rate', () => {
    const change = buildRewardRateChange({ num: 5n, den: 100n });
    expect(change.typeCode).toBe(tc.REWARD_RATE_CHANGE);
    const rate = findChild(change, tc.REWARD_RATE);
    expect(rate).toBeDefined();
    expect(rate!.value.kind).toBe('bytes');
  });
});

describe('signRewardRateChange', () => {
  it('dual-signs with owner + recorder', async () => {
    const ownerKey = await generateSigningKey();
    const recorderKey = await generateSigningKey();
    const change = buildRewardRateChange({ num: 3n, den: 100n });
    const auth = await signRewardRateChange(ownerKey, recorderKey, change);

    expect(auth.typeCode).toBe(tc.AUTHORIZATION);
    const kids = children(auth);
    expect(kids[0].typeCode).toBe(tc.REWARD_RATE_CHANGE);
    expect(kids[1].typeCode).toBe(tc.AUTH_SIG);
    expect(kids[2].typeCode).toBe(tc.AUTH_SIG);
  });
});

describe('formatRewardRate', () => {
  it('shows "No reward" for zero rate', () => {
    expect(formatRewardRate('0', '1')).toBe('No reward');
  });

  it('shows shares/cycle for den=1', () => {
    expect(formatRewardRate('10', '1')).toBe('10 shares/cycle');
  });

  it('shows percentage for den=100', () => {
    expect(formatRewardRate('5', '100')).toBe('5%');
  });

  it('shows fractional percentage for den=1000', () => {
    expect(formatRewardRate('25', '1000')).toBe('2.5%');
  });

  it('shows fraction for other denominators', () => {
    expect(formatRewardRate('3', '7')).toBe('3/7');
  });
});
