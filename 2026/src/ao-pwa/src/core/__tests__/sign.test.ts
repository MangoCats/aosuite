import { describe, it, expect } from 'vitest';
import {
  signingKeyFromSeed, generateSigningKey,
  signRaw, verifyRaw, signDataItem, verifyDataItem,
} from '../sign.ts';
import { hexToBytes, bytesToHex } from '../hex.ts';
import { vbcItem, containerItem, bytesItem } from '../dataitem.ts';
import * as tc from '../typecodes.ts';
import { fromUnixSeconds } from '../timestamp.ts';
import vectors from '../../../../../specs/conformance/vectors.json';

describe('Ed25519 RFC 8032', () => {
  const v = vectors.ed25519.rfc8032_test1;

  it('derives correct public key from seed', async () => {
    const key = await signingKeyFromSeed(hexToBytes(v.private_seed_hex));
    expect(bytesToHex(key.publicKey)).toBe(v.public_key_hex);
  });

  it('signs empty message correctly', async () => {
    const key = await signingKeyFromSeed(hexToBytes(v.private_seed_hex));
    const sig = await signRaw(key, new Uint8Array(0));
    expect(bytesToHex(sig)).toBe(v.signature_hex);
  });

  it('verifies RFC 8032 test 1 signature', async () => {
    const valid = await verifyRaw(
      hexToBytes(v.public_key_hex),
      new Uint8Array(0),
      hexToBytes(v.signature_hex),
    );
    expect(valid).toBe(true);
  });
});

describe('AO signing pipeline', () => {
  it('sign and verify round-trip', async () => {
    const key = await generateSigningKey();
    const ts = fromUnixSeconds(1772611200n); // 2026-03-06

    const assignment = containerItem(tc.ASSIGNMENT, [
      vbcItem(tc.LIST_SIZE, 2n),
      containerItem(tc.PARTICIPANT, [
        vbcItem(tc.SEQ_ID, 1n),
        bytesItem(tc.AMOUNT, new Uint8Array([0x01, 0x00])),
      ]),
      containerItem(tc.PARTICIPANT, [
        bytesItem(tc.ED25519_PUB, new Uint8Array(32).fill(0xBB)),
        bytesItem(tc.AMOUNT, new Uint8Array([0x00, 0xFF])),
      ]),
    ]);

    const sig = await signDataItem(key, assignment, ts);
    expect(sig.length).toBe(64);

    const valid = await verifyDataItem(key.publicKey, assignment, ts, sig);
    expect(valid).toBe(true);
  });

  it('wrong key rejects', async () => {
    const key1 = await generateSigningKey();
    const key2 = await generateSigningKey();
    const ts = fromUnixSeconds(1000n);
    const item = vbcItem(tc.LIST_SIZE, 42n);

    const sig = await signDataItem(key1, item, ts);
    const valid = await verifyDataItem(key2.publicKey, item, ts, sig);
    expect(valid).toBe(false);
  });

  it('wrong timestamp rejects', async () => {
    const key = await generateSigningKey();
    const ts1 = fromUnixSeconds(1000n);
    const ts2 = fromUnixSeconds(1001n);
    const item = vbcItem(tc.LIST_SIZE, 42n);

    const sig = await signDataItem(key, item, ts1);
    const valid = await verifyDataItem(key.publicKey, item, ts2, sig);
    expect(valid).toBe(false);
  });

  it('modified data rejects', async () => {
    const key = await generateSigningKey();
    const ts = fromUnixSeconds(1000n);
    const item1 = vbcItem(tc.LIST_SIZE, 42n);
    const item2 = vbcItem(tc.LIST_SIZE, 43n);

    const sig = await signDataItem(key, item1, ts);
    const valid = await verifyDataItem(key.publicKey, item2, ts, sig);
    expect(valid).toBe(false);
  });
});
