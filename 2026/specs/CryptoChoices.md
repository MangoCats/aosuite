# Cryptographic Choices — Deliverable 0C

Specifies the exact cryptographic algorithms, key formats, signature construction, and wallet encryption for the 2026 Assign Onward implementation.

Related specs: [Architecture.md](Architecture.md) (0A), [WireFormat.md](WireFormat.md) (0B), [EconomicRules.md](EconomicRules.md) (0D).

---

## 1. Algorithm Summary

| Function | Algorithm | Output Size | Crate / API | Notes |
|----------|-----------|-------------|-------------|-------|
| Signatures | Ed25519 (RFC 8032) | 64 bytes | `ring` 0.17 (Rust), Web Crypto API (browser) | Replaces 2018 ECDSA-256 + RSA-3072 |
| Chain integrity hash | SHA2-256 | 32 bytes | `sha2` (Rust), Web Crypto API (browser) | Block hashing, separable item substitution |
| Content-addressing | BLAKE3 | 32 bytes | `blake3` (Rust), JS via WASM | Fast chain replay, large separable items |
| Wallet encryption | XChaCha20-Poly1305 | — | `chacha20poly1305` (Rust) | Encrypts private key seeds |
| Key derivation | Argon2id | 32 bytes | `argon2` (Rust) | Passphrase → encryption key |

**Scope:** One signature algorithm (Ed25519), two hash algorithms (SHA2-256, BLAKE3), one symmetric cipher (XChaCha20-Poly1305), one KDF (Argon2id). Future algorithms are accommodated by the type-code system without breaking existing data.

---

## 2. Ed25519 Signatures

### 2.1 Key Format

- **Private key seed:** 32 bytes (random). Never leaves the generating device.
- **Public key:** 32 bytes (derived from seed per RFC 8032).
- **On-chain format:** Raw 32 bytes. No DER, PEM, or ASN.1 wrapping. Type code `ED25519_PUB` (1).

### 2.2 Signature Format

- **Signature:** 64 bytes (R || S per RFC 8032). Type code `ED25519_SIG` (2).
- **Signed data:** `SHA2-256(assignment_with_separable_substitution) || timestamp_8bytes`.
- The signature covers both the content hash and the signing timestamp, binding the two together.

### 2.3 Signature Construction (Step by Step)

Given an assignment agreement and a signing timestamp:

1. **Serialize** the `ASSIGNMENT` container to its binary encoding per [WireFormat.md](WireFormat.md) §2.
2. **Substitute separable items:** Walk the serialized tree. For each DataItem whose type code is separable (`|code| & 0x20 ≠ 0`), replace the entire item (type code + size + data) with a `SHA256` item (type code 3) containing the SHA2-256 hash of the original item's complete encoding.
3. **Digest:** Compute `digest = SHA2-256(substituted_bytes)` — 32 bytes.
4. **Append timestamp:** `signed_data = digest || timestamp` — 40 bytes. The timestamp is the 8-byte big-endian AO timestamp (Unix seconds × 189,000,000).
5. **Sign:** `signature = Ed25519_Sign(private_key_seed, signed_data)` — 64 bytes.

### 2.4 Signature Verification

1. Reconstruct `signed_data` from the assignment and the claimed timestamp (same steps 1–4 above).
2. Verify: `Ed25519_Verify(public_key, signed_data, signature)` → true/false.
3. Check timestamp constraints (see §2.5).

### 2.5 Timestamp Constraints

- The signing timestamp must be **strictly greater** than the timestamp of the block in which the signer's shares were received (proves the signer had the shares before signing).
- The signing timestamp must be **≤ the block timestamp** of the recording block (no future-dated signatures).
- Within a single signer's key history, timestamps must be **strictly monotonic**.

### 2.6 Why Ed25519

