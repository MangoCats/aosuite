// Recorder switch flow — build RECORDER_CHANGE_PENDING, RECORDER_CHANGE,
// and RECORDER_URL_CHANGE DataItems for TⒶ³ recorder transitions.
//
// Flow: owner initiates PENDING → wait for CAA drain → recorder records CHANGE.
// URL change: dual-signed (owner + recorder) RECORDER_URL_CHANGE.

import {
  containerItem, bytesItem,
  type DataItem,
} from './dataitem.ts';
import * as tc from './typecodes.ts';
import { signDataItem, type SigningKey } from './sign.ts';
import { fromUnixSeconds, timestampToBytes, nowUnixSeconds } from './timestamp.ts';

/** Build a RECORDER_CHANGE_PENDING DataItem (owner-initiated). */
export function buildRecorderChangePending(
  newRecorderPubkey: Uint8Array,
  newRecorderUrl: string,
): DataItem {
  const urlBytes = new TextEncoder().encode(newRecorderUrl);
  return containerItem(tc.RECORDER_CHANGE_PENDING, [
    bytesItem(tc.ED25519_PUB, newRecorderPubkey),
    bytesItem(tc.RECORDER_URL, urlBytes),
  ]);
}

/** Build a RECORDER_URL_CHANGE DataItem (dual-signed by owner + recorder). */
export function buildRecorderUrlChange(newUrl: string): DataItem {
  const urlBytes = new TextEncoder().encode(newUrl);
  return containerItem(tc.RECORDER_URL_CHANGE, [
    bytesItem(tc.RECORDER_URL, urlBytes),
  ]);
}

/** Sign a recorder operation with owner key (single AUTH_SIG). */
export async function signRecorderOp(
  signerKey: SigningKey,
  operation: DataItem,
): Promise<DataItem> {
  const ts = fromUnixSeconds(nowUnixSeconds());
  const sig = await signDataItem(signerKey, operation, ts);
  return containerItem(tc.AUTHORIZATION, [
    operation,
    containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      bytesItem(tc.ED25519_PUB, signerKey.publicKey),
    ]),
  ]);
}

/** Dual-sign a recorder operation (owner + recorder). */
export async function dualSignRecorderOp(
  ownerKey: SigningKey,
  recorderKey: SigningKey,
  operation: DataItem,
): Promise<DataItem> {
  const baseTs = nowUnixSeconds();
  const ts1 = fromUnixSeconds(baseTs);
  const ts2 = fromUnixSeconds(baseTs + 1n);
  const sig1 = await signDataItem(ownerKey, operation, ts1);
  const sig2 = await signDataItem(recorderKey, operation, ts2);
  return containerItem(tc.AUTHORIZATION, [
    operation,
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

/** Recorder switch progress phases. */
export type RecorderSwitchPhase =
  | 'idle'
  | 'pending'        // RECORDER_CHANGE_PENDING recorded
  | 'draining'       // Waiting for active CAA escrows to resolve
  | 'ready'          // All CAAs drained, CHANGE can be submitted
  | 'completed'      // RECORDER_CHANGE recorded
  | 'failed';

/** Determine switch phase from chain info state. */
export function recorderSwitchPhase(
  hasPendingChange: boolean,
  activeCaaCount: number,
  frozen: boolean,
): RecorderSwitchPhase {
  if (frozen) return 'failed'; // chain is migrated, can't switch
  if (!hasPendingChange) return 'idle';
  if (activeCaaCount > 0) return 'draining';
  return 'ready';
}
