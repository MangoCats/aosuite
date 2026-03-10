import { describe, it, expect } from 'vitest';
import {
  buildRotation, buildRevocation, signOwnerKeyOp,
  keyStatus, rotationCooldown, revocationCooldown,
} from '../ownerKeys.ts';
import { generateSigningKey } from '../sign.ts';
import { children, findChild } from '../dataitem.ts';
import * as tc from '../typecodes.ts';
import type { OwnerKeyInfo } from '../../api/client.ts';

describe('buildRotation', () => {
  it('builds OWNER_KEY_ROTATION with new pubkey', async () => {
    const newKey = await generateSigningKey();
    const rotation = buildRotation(newKey.publicKey);
    expect(rotation.typeCode).toBe(tc.OWNER_KEY_ROTATION);
    const pub = findChild(rotation, tc.ED25519_PUB);
    expect(pub).toBeDefined();
    expect(pub!.value.kind).toBe('bytes');
    // No timestamp if no expiration
    expect(findChild(rotation, tc.TIMESTAMP)).toBeUndefined();
  });

  it('includes expiration timestamp when specified', async () => {
    const newKey = await generateSigningKey();
    const rotation = buildRotation(newKey.publicKey, 86400);
    const ts = findChild(rotation, tc.TIMESTAMP);
    expect(ts).toBeDefined();
    expect(ts!.value.kind).toBe('bytes');
  });
});

describe('buildRevocation', () => {
  it('builds OWNER_KEY_REVOCATION with target pubkey', async () => {
    const key = await generateSigningKey();
    const revocation = buildRevocation(key.publicKey);
    expect(revocation.typeCode).toBe(tc.OWNER_KEY_REVOCATION);
    const pub = findChild(revocation, tc.ED25519_PUB);
    expect(pub).toBeDefined();
  });
});

describe('signOwnerKeyOp', () => {
  it('wraps operation in AUTHORIZATION with AUTH_SIG', async () => {
    const signerKey = await generateSigningKey();
    const newKey = await generateSigningKey();
    const rotation = buildRotation(newKey.publicKey);
    const auth = await signOwnerKeyOp(signerKey, rotation);

    expect(auth.typeCode).toBe(tc.AUTHORIZATION);
    const kids = children(auth);
    // First child is the operation
    expect(kids[0].typeCode).toBe(tc.OWNER_KEY_ROTATION);
    // Second child is AUTH_SIG
    expect(kids[1].typeCode).toBe(tc.AUTH_SIG);
    const sig = findChild(kids[1], tc.ED25519_SIG);
    expect(sig).toBeDefined();
  });
});

describe('keyStatus', () => {
  const AO_SCALE = 189_000_000;
  const now = 1700000000; // Unix seconds

  function makeKey(overrides: Partial<OwnerKeyInfo> = {}): OwnerKeyInfo {
    return {
      pubkey: '00'.repeat(32),
      added_height: 0,
      added_timestamp: 0,
      status: 'valid',
      ...overrides,
    };
  }

  it('returns "valid" for active key with no expiration', () => {
    expect(keyStatus(makeKey(), now)).toBe('valid');
  });

  it('returns "revoked" for revoked key', () => {
    expect(keyStatus(makeKey({ status: 'revoked' }), now)).toBe('revoked');
  });

  it('returns "held" for held key', () => {
    expect(keyStatus(makeKey({ status: 'held' }), now)).toBe('held');
  });

  it('returns "expired" for key past expiration', () => {
    const expiresAt = (now - 3600) * AO_SCALE; // expired 1 hour ago
    expect(keyStatus(makeKey({ expires_at: expiresAt }), now)).toBe('expired');
  });

  it('returns "expiring_soon" for key expiring within 24 hours', () => {
    const expiresAt = (now + 3600) * AO_SCALE; // expires in 1 hour
    expect(keyStatus(makeKey({ expires_at: expiresAt }), now)).toBe('expiring_soon');
  });

  it('returns "valid" for key expiring more than 24 hours out', () => {
    const expiresAt = (now + 100000) * AO_SCALE; // expires in ~27 hours
    expect(keyStatus(makeKey({ expires_at: expiresAt }), now)).toBe('valid');
  });
});

describe('rotationCooldown', () => {
  const AO_SCALE = 189_000_000;

  it('returns 0 when no previous rotation', () => {
    expect(rotationCooldown(null, 24 * 3600 * AO_SCALE, 1700000000)).toBe(0);
  });

  it('returns 0 when rate is 0 (pre-live)', () => {
    const lastTs = 1700000000 * AO_SCALE;
    expect(rotationCooldown(lastTs, 0, 1700000001)).toBe(0);
  });

  it('returns remaining seconds when rate-limited', () => {
    const lastTs = 1700000000 * AO_SCALE;
    const rate = 3600 * AO_SCALE; // 1 hour rate
    const now = 1700000000 + 1800; // 30 min later
    const cooldown = rotationCooldown(lastTs, rate, now);
    expect(cooldown).toBe(1800); // 30 min remaining
  });

  it('returns 0 when cooldown has elapsed', () => {
    const lastTs = 1700000000 * AO_SCALE;
    const rate = 3600 * AO_SCALE; // 1 hour rate
    const now = 1700000000 + 7200; // 2 hours later
    expect(rotationCooldown(lastTs, rate, now)).toBe(0);
  });
});

describe('revocationCooldown', () => {
  const AO_SCALE = 189_000_000;

  it('returns 0 for first revocation (no previous)', () => {
    expect(revocationCooldown(null, 24 * 3600 * AO_SCALE, false, 1700000000)).toBe(0);
  });

  it('returns full rate for single-signer subsequent revocation', () => {
    const lastTs = 1700000000 * AO_SCALE;
    const rate = 86400 * AO_SCALE; // 24 hours
    const now = 1700000000 + 43200; // 12 hours later
    const cooldown = revocationCooldown(lastTs, rate, false, now);
    expect(cooldown).toBe(43200); // 12 hours remaining
  });

  it('returns half rate for co-signed revocation', () => {
    const lastTs = 1700000000 * AO_SCALE;
    const rate = 86400 * AO_SCALE; // 24 hours base, halved = 12 hours
    const now = 1700000000 + 43200; // 12 hours later
    const cooldown = revocationCooldown(lastTs, rate, true, now);
    expect(cooldown).toBe(0); // half rate = 12h, so 12h later is exactly 0
  });
});
