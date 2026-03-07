import { describe, it, expect } from 'vitest';
import { substituteSeparable, sha256Sync } from '../separable.ts';
import { bytesItem, vbcItem, containerItem, toBytes } from '../dataitem.ts';
import * as tc from '../typecodes.ts';
import { bytesToHex } from '../hex.ts';

describe('Separable substitution', () => {
  it('replaces separable NOTE with SHA256 hash', () => {
    const note = bytesItem(tc.NOTE, new TextEncoder().encode('Hello'));
    const noteEncoded = toBytes(note);
    const expectedHash = sha256Sync(noteEncoded);

    const result = substituteSeparable(note);
    expect(result.typeCode).toBe(tc.SHA256);
    expect(result.value.kind).toBe('bytes');
    if (result.value.kind === 'bytes') {
      expect(bytesToHex(result.value.data)).toBe(bytesToHex(expectedHash));
    }
  });

  it('leaves inseparable items unchanged', () => {
    const key = bytesItem(tc.ED25519_PUB, new Uint8Array(32).fill(0xAA));
    const result = substituteSeparable(key);
    expect(result).toEqual(key);
  });

  it('recurses into non-separable containers', () => {
    const assignment = containerItem(tc.ASSIGNMENT, [
      vbcItem(tc.LIST_SIZE, 1n),
      bytesItem(tc.NOTE, new TextEncoder().encode('test note')),
      containerItem(tc.PARTICIPANT, [
        bytesItem(tc.ED25519_PUB, new Uint8Array(32).fill(0xBB)),
      ]),
    ]);

    const result = substituteSeparable(assignment);
    if (result.value.kind !== 'container') throw new Error('expected container');
    const children = result.value.children;
    expect(children.length).toBe(3);
    expect(children[0].typeCode).toBe(tc.LIST_SIZE);
    expect(children[1].typeCode).toBe(tc.SHA256); // NOTE replaced
    expect(children[2].typeCode).toBe(tc.PARTICIPANT);
  });

  it('replaces entire separable container (VENDOR_PROFILE)', () => {
    const vp = containerItem(tc.VENDOR_PROFILE, [
      bytesItem(tc.NOTE, new TextEncoder().encode('name')),
    ]);
    const result = substituteSeparable(vp);
    expect(result.typeCode).toBe(tc.SHA256);
  });
});
