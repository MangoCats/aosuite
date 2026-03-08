import { describe, it, expect } from 'vitest';
import { buildAssignment } from '../assignment.ts';
import { substituteSeparable, sha256Sync } from '../separable.ts';
import { bytesItem, toBytes, containerItem } from '../dataitem.ts';
import * as tc from '../typecodes.ts';
import { recordingFee } from '../fees.ts';
import type { Giver, Receiver, FeeRate } from '../assignment.ts';
import { generateSigningKey } from '../sign.ts';

// Helpers
function makeGiver(seqId: bigint, amount: bigint, key: Awaited<ReturnType<typeof generateSigningKey>>): Giver {
  return { seqId, amount, key };
}

function makeReceiver(key: Awaited<ReturnType<typeof generateSigningKey>>, amount: bigint): Receiver {
  return { pubkey: key.publicKey, amount, key };
}

const feeRate: FeeRate = { num: 1n, den: 1000000n };

describe('Blob on-chain linking + pre-substitution fees', () => {
  it('buildAssignment includes DATA_BLOB children when passed as separableItems', async () => {
    const giverKey = await generateSigningKey();
    const recvKey = await generateSigningKey();
    const givers = [makeGiver(0n, 1000n, giverKey)];
    const receivers = [makeReceiver(recvKey, 990n)];

    const blobPayload = new Uint8Array([
      ...new TextEncoder().encode('image/png\0'),
      0x89, 0x50, 0x4e, 0x47, // fake PNG header
    ]);
    const blobItem = bytesItem(tc.DATA_BLOB, blobPayload);

    const assignment = buildAssignment(givers, receivers, feeRate, [blobItem]);

    // Assignment should contain the DATA_BLOB child
    if (assignment.value.kind !== 'container') throw new Error('expected container');
    const blobChildren = assignment.value.children.filter(c => c.typeCode === tc.DATA_BLOB);
    expect(blobChildren.length).toBe(1);
    if (blobChildren[0].value.kind !== 'bytes') throw new Error('expected bytes');
    expect(blobChildren[0].value.data).toEqual(blobPayload);
  });

  it('pre-sub assignment is larger than post-sub assignment', async () => {
    const giverKey = await generateSigningKey();
    const recvKey = await generateSigningKey();
    const givers = [makeGiver(0n, 1000n, giverKey)];
    const receivers = [makeReceiver(recvKey, 990n)];

    // 1KB blob
    const content = new Uint8Array(1024).fill(0xAA);
    const blobPayload = new Uint8Array([
      ...new TextEncoder().encode('image/png\0'),
      ...content,
    ]);
    const blobItem = bytesItem(tc.DATA_BLOB, blobPayload);

    const preSubAssignment = buildAssignment(givers, receivers, feeRate, [blobItem]);
    const postSubAssignment = substituteSeparable(preSubAssignment);

    const preSubBytes = toBytes(preSubAssignment).length;
    const postSubBytes = toBytes(postSubAssignment).length;

    // Pre-sub should be ~1KB larger (blob content vs 33-byte hash)
    expect(preSubBytes).toBeGreaterThan(postSubBytes + 900);
  });

  it('fee computed on pre-sub size is higher than on post-sub size', async () => {
    const giverKey = await generateSigningKey();
    const recvKey = await generateSigningKey();
    const givers = [makeGiver(0n, 1000n, giverKey)];
    const receivers = [makeReceiver(recvKey, 990n)];

    const content = new Uint8Array(2048).fill(0xBB);
    const blobPayload = new Uint8Array([
      ...new TextEncoder().encode('image/png\0'),
      ...content,
    ]);
    const blobItem = bytesItem(tc.DATA_BLOB, blobPayload);

    const preSubAssignment = buildAssignment(givers, receivers, feeRate, [blobItem]);
    const postSubAssignment = substituteSeparable(preSubAssignment);

    const sharesOut = 1000000n;
    const preSubFee = recordingFee(
      BigInt(toBytes(preSubAssignment).length + 200),
      feeRate.num, feeRate.den, sharesOut,
    );
    const postSubFee = recordingFee(
      BigInt(toBytes(postSubAssignment).length + 200),
      feeRate.num, feeRate.den, sharesOut,
    );

    expect(preSubFee).toBeGreaterThan(postSubFee);
  });

  it('substituteSeparable replaces DATA_BLOB with SHA256 hash', async () => {
    const blobPayload = new Uint8Array([
      ...new TextEncoder().encode('image/png\0'),
      0x89, 0x50, 0x4e, 0x47,
    ]);
    const blobItem = bytesItem(tc.DATA_BLOB, blobPayload);
    const blobEncoded = toBytes(blobItem);
    const expectedHash = sha256Sync(blobEncoded);

    const result = substituteSeparable(blobItem);
    expect(result.typeCode).toBe(tc.SHA256);
    if (result.value.kind !== 'bytes') throw new Error('expected bytes');
    expect(result.value.data).toEqual(expectedHash);
  });

  it('assignment without blobs is unchanged by substitution', async () => {
    const giverKey = await generateSigningKey();
    const recvKey = await generateSigningKey();
    const givers = [makeGiver(0n, 1000n, giverKey)];
    const receivers = [makeReceiver(recvKey, 990n)];

    const assignment = buildAssignment(givers, receivers, feeRate);
    const substituted = substituteSeparable(assignment);

    // Without blobs, sizes should be identical
    const origBytes = toBytes(assignment).length;
    const subBytes = toBytes(substituted).length;
    expect(origBytes).toBe(subBytes);
  });

  it('multiple blobs are each substituted independently', async () => {
    const giverKey = await generateSigningKey();
    const recvKey = await generateSigningKey();
    const givers = [makeGiver(0n, 1000n, giverKey)];
    const receivers = [makeReceiver(recvKey, 990n)];

    const blob1 = bytesItem(tc.DATA_BLOB, new Uint8Array([
      ...new TextEncoder().encode('image/png\0'),
      ...new Uint8Array(100).fill(0x01),
    ]));
    const blob2 = bytesItem(tc.DATA_BLOB, new Uint8Array([
      ...new TextEncoder().encode('image/jpeg\0'),
      ...new Uint8Array(200).fill(0x02),
    ]));

    const assignment = buildAssignment(givers, receivers, feeRate, [blob1, blob2]);
    const substituted = substituteSeparable(assignment);

    if (substituted.value.kind !== 'container') throw new Error('expected container');
    const sha256Children = substituted.value.children.filter(c => c.typeCode === tc.SHA256);
    expect(sha256Children.length).toBe(2);

    // Each hash should be 32 bytes
    for (const h of sha256Children) {
      if (h.value.kind !== 'bytes') throw new Error('expected bytes');
      expect(h.value.data.length).toBe(32);
    }
  });
});
