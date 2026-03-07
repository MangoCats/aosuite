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

export interface Giver {
  seqId: bigint;
  amount: bigint;
  key: SigningKey;
}

export interface Receiver {
  pubkey: Uint8Array;
  amount: bigint;
  key: SigningKey;
}

export interface FeeRate {
  num: bigint;
  den: bigint;
}

/** Build an ASSIGNMENT DataItem from givers and receivers. */
export function buildAssignment(
  givers: Giver[],
  receivers: Receiver[],
  feeRate: FeeRate,
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

  // Deadline: 1 day from now
  const deadlineSecs = nowUnixSeconds() + 86400n;
  const deadlineTs = fromUnixSeconds(deadlineSecs);
  children.push(bytesItem(tc.DEADLINE, timestampToBytes(deadlineTs)));

  return containerItem(tc.ASSIGNMENT, children);
}

/** Build a complete AUTHORIZATION DataItem with all signatures.
 *  Each participant signs the assignment with incrementing timestamps. */
export async function buildAuthorization(
  givers: Giver[],
  receivers: Receiver[],
  feeRate: FeeRate,
): Promise<DataItem> {
  const assignment = buildAssignment(givers, receivers, feeRate);
  const baseUnixSecs = nowUnixSeconds();
  const participantCount = givers.length + receivers.length;

  const authChildren: DataItem[] = [assignment];

  // Giver signatures
  for (let i = 0; i < givers.length; i++) {
    const ts = fromUnixSeconds(baseUnixSecs + 1n + BigInt(i));
    const sig = await signDataItem(givers[i].key, assignment, ts);
    authChildren.push(containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      vbcItem(tc.PAGE_INDEX, BigInt(i)),
    ]));
  }

  // Receiver signatures
  for (let j = 0; j < receivers.length; j++) {
    const pageIdx = givers.length + j;
    const ts = fromUnixSeconds(
      baseUnixSecs + 1n + BigInt(participantCount) + BigInt(j),
    );
    const sig = await signDataItem(receivers[j].key, assignment, ts);
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
): Promise<DataItemJson> {
  const auth = await buildAuthorization(givers, receivers, feeRate);
  return toJson(auth);
}
