// Tests for pairing.ts — payload serialization, validation, and base64 helpers.
// X25519 crypto operations require browser Web Crypto; these tests cover
// the validation and serialization logic that runs in any environment.

import { describe, it, expect } from 'vitest';
import {
  serializePairPayload, deserializePairPayload,
  type PairInitPayload, type PairAckPayload,
} from '../pairing.ts';

describe('pairing payload serialization', () => {
  const validInit: PairInitPayload = {
    v: 1,
    type: 'pair',
    pub: 'a'.repeat(64),
    deviceId: 'dev123',
    label: 'My Phone',
  };

  const validAck: PairAckPayload = {
    v: 1,
    type: 'pair_ack',
    pub: 'b'.repeat(64),
    deviceId: 'dev456',
    label: 'My Desktop',
  };

  it('round-trips init payload', () => {
    const json = serializePairPayload(validInit);
    const parsed = deserializePairPayload(json);
    expect(parsed.type).toBe('pair');
    expect(parsed.pub).toBe(validInit.pub);
    expect(parsed.deviceId).toBe('dev123');
    expect(parsed.label).toBe('My Phone');
  });

  it('round-trips ack payload', () => {
    const json = serializePairPayload(validAck);
    const parsed = deserializePairPayload(json);
    expect(parsed.type).toBe('pair_ack');
    expect(parsed.pub).toBe(validAck.pub);
  });

  it('rejects wrong version', () => {
    const bad = JSON.stringify({ ...validInit, v: 2 });
    expect(() => deserializePairPayload(bad)).toThrow('Invalid pairing payload');
  });

  it('rejects unknown type', () => {
    const bad = JSON.stringify({ ...validInit, type: 'unknown' });
    expect(() => deserializePairPayload(bad)).toThrow('Invalid pairing payload');
  });

  it('rejects missing pub', () => {
    const { pub: _, ...noPub } = validInit;
    const bad = JSON.stringify(noPub);
    expect(() => deserializePairPayload(bad)).toThrow('Missing pairing fields');
  });

  it('rejects missing deviceId', () => {
    const { deviceId: _, ...noId } = validInit;
    const bad = JSON.stringify(noId);
    expect(() => deserializePairPayload(bad)).toThrow('Missing pairing fields');
  });

  it('rejects missing label', () => {
    const { label: _, ...noLabel } = validInit;
    const bad = JSON.stringify(noLabel);
    expect(() => deserializePairPayload(bad)).toThrow('Missing pairing fields');
  });

  it('rejects pub key with wrong length', () => {
    const bad = JSON.stringify({ ...validInit, pub: 'aa' });
    expect(() => deserializePairPayload(bad)).toThrow('Invalid public key');
  });

  it('rejects pub key with non-hex chars', () => {
    const bad = JSON.stringify({ ...validInit, pub: 'g'.repeat(64) });
    expect(() => deserializePairPayload(bad)).toThrow('Invalid public key');
  });

  it('rejects invalid JSON', () => {
    expect(() => deserializePairPayload('not json')).toThrow();
  });

  it('accepts uppercase hex in pub key', () => {
    const upper = JSON.stringify({ ...validInit, pub: 'A'.repeat(64) });
    const parsed = deserializePairPayload(upper);
    expect(parsed.pub).toBe('A'.repeat(64));
  });
});
