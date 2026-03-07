import { describe, it, expect } from 'vitest';
import { encryptSeed, decryptSeed } from '../wallet.ts';
import { bytesToHex } from '../hex.ts';

describe('Wallet encryption', () => {
  it('encrypts and decrypts a seed', async () => {
    const seed = new Uint8Array(32);
    crypto.getRandomValues(seed);
    const password = 'test-password-123';

    const encrypted = await encryptSeed(seed, password);
    expect(encrypted.kdf).toBe('pbkdf2-sha256');
    expect(encrypted.data.length).toBe(152); // (16+12+48) * 2 hex chars

    const decrypted = await decryptSeed(encrypted, password);
    expect(bytesToHex(decrypted)).toBe(bytesToHex(seed));
  });

  it('wrong password fails', async () => {
    const seed = new Uint8Array(32).fill(0x42);
    const encrypted = await encryptSeed(seed, 'correct-password');

    await expect(decryptSeed(encrypted, 'wrong-password'))
      .rejects.toThrow();
  });
});
