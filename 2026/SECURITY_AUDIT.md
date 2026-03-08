# Security Audit Report — aosuite 2026 Implementation

**Date:** 2026-03-07
**Scope:** All Rust crates in `2026/src/` (ao-types, ao-crypto, ao-chain, ao-recorder, ao-exchange, ao-validator, ao-cli)
**Method:** Full source review by AI security auditor (Claude Opus 4.6)

## Executive Summary

The core cryptography, serialization, and chain logic are well-engineered. Rust's type system, ring's Ed25519, and arbitrary-precision arithmetic eliminate entire classes of bugs. The significant risks are in the network/deployment layer: missing authentication, denial-of-service vectors, and trust assumptions around multi-chain operations.

---

## Findings

### CRITICAL / HIGH

#### F1. No Authentication on Any HTTP API

**Affected:** ao-recorder, ao-exchange, ao-validator — all endpoints publicly accessible.

Anyone can submit blocks (`POST /chain/{id}/submit`), initiate atomic swaps (`POST /chain/{id}/caa/submit`), register exchange agents (`POST /chain/{id}/exchange-agent`), and query validator status. Signature verification on block *contents* is strong, but there is no gating on who can talk to the server. A spam attacker does not need valid signatures to consume CPU and IO attempting to submit garbage.

**Recommendation:** Authentication layer (API keys, mTLS, or at minimum IP allowlisting) before any public deployment.

#### F2. Unbounded SSE/WebSocket Connections (DoS)

**File:** `ao-recorder/src/lib.rs` — broadcast channel capacity is 64.

No limit on concurrent SSE or WebSocket connections. An attacker can open thousands of connections, exhausting memory and file descriptors. Lagged SSE subscribers are silently dropped; lagged WebSocket clients silently skip messages with no notification.

**Recommendation:** Per-IP connection limits, explicit lag notification or disconnection, Tower middleware for rate limiting.

#### F3. CAA Known-Recorder Trust Is Config-Only

**File:** `ao-recorder/src/lib.rs` — `known_recorders` map loaded from config.

No runtime verification of recorder identity. An attacker who controls the config (or a compromised config file) can inject fake recorder keys, allowing forged recording proofs in multi-chain CAA exchanges.

**Recommendation:** Cryptographic recorder identity verification (signed discovery, pinned keys).

#### F4. No Rate Limiting Anywhere

No rate limiting on any endpoint. Block submission, CAA submission, chain creation — all can be spammed without throttling.

**Recommendation:** Tower middleware rate limiting per IP. Exponential backoff for repeated invalid submissions.

---

### MEDIUM

#### F5. Exchange Rate Uses f64 Arithmetic

**File:** `ao-exchange/src/engine.rs:46-58`

`compute_sell_amount` converts BigInt to f64 for division, then truncates back to i64. Precision loss on large amounts could cause the exchange to systematically lose fractional shares per trade.

**Recommendation:** Use `num-rational` or BigInt division with explicit rounding policy.

#### F6. PREV_HASH Not Validated on Block Acceptance

**File:** `ao-chain/src/block.rs`

Blocks carry a `PREV_HASH` field, and the store advances it, but incoming blocks are never checked against the expected previous hash. In the current single-blockmaker design this is safe. If multi-producer consensus is ever added, this becomes a critical fork attack vector.

**Recommendation:** Add explicit PREV_HASH validation now — cheap insurance against future architectural changes.

#### F7. `.expect()` on Locks and System Clock

**Files:** ~20+ locations across ao-recorder, ao-exchange, ao-validator.

`.expect()` calls on mutex locks (`"store lock poisoned"`) and `SystemTime::now()` (`"system clock"`). Lock poisoning from a thread panic or clock failure crashes the entire daemon.

**Recommendation:** Graceful error handling, or panic isolation via `spawn_blocking` with catch_unwind.

#### F8. Validator Polling — No TLS Validation, Potential SSRF

**File:** `ao-recorder/src/lib.rs`

Validator URLs from config are fetched via HTTP with no TLS pinning or hostname verification. If config is user-supplied, this is an SSRF vector. Chain IDs interpolated into URLs are not URL-encoded.

**Recommendation:** Enforce HTTPS, validate URLs at config load time, URL-encode path parameters.

#### F9. Block Deserialization on GET /blocks

Up to 1000 blocks deserialized from DB and JSON-serialized in a single response. A malicious blockmaker could craft large blocks that exhaust memory during serialization.

**Recommendation:** Per-block size limit, streaming JSON response, or timeout on serialization.

---

### LOW

#### F10. Error Messages Leak Internal State

`RecorderError::BadRequest(e.to_string())` exposes parsing error details to callers — field names, expected formats, internal structure.

**Recommendation:** Generic error messages in production responses; detailed errors only in server logs.

#### F11. MQTT Backpressure

Channel capacity 64 with no overflow handling. If broker is slow, messages are silently dropped.

**Recommendation:** Monitor queue depth, explicit overflow handling.

#### F12. Exchange Seeds Returned Over HTTP

`/trade` endpoint returns `deposit_seed` and `receive_seed` in plaintext. By design (consumer needs them), but requires TLS in deployment — not enforced in code.

**Recommendation:** Document TLS requirement. Consider enforcing HTTPS-only responses for seed-bearing endpoints.

---

## What's Done Well

| Area | Assessment |
|------|-----------|
| Ed25519 (ring 0.17) | Correct, constant-time, no custom crypto |
| Signature enforcement | Verified at every entry point — genesis, assignments, CAA components, recording proofs |
| Key reuse prevention | Per-chain used_keys table, one key per UTXO |
| VBC encoding | Overflow-safe, rejects negative zero, extensive conformance tests |
| Fee arithmetic | BigInt throughout, ceiling rounding, conformance vectors |
| Double-spend prevention | UTXO status atomically transitions Unspent→Spent inside `BEGIN IMMEDIATE` transactions |
| Separable substitution | Correctly walks tree, replaces by bitmask, well-tested |
| Timestamp binding | Cryptographically bound into signatures, monotonically enforced |
| Late recording | Bounded by both refutation mechanism and UTXO expiry |
| Buffer safety | All bounds-checked, no unsafe in data paths, body size limits |
| SQL injection | All queries parameterized via `params!` macro |
| No unsafe code | Zero `unsafe` blocks in any security-critical path |
| Type safety | Fixed-size arrays for keys/hashes prevent off-by-one; unknown type codes rejected |
| JSON round-trip | Serialize→deserialize preserves all data exactly; tested with conformance vectors |

---

## Methodology Notes

- Full source review of all `.rs` files in `2026/src/`
- Traced all HTTP handler paths from Axum router to storage layer
- Verified signature verification is enforced at every block/assignment/CAA acceptance point
- Checked all `unwrap()`/`expect()` calls for untrusted-input panic risk
- Reviewed VBC, DataItem, BigInt deserialization for overflow, nesting, and bounds issues
- Examined fee calculation for rounding leakage
- Reviewed UTXO lifecycle for double-spend and race conditions
- Checked SQL queries for injection
- Assessed CAA escrow flow for trust assumptions

No penetration testing or fuzzing was performed. This audit covers design and implementation review only.
