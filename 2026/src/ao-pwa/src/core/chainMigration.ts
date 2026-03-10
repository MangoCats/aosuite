// Chain migration — build CHAIN_MIGRATION DataItem and handle wallet behavior
// for migrated chains. TⒶ³ §7.
//
// Migration freezes the chain permanently. Three tiers:
// - Full-tier: owner-signed CHAIN_MIGRATION (cooperative)
// - Surrogate-tier: >50% share majority with SURROGATE_PROOF
// - Social-tier: no signatures, chains treated as independent

import {
  containerItem, bytesItem,
  type DataItem,
} from './dataitem.ts';
import * as tc from './typecodes.ts';
import { signDataItem, type SigningKey } from './sign.ts';
import { fromUnixSeconds, timestampToBytes, nowUnixSeconds } from './timestamp.ts';

/** Build a CHAIN_MIGRATION DataItem (full-tier, owner-signed).
 *  newChainId is the 32-byte SHA2-256 hash of the new chain's genesis. */
export function buildChainMigration(newChainId: Uint8Array): DataItem {
  return containerItem(tc.CHAIN_MIGRATION, [
    bytesItem(tc.CHAIN_REF, newChainId),
  ]);
}

/** Sign a chain migration with owner key. */
export async function signChainMigration(
  ownerKey: SigningKey,
  migration: DataItem,
): Promise<DataItem> {
  const ts = fromUnixSeconds(nowUnixSeconds());
  const sig = await signDataItem(ownerKey, migration, ts);
  return containerItem(tc.AUTHORIZATION, [
    migration,
    containerItem(tc.AUTH_SIG, [
      bytesItem(tc.ED25519_SIG, sig),
      bytesItem(tc.TIMESTAMP, timestampToBytes(ts)),
      bytesItem(tc.ED25519_PUB, ownerKey.publicKey),
    ]),
  ]);
}

/** Migration tier for display. */
export type MigrationTier = 'full' | 'surrogate' | 'social';

/** Chain migration status for wallet display. */
export interface MigrationStatus {
  frozen: boolean;
  tier?: MigrationTier;
}

/** Determine migration display from chain info. */
export function migrationStatus(frozen: boolean): MigrationStatus {
  if (!frozen) return { frozen: false };
  // When frozen, the tier depends on how it was frozen (from blocks),
  // but the API just reports frozen=true. Default to 'full' display.
  return { frozen: true, tier: 'full' };
}

/** Check if a chain should be treated as read-only (frozen). */
export function isReadOnly(frozen: boolean): boolean {
  return frozen;
}
