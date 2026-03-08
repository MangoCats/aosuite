// Assignment builder — port of ao-cli/src/assign.rs and accept.rs
// Builds ASSIGNMENT and AUTHORIZATION DataItems for the recorder.

import {
  containerItem, vbcItem, bytesItem, toJson,
  type DataItem, type DataItemJson,
} from './dataitem.ts';
import * as tc from './typecodes.ts';
import { encodeBigint, encodeRational, type Rational } from './bigint.ts';
import { fromUnixSeconds, timestampToBytes, nowUnixSeconds } from './timestamp.ts';
import { signDataItem, type SigningKey } from './sign.ts';
import { substituteSeparable } from './separable.ts';

export interface Giver {
  seqId: bigint;
  amount: bigint;
  key: SigningKey;
}

export interface Receiver {
  pubkey: Uint8Array;
  amount: bigint;
  key: SigningKey | null;  // null for external recipients (no signing key available)
}

export interface FeeRate {
  num: bigint;
  den: bigint;
}

export interface VendorProfileData {
  name?: string;
  description?: string;
  lat?: number;
  lon?: number;
}

/** Build a VENDOR_PROFILE DataItem from profile fields.
 *  Encodes as JSON in a NOTE child (all fields including lat/lon). */
export function buildVendorProfile(profile: VendorProfileData): DataItem {
  const enc = new TextEncoder();
  const json = JSON.stringify(profile);
  return containerItem(tc.VENDOR_PROFILE, [
    bytesItem(tc.NOTE, enc.encode(json)),
  ]);
}

/** Build an ASSIGNMENT DataItem from givers, receivers, and optional separable items. */
export function buildAssignment(
  givers: Giver[],
  receivers: Receiver[],
  feeRate: FeeRate,
  separableItems?: DataItem[],
): DataItem {
  const participantCount = givers.length + receivers.length;
  const children: DataItem[] = [vbcItem(tc.LIST_SIZE, BigInt(participantCount))];

  for (const giver of givers) {
    const amountBytes = encodeBigint(giver.amount);
    children.push(containerItem(tc.PARTICIPANT, [
      vbcItem(tc.SEQ_ID, giver.seqId),
      bytesItem(tc.AMOUNT, amountBytes),
    ]));
  }

  for (const receiver of receivers) {
    const amountBytes = encodeBigint(receiver.amount);
    children.push(containerItem(tc.PARTICIPANT, [
      bytesItem(tc.ED25519_PUB, receiver.pubkey),
      bytesItem(tc.AMOUNT, amountBytes),
    ]));
  }

  // Recording bid = fee rate
  const bid: Rational = { num: feeRate.num, den: feeRate.den };
  const bidBytes = encodeRational(bid);
  children.push(bytesItem(tc.RECORDING_BID, bidBytes));

  // Optional separable items (VENDOR_PROFILE, DATA_BLOB, etc.)
  if (separableItems) {
    for (const item of separableItems) children.push(item);
  }

  // Deadline: 1 day from now
  const deadlineSecs = nowUnixSeconds() + 86400n;
  const deadlineTs = fromUnixSeconds(deadlineSecs);
  children.push(bytesItem(tc.DEADLINE, timestampToBytes(deadlineTs)));

  return containerItem(tc.ASSIGNMENT, children);
}

/** Build a complete AUTHORIZATION DataItem with all signatures.
 *  Each participant signs the assignment with incrementing timestamps.
 *  If separableItems are present, they are included in the assignment for
 *  fee calculation, then substituteSeparable() replaces them with SHA256
 *  hashes before signing (pre-substitution fee, post-substitution signature). */
export async function buildAuthorization(
  givers: Giver[],
  receivers: Receiver[],
  feeRate: FeeRate,
  separableItems?: DataItem[],
): Promise<DataItem> {
  const assignment = buildAssignment(givers, receivers, feeRate, separableItems);

  // Apply separable substitution: blobs → SHA256 hashes.
  // Signatures are computed over the post-substitution assignment.
  const signableAssignment = substituteSeparable(assignment);

  const baseUnixSecs = nowUnixSeconds();
  const participantCount = givers.length + receivers.length;

  const authChildren: DataItem[] = [signableAssignment];

  // Giver signatures
  for (let i = 0; i < givers.length; i++) {
    const ts = fromUnixSeconds(baseUnixSecs + 1n + BigInt(i));
    const sig = await signDataItem(givers[i].key, signableAssignment, ts);
    authChildren.push(containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      vbcItem(tc.PAGE_INDEX, BigInt(i)),
    ]));
  }

  // Receiver signatures (skip external recipients without signing key)
  for (let j = 0; j < receivers.length; j++) {
    if (!receivers[j].key) continue;  // external recipient — no key to sign with
    const pageIdx = givers.length + j;
    const ts = fromUnixSeconds(
      baseUnixSecs + 1n + BigInt(participantCount) + BigInt(j),
    );
    const sig = await signDataItem(receivers[j].key, signableAssignment, ts);
    authChildren.push(containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      vbcItem(tc.PAGE_INDEX, BigInt(pageIdx)),
    ]));
  }

  return containerItem(tc.AUTHORIZATION, authChildren);
}

/** Build authorization and return as JSON for submission to recorder. */
export async function buildAuthorizationJson(
  givers: Giver[],
  receivers: Receiver[],
  feeRate: FeeRate,
  separableItems?: DataItem[],
): Promise<DataItemJson> {
  const auth = await buildAuthorization(givers, receivers, feeRate, separableItems);
  return toJson(auth);
}
