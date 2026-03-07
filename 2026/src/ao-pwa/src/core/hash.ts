// SHA-256 hashing via Web Crypto API — port of ao-crypto/src/hash.rs

/** Compute SHA-256 hash, returns 32 bytes. Works in browser and Node 18+. */
export async function sha256(data: Uint8Array): Promise<Uint8Array> {
  const hash = await crypto.subtle.digest('SHA-256', data.buffer as ArrayBuffer);
  return new Uint8Array(hash);
}