| Criterion | Ed25519 | ECDSA (2018 choice) | RSA-3072 (2018 choice) |
|-----------|---------|---------------------|------------------------|
| Key size | 32 bytes | 32–33 bytes | 384 bytes |
| Signature size | 64 bytes | 64–72 bytes (DER) | 384 bytes |
| Sign speed | ~50 μs | ~100 μs | ~1 ms |
| Verify speed | ~150 μs | ~200 μs | ~50 μs |
| Deterministic | Yes (no random nonce) | Requires RFC 6979 | Yes |
| Web Crypto API | Yes (Chrome, Edge, Firefox, Safari 17+) | Yes | Yes |
| Rust support | `ring` 0.17 | `p256` | `rsa` (needs alloc) |

Ed25519 wins on compactness (critical for wire format thrift), determinism (no nonce-reuse vulnerability), and simplicity. RSA-3072 is dropped entirely — 384-byte signatures are unacceptable for LoRa mesh transport.

### 2.7 Crate Selection

Ed25519 is implemented via `ring` 0.17. The original plan used `ed25519-dalek`, but it was replaced during Phase 1 due to an RFC 8032 test vector discrepancy — see [lessons/wrong-test-vector.md](../lessons/wrong-test-vector.md). `ring` passes all RFC 8032 test vectors and is widely deployed (used by rustls, webpki).

---

## 3. Hash Algorithms

### 3.1 SHA2-256 (Primary)

Used for: block chain hashing (`PREV_HASH`), separable item substitution, chain ID computation, signature digest.

- **Output:** 32 bytes. Type code `SHA256` (3).
- **Input:** Raw bytes (no length prefix, no domain separator needed — the type-code structure provides implicit domain separation).
- **Rust crate:** `sha2` (RustCrypto, audited, `no_std`).
- **Browser:** `crypto.subtle.digest("SHA-256", data)`.

### 3.2 BLAKE3 (Secondary)

Used for: content-addressing of large separable items, performance-sensitive chain replay verification, any context where "hash this data quickly" is needed and the result is not part of the on-chain consensus.

- **Output:** 32 bytes. Type code `BLAKE3` (4).
- **Rust crate:** `blake3` (official, `no_std`).
- **Browser:** WASM build of `blake3` crate.

**Important:** BLAKE3 is **not** used in signature construction or chain hash linking. Those paths always use SHA2-256 for maximum interoperability and cryptanalytic confidence.

### 3.3 Hash Selection Rationale

| Criterion | SHA2-256 | SHA3-256 | BLAKE3 |
|-----------|----------|----------|--------|
| Cryptanalytic maturity | 23 years, extensive | 11 years, solid | 6 years, less scrutinized |
| ARM hardware accel | SHA2 extensions (Pi 5) | None | None (uses SIMD) |
| Web Crypto API | Native | Not available | WASM only |
| Software throughput | ~2–4 GB/s (x86 SHA-NI) | ~500 MB/s | ~4–8 GB/s |
| Construction | Merkle-Damgård | Sponge (Keccak) | Tree (ChaCha-based ARX) |

SHA3-256 is dropped: slower than both alternatives, no Web Crypto support, and BLAKE3 already provides structural diversity as a hedge against SHA2 family weaknesses.

### 3.4 Quantum Considerations

Grover's algorithm reduces preimage resistance from 2²⁵⁶ to ~2¹²⁸ and collision resistance from 2¹²⁸ to ~2⁸⁵. All 256-bit hashes are equally affected. The ~2¹²⁸ preimage bound remains infeasible for foreseeable quantum hardware. If a quantum threat materializes, the type-code system allows migration to SHA2-384 or SHA2-512 without protocol redesign.

---

## 4. Wallet Encryption

Private key seeds stored on user devices (browser IndexedDB, CLI config file) are encrypted at rest.

### 4.1 Key Derivation

```
salt = random 16 bytes (generated once per wallet, stored alongside ciphertext)
key  = Argon2id(passphrase, salt, memory=64MB, iterations=3, parallelism=1, output=32 bytes)
```

Argon2id parameters are chosen to take ~1 second on a mid-range phone (the weakest expected device). Desktop and Pi will be faster.

### 4.2 Encryption

```
nonce      = random 24 bytes
ciphertext = XChaCha20-Poly1305_Encrypt(key, nonce, private_key_seed)
stored     = salt || nonce || ciphertext  (16 + 24 + 48 = 88 bytes)
```

