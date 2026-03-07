// Ed25519 signing pipeline — port of ao-crypto/src/sign.rs
// Uses Web Crypto API (Ed25519 support: Chrome 113+, Firefox 129+, Safari 17+).
//
// AO signing pipeline (WireFormat.md §6.2):
// 1. Substitute separable items with SHA256 hashes
// 2. Serialize substituted tree to bytes
// 3. digest = SHA256(substituted_bytes)
// 4. signed_data = digest || timestamp (8 bytes BE)
// 5. Ed25519 sign the 40-byte message

import { type DataItem, toBytes as dataItemToBytes } from './dataitem.ts';
import { substituteSeparable } from './separable.ts';
import { sha256 } from './hash.ts';
import { timestampToBytes } from './timestamp.ts';
import { buf, concatBytes } from './hex.ts';

/** An Ed25519 signing key backed by Web Crypto. */
export interface SigningKey {
  /** The CryptoKeyPair from Web Crypto. */
  keyPair: CryptoKeyPair;
  /** The 32-byte seed (private key material). */
  seed: Uint8Array;
  /** The 32-byte public key. */
  publicKey: Uint8Array;
}

/** Import a 32-byte Ed25519 seed into a Web Crypto signing key. */
export async function signingKeyFromSeed(seed: Uint8Array): Promise<SigningKey> {
  const pkcs8Prefix = new Uint8Array([
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06,
    0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04, 0x20,
  ]);
  const pkcs8 = concatBytes(pkcs8Prefix, seed);

  const privateKey = await crypto.subtle.importKey(
    'pkcs8', buf(pkcs8), { name: 'Ed25519' }, true, ['sign'],
  );

  const jwk = await crypto.subtle.exportKey('jwk', privateKey);
  const publicKey = base64urlToBytes(jwk.x!);

  const pubCryptoKey = await crypto.subtle.importKey(
    'raw', buf(publicKey), { name: 'Ed25519' }, true, ['verify'],
  );

  return {
    keyPair: { privateKey, publicKey: pubCryptoKey },
    seed: new Uint8Array(seed),
    publicKey,
  };
}

/** Generate a new random Ed25519 signing key. */
export async function generateSigningKey(): Promise<SigningKey> {
  const seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  return signingKeyFromSeed(seed);
}

/** Sign raw bytes with Ed25519, returns 64-byte signature. */
export async function signRaw(key: SigningKey, message: Uint8Array): Promise<Uint8Array> {
  const sig = await crypto.subtle.sign('Ed25519', key.keyPair.privateKey, buf(message));
  return new Uint8Array(sig);
}

/** Verify a raw Ed25519 signature. */
export async function verifyRaw(
  publicKey: Uint8Array, message: Uint8Array, signature: Uint8Array,
): Promise<boolean> {
  const pubKey = await crypto.subtle.importKey(
    'raw', buf(publicKey), { name: 'Ed25519' }, false, ['verify'],
  );
  return crypto.subtle.verify('Ed25519', pubKey, buf(signature), buf(message));
}

/** Build the 40-byte signed_data = SHA256(substituted_encoding) || timestamp. */
export async function buildSignedData(item: DataItem, aoTimestamp: bigint): Promise<Uint8Array> {
  const substituted = substituteSeparable(item);
  const encoded = dataItemToBytes(substituted);
  const digest = await sha256(encoded);
  const tsBytes = timestampToBytes(aoTimestamp);
  return concatBytes(digest, tsBytes);
}

/** AO signing: substitute separable -> encode -> SHA256 -> append timestamp -> Ed25519 sign. */
export async function signDataItem(
  key: SigningKey, item: DataItem, aoTimestamp: bigint,
): Promise<Uint8Array> {
  const signedData = await buildSignedData(item, aoTimestamp);
  return signRaw(key, signedData);
}

/** Verify an AO signature against a DataItem, timestamp, and public key. */
export async function verifyDataItem(
  publicKey: Uint8Array, item: DataItem, aoTimestamp: bigint, signature: Uint8Array,
): Promise<boolean> {
  const signedData = await buildSignedData(item, aoTimestamp);
  return verifyRaw(publicKey, signedData, signature);
}

function base64urlToBytes(b64url: string): Uint8Array {
  const b64 = b64url.replace(/-/g, '+').replace(/_/g, '/');
  const padded = b64 + '='.repeat((4 - b64.length % 4) % 4);
  const binary = atob(padded);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
