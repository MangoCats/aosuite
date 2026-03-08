// Wallet backup encryption: PBKDF2-SHA256 + AES-256-GCM.
// Matches wallet.ts pattern but operates on arbitrary-length strings.

import { bytesToHex, hexToBytes, concatBytes, buf } from './hex.ts';

const PBKDF2_ITERATIONS = 600_000;

async function deriveKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const enc = new TextEncoder();
  const keyMaterial = await crypto.subtle.importKey(
    'raw', buf(enc.encode(password)), 'PBKDF2', false, ['deriveKey'],
  );
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt: buf(salt), iterations: PBKDF2_ITERATIONS, hash: 'SHA-256' },
    keyMaterial,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}

/** Encrypt a plaintext string with a password. Returns hex(salt || iv || ciphertext+tag). */
export async function encryptBackup(plaintext: string, password: string): Promise<string> {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const key = await deriveKey(password, salt);
  const data = new TextEncoder().encode(plaintext);
  const ciphertext = new Uint8Array(await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv: buf(iv) }, key, buf(data),
  ));
  return bytesToHex(concatBytes(salt, iv, ciphertext));
}

/** Decrypt a hex-encoded encrypted backup. Throws on wrong password. */
export async function decryptBackup(encryptedHex: string, password: string): Promise<string> {
  const raw = hexToBytes(encryptedHex);
  const salt = raw.slice(0, 16);
  const iv = raw.slice(16, 28);
  const ciphertext = raw.slice(28);
  const key = await deriveKey(password, salt);
  const plaintext = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: buf(iv) }, key, buf(ciphertext),
  );
  return new TextDecoder().decode(plaintext);
}

/** Backup file envelope structure. */
export interface BackupFile {
  v: 1;
  type: 'ao_wallet_backup';
  created: string;
  encrypted: string;
}
