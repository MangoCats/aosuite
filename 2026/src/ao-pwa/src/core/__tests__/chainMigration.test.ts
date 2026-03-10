import { describe, it, expect } from 'vitest';
import {
  buildChainMigration, signChainMigration,
  migrationStatus, isReadOnly,
} from '../chainMigration.ts';
import { generateSigningKey } from '../sign.ts';
import { children, findChild } from '../dataitem.ts';
import * as tc from '../typecodes.ts';

const fakeChainId = new Uint8Array(32).fill(0xab);

describe('buildChainMigration', () => {
  it('builds CHAIN_MIGRATION with CHAIN_REF child', () => {
    const migration = buildChainMigration(fakeChainId);
    expect(migration.typeCode).toBe(tc.CHAIN_MIGRATION);
    const ref = findChild(migration, tc.CHAIN_REF);
    expect(ref).toBeDefined();
    expect(ref!.value.kind).toBe('bytes');
  });
});

describe('signChainMigration', () => {
  it('wraps in AUTHORIZATION with single AUTH_SIG', async () => {
    const ownerKey = await generateSigningKey();
    const migration = buildChainMigration(fakeChainId);
    const auth = await signChainMigration(ownerKey, migration);

    expect(auth.typeCode).toBe(tc.AUTHORIZATION);
    const kids = children(auth);
    expect(kids[0].typeCode).toBe(tc.CHAIN_MIGRATION);
    expect(kids[1].typeCode).toBe(tc.AUTH_SIG);
  });
});

describe('migrationStatus', () => {
  it('returns frozen=false for active chain', () => {
    const status = migrationStatus(false);
    expect(status.frozen).toBe(false);
    expect(status.tier).toBeUndefined();
  });

  it('returns frozen=true with tier for migrated chain', () => {
    const status = migrationStatus(true);
    expect(status.frozen).toBe(true);
    expect(status.tier).toBe('full');
  });
});

describe('isReadOnly', () => {
  it('returns true for frozen chain', () => {
    expect(isReadOnly(true)).toBe(true);
  });

  it('returns false for active chain', () => {
    expect(isReadOnly(false)).toBe(false);
  });
});
