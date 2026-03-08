import { describe, it, expect } from 'vitest';
import { encryptBackup, decryptBackup } from '../backup.ts';

describe('backup encrypt/decrypt', () => {
  it('round-trips plaintext through encrypt then decrypt', async () => {
    const plaintext = '{"v":1,"type":"key_sync","device_id":"abc","keys":[],"spent":[]}';
    const password = 'testpassword123';
    const encrypted = await encryptBackup(plaintext, password);

    // Encrypted is hex string — at least salt(16) + iv(12) + data + tag(16) = 44 bytes = 88 hex
    expect(encrypted.length).toBeGreaterThan(88);
    expect(/^[0-9a-f]+$/.test(encrypted)).toBe(true);

    const decrypted = await decryptBackup(encrypted, password);
    expect(decrypted).toBe(plaintext);
  });

  it('fails with wrong password', async () => {
    const encrypted = await encryptBackup('secret data', 'correctpassword');
    await expect(decryptBackup(encrypted, 'wrongpassword')).rejects.toThrow();
  });

  it('handles large payloads', async () => {
    const large = JSON.stringify({ keys: Array(100).fill({ k: 'x'.repeat(64) }) });
    const encrypted = await encryptBackup(large, 'password123');
    const decrypted = await decryptBackup(encrypted, 'password123');
    expect(decrypted).toBe(large);
  });

  it('produces different ciphertext each time (random salt/iv)', async () => {
    const plaintext = 'same data';
    const password = 'samepassword';
    const a = await encryptBackup(plaintext, password);
    const b = await encryptBackup(plaintext, password);
    expect(a).not.toBe(b); // different salt + IV each time
  });
});