XChaCha20-Poly1305 provides authenticated encryption with a 24-byte nonce (no nonce-reuse concern with random generation).

### 4.3 Decryption

```
key            = Argon2id(passphrase, stored[0:16], same params)
private_key    = XChaCha20-Poly1305_Decrypt(key, stored[16:40], stored[40:88])
```

Decryption failure (bad passphrase) produces an authentication error, not garbage data.

### 4.4 Backup Format

Wallet export is a JSON file:

```json
{
  "version": 1,
  "keys": [
    {
      "public_key": "<hex 64 chars>",
      "encrypted_seed": "<hex 176 chars>",
      "chain_id": "<hex 64 chars>",
      "label": "My BCG wallet"
    }
  ]
}
```

The `encrypted_seed` field is the 88-byte `salt || nonce || ciphertext` hex-encoded. The backup file itself should be further protected by the user (encrypted storage, secure transfer).

---

## 5. Key Management Rules

### 5.1 Single-Use Share Keys

A public key used to receive shares on a chain MUST NOT be used to receive shares again on the same chain. This is enforced by the Recorder (reject duplicate receiver keys) and verified by Validators.

**Identity keys** (used for signing non-share messages, e.g., vendor profiles) MAY be reused. These are not share-receiving keys and do not appear in the UTXO set.

### 5.2 Key-Never-Reuse Tracking

Key-never-reuse is enforced by the Recorder's UTXO layer (`ao-chain`), which maintains a persistent `used_keys` table in SQLite. Any assignment with a receiver public key already present in the table is rejected. This was originally planned as a stateless check in `ao-crypto`, but was moved to `ao-chain` during Phase 2 because enforcement requires persistent state across sessions.

### 5.3 Key Generation

Ed25519 key pairs are generated from 32 bytes of cryptographically secure randomness:

- **Rust:** `SystemRandom` from `ring` crate.
- **Browser:** `crypto.getRandomValues(new Uint8Array(32))`.

No deterministic key derivation (BIP-32, etc.) is used. Each key is independently random. This simplifies the security model at the cost of requiring explicit backup of each key.

### 5.4 Multi-Device Key Sync

When users access the same wallet from multiple devices, private key seeds must be transferred securely between devices. Three mechanisms are specified in [WalletSync.md](WalletSync.md):

1. **QR/NFC transfer (default):** Seeds are encrypted with the wallet passphrase (§4.1–4.2), then encoded as a QR payload. Air-gapped — seeds never transit a network.
2. **Paired-device relay:** Seeds are double-encrypted — inner layer with wallet passphrase (§4.1–4.2), outer layer with a shared `group_key` (X25519 key agreement + HKDF-SHA256) for relay transport. The relay server sees only opaque ciphertext.
3. **Cloud vault (optional):** Full wallet state encrypted with a vault passphrase (same Argon2id + XChaCha20-Poly1305 as §4.1–4.2) and stored as a single blob.

In all cases, private key seeds are encrypted before leaving the originating device. The wallet passphrase provides at-rest protection; the relay/vault layer provides in-transit protection. Neither the relay server nor the cloud storage endpoint ever sees plaintext seeds.

---

## 6. Divergence from 2018 Spec

| Aspect | 2018 | 2026 | Rationale |
|--------|------|------|-----------|
| Signatures | ECDSA brainpool-256 + RSA-3072 | Ed25519 only | Compact, deterministic, web-native |
| Hashes | SHA2-256 + SHA3-512 | SHA2-256 + BLAKE3 | SHA3 dropped (slow, no Web Crypto); BLAKE3 faster and structurally diverse |
| Key wrapping | Not specified | Argon2id + XChaCha20-Poly1305 | Modern, audited, memory-hard KDF |
| Signature encoding | DER-wrapped | Raw bytes | Smaller, simpler |
| Multiple sig algorithms | Two (ECDSA + RSA) | One (Ed25519) | Simplicity; agility preserved via type codes for future additions |
