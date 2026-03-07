import { describe, it, expect } from 'vitest';
import {
  bytesItem, vbcItem, containerItem,
  toBytes, fromBytes, toJson, fromJson,
} from '../dataitem.ts';
import * as tc from '../typecodes.ts';
import { bytesToHex } from '../hex.ts';

describe('DataItem binary round-trip', () => {
  it('fixed-size (ED25519_PUB)', () => {
    const key = bytesItem(tc.ED25519_PUB, new Uint8Array(32).fill(0xAA));
    const encoded = toBytes(key);
    // type code 1 = signed VBC wire 2 = 0x02, then 32 raw bytes
    expect(encoded.length).toBe(1 + 32);
    const decoded = fromBytes(encoded);
    expect(decoded).toEqual(key);
  });

  it('variable-size (NOTE)', () => {
    const note = bytesItem(tc.NOTE, new TextEncoder().encode('Hello, AO!'));
    const encoded = toBytes(note);
    const decoded = fromBytes(encoded);
    expect(decoded).toEqual(note);
  });

  it('vbc-value (SEQ_ID)', () => {
    const seq = vbcItem(tc.SEQ_ID, 42n);
    const encoded = toBytes(seq);
    const decoded = fromBytes(encoded);
    expect(decoded).toEqual(seq);
  });

  it('container (PARTICIPANT)', () => {
    const participant = containerItem(tc.PARTICIPANT, [
      bytesItem(tc.ED25519_PUB, new Uint8Array(32).fill(0x11)),
      bytesItem(tc.AMOUNT, new Uint8Array([0x01, 0x00])),
    ]);
    const encoded = toBytes(participant);
    const decoded = fromBytes(encoded);
    expect(decoded).toEqual(participant);
  });

  it('nested container (ASSIGNMENT)', () => {
    const assignment = containerItem(tc.ASSIGNMENT, [
      vbcItem(tc.LIST_SIZE, 2n),
      containerItem(tc.PARTICIPANT, [
        vbcItem(tc.SEQ_ID, 1n),
        bytesItem(tc.AMOUNT, new Uint8Array([0x01, 0xFF])),
      ]),
      containerItem(tc.PARTICIPANT, [
        bytesItem(tc.ED25519_PUB, new Uint8Array(32).fill(0xBB)),
        bytesItem(tc.AMOUNT, new Uint8Array([0x01, 0xFE])),
      ]),
    ]);
    const encoded = toBytes(assignment);
    const decoded = fromBytes(encoded);
    expect(decoded).toEqual(assignment);
  });

  it('negative type code (EXPIRY_MODE)', () => {
    const mode = vbcItem(tc.EXPIRY_MODE, 1n);
    const encoded = toBytes(mode);
    expect(encoded[0]).toBe(0x03); // -1 → wire 3
    const decoded = fromBytes(encoded);
    expect(decoded).toEqual(mode);
  });

  it('empty container', () => {
    const empty = containerItem(tc.TAX_PARAMS, []);
    const encoded = toBytes(empty);
    const decoded = fromBytes(encoded);
    expect(decoded).toEqual(empty);
  });
});

describe('DataItem JSON round-trip', () => {
  it('bytes item', () => {
    const key = bytesItem(tc.ED25519_PUB, new Uint8Array(32).fill(0xAA));
    const json = toJson(key);
    expect(json.type).toBe('ED25519_PUB');
    expect(json.code).toBe(1);
    expect((json.value as string).length).toBe(64);
    const decoded = fromJson(json);
    expect(decoded).toEqual(key);
  });

  it('vbc item', () => {
    const seq = vbcItem(tc.SEQ_ID, 42n);
    const json = toJson(seq);
    expect(json.value).toBe(42);
    const decoded = fromJson(json);
    expect(decoded).toEqual(seq);
  });

  it('container item', () => {
    const p = containerItem(tc.PARTICIPANT, [
      vbcItem(tc.SEQ_ID, 1n),
      bytesItem(tc.AMOUNT, new Uint8Array([0x01, 0xFF])),
    ]);
    const json = toJson(p);
    expect(json.items?.length).toBe(2);
    const decoded = fromJson(json);
    expect(decoded).toEqual(p);
  });

  it('JSON → binary → JSON consistency', () => {
    const original = containerItem(tc.ASSIGNMENT, [
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

    const binary1 = toBytes(original);
    const fromBin = fromBytes(binary1);
    expect(fromBin).toEqual(original);

    const json = toJson(original);
    const fromJ = fromJson(json);
    expect(fromJ).toEqual(original);

    const binary2 = toBytes(fromJ);
    expect(bytesToHex(binary2)).toBe(bytesToHex(binary1));
  });
});
