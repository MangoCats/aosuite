// Wallet: Ed25519 key management with encrypted storage.
// For Phase 3C MVP: PBKDF2 + AES-GCM via Web Crypto API (no WASM needed).
// Format: hex-encoded salt(16) || iv(12) || ciphertext(32+16) = 76 bytes total.

import { buf, bytesToHex, hexToBytes, concatBytes } from './hex.ts';

export interface EncryptedSeed {
  data: string;
  kdf: 'pbkdf2-sha256';
  iterations: number;
}

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

export async function encryptSeed(seed: Uint8Array, password: string): Promise<EncryptedSeed> {
  const salt = new Uint8Array(16);
  crypto.getRandomValues(salt);
  const iv = new Uint8Array(12);
  crypto.getRandomValues(iv);

  const key = await deriveKey(password, salt);
  const ciphertext = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv: buf(iv) }, key, buf(seed),
  );

  const data = concatBytes(salt, iv, new Uint8Array(ciphertext));
  return { data: bytesToHex(data), kdf: 'pbkdf2-sha256', iterations: PBKDF2_ITERATIONS };
}

export async function decryptSeed(encrypted: EncryptedSeed, password: string): Promise<Uint8Array> {
  const raw = hexToBytes(encrypted.data);
  const salt = raw.slice(0, 16);
  const iv = raw.slice(16, 28);
  const ciphertext = raw.slice(28);

  const key = await deriveKey(password, salt);
  const plaintext = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: buf(iv) }, key, buf(ciphertext),
  );

  return new Uint8Array(plaintext);
}

export interface WalletEntry {
  label: string;
  publicKeyHex: string;
  encryptedSeed: EncryptedSeed;
}
