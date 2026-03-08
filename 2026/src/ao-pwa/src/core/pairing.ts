// Device pairing: X25519 key agreement + HKDF-SHA256 for relay key derivation.
// Spec: specs/WalletSync.md §4.1
//
// Uses Web Crypto API ECDH with X25519 (Chrome 113+, Firefox 129+, Safari 17+).
// Falls back gracefully — pairing is unavailable if X25519 ECDH is not supported.

import { bytesToHex, hexToBytes, buf } from './hex.ts';

// ── X25519 Key Agreement ─────────────────────────────────────────────

/** Generate an ephemeral X25519 keypair for pairing. */
export async function generateX25519Pair(): Promise<{ privateKey: CryptoKey; publicKeyHex: string }> {
  const keyPair = await crypto.subtle.generateKey(
    { name: 'X25519' },
    true,  // extractable (we need the raw public key)
    ['deriveBits'],
  ) as CryptoKeyPair;

  const rawPub = await crypto.subtle.exportKey('raw', keyPair.publicKey);
  return {
    privateKey: keyPair.privateKey,
    publicKeyHex: bytesToHex(new Uint8Array(rawPub)),
  };
}

/** Derive a shared secret from our private key + their public key. */
async function deriveSharedSecret(
  privateKey: CryptoKey,
  peerPublicKeyHex: string,
): Promise<Uint8Array> {
  const peerPubRaw = hexToBytes(peerPublicKeyHex);
  if (peerPubRaw.length !== 32) {
    throw new Error(`Invalid X25519 public key: expected 32 bytes, got ${peerPubRaw.length}`);
  }
  const peerPubKey = await crypto.subtle.importKey(
    'raw', buf(peerPubRaw), { name: 'X25519' }, false, [],
  );
  const bits = await crypto.subtle.deriveBits(
    { name: 'X25519', public: peerPubKey },
    privateKey,
    256, // 32 bytes
  );
  return new Uint8Array(bits);
}

/** HKDF-SHA256 to derive the relay key from the shared secret. */
async function hkdfDerive(
  sharedSecret: Uint8Array,
  salt: string,
  info: string,
  length: number,
): Promise<Uint8Array> {
  const keyMaterial = await crypto.subtle.importKey(
    'raw', buf(sharedSecret), 'HKDF', false, ['deriveBits'],
  );
  const derived = await crypto.subtle.deriveBits(
    {
      name: 'HKDF',
      hash: 'SHA-256',
      salt: new TextEncoder().encode(salt),
      info: new TextEncoder().encode(info),
    },
    keyMaterial,
    length * 8,
  );
  return new Uint8Array(derived);
}

/**
 * Complete one side of the pairing ceremony.
 * Given our ephemeral private key and the peer's public key,
 * derive the symmetric relay_key.
 */
export async function deriveRelayKey(
  ourPrivateKey: CryptoKey,
  peerPublicKeyHex: string,
): Promise<string> {
  const shared = await deriveSharedSecret(ourPrivateKey, peerPublicKeyHex);
  const relayKey = await hkdfDerive(shared, 'ao-wallet-sync-v1', 'relay', 32);
  return bytesToHex(relayKey);
}

// ── Pairing QR Payloads ──────────────────────────────────────────────

export interface PairInitPayload {
  v: 1;
  type: 'pair';
  pub: string;       // X25519 public key hex
  deviceId: string;
  label: string;
}

export interface PairAckPayload {
  v: 1;
  type: 'pair_ack';
  pub: string;       // X25519 public key hex
  deviceId: string;
  label: string;
}

export function serializePairPayload(p: PairInitPayload | PairAckPayload): string {
  return JSON.stringify(p);
}

