// Reward rate display and change proposal — TⒶ³ REWARD_RATE_CHANGE.
//
// Reward rate = share reward per recording cycle (num/den rational).
// Changes require dual signatures (owner + recorder) per spec §8.

import {
  containerItem, bytesItem,
  type DataItem,
} from './dataitem.ts';
import * as tc from './typecodes.ts';
import { encodeRational, type Rational } from './bigint.ts';
import { signDataItem, type SigningKey } from './sign.ts';
import { fromUnixSeconds, timestampToBytes, nowUnixSeconds } from './timestamp.ts';

/** Build a REWARD_RATE_CHANGE DataItem with new rate. */
export function buildRewardRateChange(newRate: Rational): DataItem {
  return containerItem(tc.REWARD_RATE_CHANGE, [
    bytesItem(tc.REWARD_RATE, encodeRational(newRate)),
  ]);
}

/** Dual-sign a reward rate change (owner + recorder). */
export async function signRewardRateChange(
  ownerKey: SigningKey,
  recorderKey: SigningKey,
  change: DataItem,
): Promise<DataItem> {
  const baseTs = nowUnixSeconds();
  const ts1 = fromUnixSeconds(baseTs);
  const ts2 = fromUnixSeconds(baseTs + 1n);
  const sig1 = await signDataItem(ownerKey, change, ts1);
  const sig2 = await signDataItem(recorderKey, change, ts2);
  return containerItem(tc.AUTHORIZATION, [
    change,
    containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig1),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts1)),
      bytesItem(tc.ED25519_PUB, ownerKey.publicKey),
    ]),
    containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig2),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts2)),
      bytesItem(tc.ED25519_PUB, recorderKey.publicKey),
    ]),
  ]);
}

/** Format a reward rate for display. */
export function formatRewardRate(num: string, den: string): string {
  const n = BigInt(num);
  const d = BigInt(den);
  if (n === 0n) return 'No reward';
  if (d === 1n) return `${n} shares/cycle`;
  // Display as percentage if den is 100 or 1000
  if (d === 100n) return `${n}%`;
  if (d === 1000n) {
    const pct = Number(n) / 10;
    return `${pct}%`;
  }
  return `${n}/${d}`;
}
