import { describe, it, expect } from 'vitest';
import {
  buildRecorderChangePending, buildRecorderUrlChange,
  signRecorderOp, dualSignRecorderOp,
  recorderSwitchPhase,
} from '../recorderSwitch.ts';
import { generateSigningKey } from '../sign.ts';
import { children, findChild } from '../dataitem.ts';
import * as tc from '../typecodes.ts';

describe('buildRecorderChangePending', () => {
  it('builds RECORDER_CHANGE_PENDING with pubkey and URL', async () => {
    const newRecorder = await generateSigningKey();
    const pending = buildRecorderChangePending(
      newRecorder.publicKey,
      'https://new-recorder.example.com',
    );
    expect(pending.typeCode).toBe(tc.RECORDER_CHANGE_PENDING);
    const pub = findChild(pending, tc.ED25519_PUB);
    expect(pub).toBeDefined();
    const url = findChild(pending, tc.RECORDER_URL);
    expect(url).toBeDefined();
    if (url && url.value.kind === 'bytes') {
      const urlStr = new TextDecoder().decode(url.value.data);
      expect(urlStr).toBe('https://new-recorder.example.com');
    }
  });
});

describe('buildRecorderUrlChange', () => {
  it('builds RECORDER_URL_CHANGE with new URL', () => {
    const change = buildRecorderUrlChange('https://updated.example.com');
    expect(change.typeCode).toBe(tc.RECORDER_URL_CHANGE);
    const url = findChild(change, tc.RECORDER_URL);
    expect(url).toBeDefined();
  });
});

describe('signRecorderOp', () => {
  it('wraps in AUTHORIZATION with single AUTH_SIG', async () => {
    const ownerKey = await generateSigningKey();
    const newRecorder = await generateSigningKey();
    const pending = buildRecorderChangePending(newRecorder.publicKey, 'https://r.com');
    const auth = await signRecorderOp(ownerKey, pending);

    expect(auth.typeCode).toBe(tc.AUTHORIZATION);
    const kids = children(auth);
    expect(kids[0].typeCode).toBe(tc.RECORDER_CHANGE_PENDING);
    expect(kids[1].typeCode).toBe(tc.AUTH_SIG);
  });
});

describe('dualSignRecorderOp', () => {
  it('wraps in AUTHORIZATION with two AUTH_SIGs', async () => {
    const ownerKey = await generateSigningKey();
    const recorderKey = await generateSigningKey();
    const change = buildRecorderUrlChange('https://new.example.com');
    const auth = await dualSignRecorderOp(ownerKey, recorderKey, change);

    expect(auth.typeCode).toBe(tc.AUTHORIZATION);
    const kids = children(auth);
    expect(kids[0].typeCode).toBe(tc.RECORDER_URL_CHANGE);
    expect(kids[1].typeCode).toBe(tc.AUTH_SIG);
    expect(kids[2].typeCode).toBe(tc.AUTH_SIG);
    // Two different signers
    const pub1 = findChild(kids[1], tc.ED25519_PUB);
    const pub2 = findChild(kids[2], tc.ED25519_PUB);
    expect(pub1).toBeDefined();
    expect(pub2).toBeDefined();
  });
});

describe('recorderSwitchPhase', () => {
  it('returns idle when no pending change', () => {
    expect(recorderSwitchPhase(false, 0, false)).toBe('idle');
  });

  it('returns draining when pending with active CAAs', () => {
    expect(recorderSwitchPhase(true, 3, false)).toBe('draining');
  });

  it('returns ready when pending with no active CAAs', () => {
    expect(recorderSwitchPhase(true, 0, false)).toBe('ready');
  });

  it('returns failed when chain is frozen', () => {
    expect(recorderSwitchPhase(true, 0, true)).toBe('failed');
    expect(recorderSwitchPhase(false, 0, true)).toBe('failed');
  });
});