export function deserializePairPayload(json: string): PairInitPayload | PairAckPayload {
  const obj = JSON.parse(json);
  if (obj.v !== 1 || (obj.type !== 'pair' && obj.type !== 'pair_ack')) {
    throw new Error('Invalid pairing payload');
  }
  if (!obj.pub || !obj.deviceId || !obj.label) {
    throw new Error('Missing pairing fields');
  }
  if (typeof obj.label !== 'string' || obj.label.length > 200) {
    throw new Error('Invalid label: must be a string of at most 200 characters');
  }
  // X25519 public key must be exactly 32 bytes (64 hex chars)
  if (typeof obj.pub !== 'string' || obj.pub.length !== 64 || !/^[0-9a-f]+$/i.test(obj.pub)) {
    throw new Error('Invalid public key: expected 64-char hex string');
  }
  return obj;
}

// ── Relay Encryption ─────────────────────────────────────────────────

/**
 * Encrypt a sync message for relay transport using the group/relay key.
 * Uses AES-GCM with a random 12-byte IV (Web Crypto native, no WASM needed).
 */
export async function encryptForRelay(
  relayKeyHex: string,
  plaintext: string,
): Promise<string> {
  const keyBytes = hexToBytes(relayKeyHex);
  const key = await crypto.subtle.importKey(
    'raw', buf(keyBytes), { name: 'AES-GCM', length: 256 }, false, ['encrypt'],
  );
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const encoded = new TextEncoder().encode(plaintext);
  const ciphertext = await crypto.subtle.encrypt(
    { name: 'AES-GCM', iv: buf(iv) }, key, buf(encoded),
  );
  // Prepend IV to ciphertext, base64-encode the whole thing
  const combined = new Uint8Array(12 + ciphertext.byteLength);
  combined.set(iv);
  combined.set(new Uint8Array(ciphertext), 12);
  return uint8ToBase64(combined);
}

/**
 * Decrypt a relay message using the group/relay key.
 */
export async function decryptFromRelay(
  relayKeyHex: string,
  encrypted: string,
): Promise<string> {
  const combined = base64ToUint8(encrypted);
  const iv = combined.slice(0, 12);
  const ciphertext = combined.slice(12);
  const keyBytes = hexToBytes(relayKeyHex);
  const key = await crypto.subtle.importKey(
    'raw', buf(keyBytes), { name: 'AES-GCM', length: 256 }, false, ['decrypt'],
  );
  const plaintext = await crypto.subtle.decrypt(
    { name: 'AES-GCM', iv: buf(iv) }, key, buf(ciphertext),
  );
  return new TextDecoder().decode(plaintext);
}

// ── Wallet ID ────────────────────────────────────────────────────────

/**
 * Derive the wallet_id from the relay key for relay topic addressing.
 * wallet_id = SHA-256(relay_key) truncated to 16 bytes, hex-encoded.
 *
 * Spec §4.2 says BLAKE3, but BLAKE3 requires WASM in the browser and
 * wallet_id is NOT a security-critical hash — it's an opaque topic name
 * on a relay that already handles encrypted blobs. SHA-256 is native to
 * Web Crypto, avoiding a ~200KB WASM dependency for a single derivation.
 */
export async function deriveWalletId(relayKeyHex: string): Promise<string> {
  const keyBytes = hexToBytes(relayKeyHex);
  const hash = await crypto.subtle.digest('SHA-256', buf(keyBytes));
  return bytesToHex(new Uint8Array(hash).slice(0, 16));
}

// ── Base64 helpers ───────────────────────────────────────────────────

function uint8ToBase64(bytes: Uint8Array): string {
  let binary = '';
  for (const b of bytes) binary += String.fromCharCode(b);
  return btoa(binary);
}

function base64ToUint8(b64: string): Uint8Array {
  const binary = atob(b64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

// ── Feature Detection ────────────────────────────────────────────────

/** Check if X25519 ECDH is available in this browser. */
export async function isX25519Available(): Promise<boolean> {
  try {
    await crypto.subtle.generateKey({ name: 'X25519' }, false, ['deriveBits']);
    return true;
  } catch {
    return false;
  }
}
