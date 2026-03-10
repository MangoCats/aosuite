// Owner key management — build OWNER_KEY_ROTATION, OWNER_KEY_REVOCATION,
// and OWNER_KEY_OVERRIDE DataItems for TⒶ³ key lifecycle.
//
// Key rotation: adds a new owner key with optional expiration on the old key.
// Key revocation: removes an owner key (requires signer != revoked key).
// Key override: emergency replacement (not yet implemented in ao-chain).

import {
  containerItem, bytesItem,
  type DataItem, type DataItemJson,
} from './dataitem.ts';
import * as tc from './typecodes.ts';
import { signDataItem, type SigningKey } from './sign.ts';
import { fromUnixSeconds, timestampToBytes, nowUnixSeconds } from './timestamp.ts';
import type { OwnerKeyInfo } from '../api/client.ts';

/** Build an OWNER_KEY_ROTATION DataItem.
 *  Adds newPubkey as owner key. Optional expiresInSecs sets expiration on signer's key. */
export function buildRotation(
  newPubkey: Uint8Array,
  expiresInSecs?: number,
): DataItem {
  const children: DataItem[] = [
    bytesItem(tc.ED25519_PUB, newPubkey),
  ];
  if (expiresInSecs !== undefined) {
    const expTs = fromUnixSeconds(nowUnixSeconds() + BigInt(expiresInSecs));
    children.push(bytesItem(tc.TIMESTAMP, timestampToBytes(expTs)));
  }
  return containerItem(tc.OWNER_KEY_ROTATION, children);
}

/** Build an OWNER_KEY_REVOCATION DataItem.
 *  Revokes the specified pubkey. Signer must be a different valid owner key. */
export function buildRevocation(revokedPubkey: Uint8Array): DataItem {
  return containerItem(tc.OWNER_KEY_REVOCATION, [
    bytesItem(tc.ED25519_PUB, revokedPubkey),
  ]);
}

/** Sign an owner key operation (rotation or revocation) with the AUTHORIZATION wrapper. */
export async function signOwnerKeyOp(
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

/** Classify an owner key's display status. */
export function keyStatus(
  key: OwnerKeyInfo,
  nowUnixSecs: number,
): 'valid' | 'expiring_soon' | 'expired' | 'revoked' | 'held' {
  if (key.status === 'revoked') return 'revoked';
  if (key.status === 'held') return 'held';
  if (key.expires_at != null) {
    // expires_at is AO timestamp (Unix seconds × 189_000_000)
    const expirySecs = key.expires_at / 189_000_000;
    if (expirySecs <= nowUnixSecs) return 'expired';
    // "Expiring soon" = within 24 hours
    if (expirySecs - nowUnixSecs < 86400) return 'expiring_soon';
  }
  return 'valid';
}

/** Check if a key rotation is rate-limited.
 *  Returns seconds until next rotation is allowed, or 0 if allowed now.
 *  keyRotationRate is a string (AO timestamp delta) to avoid JS number overflow. */
export function rotationCooldown(
  lastRotationTimestamp: number | null,
  keyRotationRate: string | number,
  nowUnixSecs: number,
): number {
  const rate = typeof keyRotationRate === 'string' ? Number(keyRotationRate) : keyRotationRate;
  if (lastRotationTimestamp == null || rate <= 0) return 0;
  // lastRotationTimestamp is AO timestamp
  const lastSecs = lastRotationTimestamp / 189_000_000;
  const rateSecs = rate / 189_000_000;
  const nextAllowed = lastSecs + rateSecs;
  return Math.max(0, Math.ceil(nextAllowed - nowUnixSecs));
}

/** Check if a key revocation is rate-limited.
 *  First revocation is free; subsequent ones require waiting revocationRateBase.
 *  With co-signer, the rate is halved (12h vs 24h default).
 *  revocationRateBase is a string (AO timestamp delta) to avoid JS number overflow. */
export function revocationCooldown(
  lastRevocationTimestamp: number | null,
  revocationRateBase: string | number,
  hasCosigner: boolean,
  nowUnixSecs: number,
): number {
  const base = typeof revocationRateBase === 'string' ? Number(revocationRateBase) : revocationRateBase;
  if (lastRevocationTimestamp == null) return 0; // first is free
  const lastSecs = lastRevocationTimestamp / 189_000_000;
  let rateSecs = base / 189_000_000;
  if (hasCosigner) rateSecs /= 2;
  const nextAllowed = lastSecs + rateSecs;
  return Math.max(0, Math.ceil(nextAllowed - nowUnixSecs));
}
