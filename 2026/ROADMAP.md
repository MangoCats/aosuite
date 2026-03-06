# Development Roadmap

This is a living document, updated as development progresses. Phase descriptions will be revised as earlier phases reveal better approaches, and completed phases will be condensed to reflect what was actually built rather than what was planned.

Six phases over approximately 54 weeks, starting with architecture and specification before any code is written. Each phase produces a working, demonstrable system before the next begins.

## Technology Stack

| Component | Choice | Notes |
|-----------|--------|-------|
| Language | Rust (stable, `no_std` for core crates) | Memory safety for crypto, cross-compile to ARM |
| Signatures | `ed25519-dalek` 2.x | Pin version, audit. `ring` as backup. |
| Hashes | SHA2-256 + BLAKE3 (`sha2`, `blake3`) | Mature, audited, `no_std`. SHA2-256 is default; BLAKE3 where explicitly specified. |
| Big integers | `num-bigint` + `num-rational` | Pure Rust, adequate for AO's scale. Benchmark fee calculation paths. |
| HTTP server | Axum 0.8+ | Stable, tokio ecosystem. |
| MQTT | `rumqttc` | Consider `rumqttd` embedded broker for simple deployments. |
| Storage | `rusqlite` | Synchronous, wrap with `spawn_blocking` if needed. |
| Client UI | React PWA (TypeScript) | Cross-platform, no app store, offline capable. |
| Client crypto | Web Crypto API (primary) | Ed25519 in Chrome, Edge, Firefox. `tweetnacl-js` as Safari fallback. |
| Wallet encryption | Argon2id + XChaCha20-Poly1305 | For private key storage in browser and CLI. |
| Testing | `cargo test` + `proptest` + conformance vectors | Property-based + hand-computed ground truth in JSON. |

## Phase Overview

| Phase | Weeks | Deliverables |
|-------|-------|-------------|
| 0: Architecture & Specification | 1–4 | Resolve ambiguities, produce implementation-grade specs, generate test vectors |
| 1: Foundation | 5–10 | `ao-types` + `ao-crypto` crates, genesis CLI |
| 2: Single-Chain Recorder (TⒶ¹) | 11–20 | `ao-chain` + `ao-recorder`, full CLI tools |
| 3: Vendor + Consumer Apps | 21–28 | React PWA with AOS + AOE views |
| 4: Market Making + Exchange | 29–38 | AOI view, exchange agents, MQTT, automated trading |
| 5: Validation + Trust (AOV) | 39–44 | Validator, anchor proofs, vendor credentials |
| 6: Atomic Multi-Chain (TⒶ²) | 45–54 | Full CAA escrow protocol |

---

## Phase 0: Architecture & Specification (Weeks 1–4)

This phase produces documents, not code. The 2018 design documents are spread across 30+ HTML files with numerous ambiguities — unspecified negative VBC encoding, undocumented type code schema, unresolved separability bit position, unspecified signature byte format, unspecified deterministic rounding rules. All of these are resolved in writing before coding begins.

### Deliverable 0A: System Architecture Document — [specs/Architecture.md](specs/Architecture.md) ✓ 2026-03-05



Consolidates architectural decisions into a single implementation-grade document covering:

**Actor model:** Precise definitions of each actor (Asset Organizer, Recorder, Validator, Checker) with trust boundaries, data held, operations performed, and messages exchanged. Separates A1 (what we build now) from future protocol levels.

**System topology:** How AOR servers, AOV validators, AOE/AOS/AOI clients, and MQTT brokers connect. Which connections are HTTP, WebSocket, or MQTT. Where TLS is required vs. optional. How discovery works (genesis block URLs, exchange agent directories).

**Data flow diagrams:** Complete lifecycle of a share assignment from proposal through recording, for both single-chain (A1) and the simplified exchange-agent model. Sequence diagrams, not prose.

**Security model:** Where private keys live and never leave. What each component can and cannot do if compromised. The recorder never sees private keys. The validator can only invalidate, never validate.

**Non-negotiable principles:** Stated as testable assertions. No proof of work. Single-use keys for share transfers. Mutual consent. Timestamped signatures. Share expiration. Separable items. Cryptographic agility. Open source.

### Deliverable 0B: Wire Format Specification — [specs/WireFormat.md](specs/WireFormat.md) ✓ 2026-03-06



Precise, byte-level specification of the on-chain binary format. Design for minimal message size — protocol messages must be viable not only over standard internet and 5G but also over low-bandwidth transports such as [Meshtastic](https://meshtastic.org/) LoRa mesh networks.

**VBC encoding:** Worked examples for positive integers (0, 1, 63, 64, 127, 128, 8191, 8192), negative integers (-1, -64, -65), and boundary values near the 10-byte limit. Bit 0 = sign (0=positive, 1=negative), bits 1-6 of byte 0 = least significant 6 magnitude bits, bit 7 = continuation. Continuation bytes: 7 data bits each (bits 0-6), bit 7 = continuation. Total: 63 bits magnitude + sign = signed 64-bit integer. Table of byte sequences for 20+ test values.

**DataItem structure:** Type code (VBC) + optional size (VBC) + data. Complete type code registry table replacing the undocumented `byteCodeDefinitions.json`.

**GMP integer encoding:** VBC byte count + big-endian bytes, MSB of first byte = sign. Zero = zero bytes. Worked examples for 0, 1, -1, 127, 128, -128, 2⁶⁴, ~2⁸⁶.

**Rational fraction encoding:** Nested VBC sizes (outer total, inner numerator). Denominator always positive. Worked examples for 1/2, -3/7, and a typical fee calculation fraction.

**Timestamp encoding:** 8 bytes big-endian, Unix seconds × 189,000,000 (~5.29ns resolution). Monotonically increasing per actor. Worked examples for epoch, 2026, and rollover.

**Block structure:** Complete byte-level layout of an A1 signed block. Worked example of a minimal block containing one assignment.

### Deliverable 0C: Cryptographic Choices Document — [specs/CryptoChoices.md](specs/CryptoChoices.md) ✓ 2026-03-06



**Signature:** Ed25519 per RFC 8032. 32-byte keys, 64-byte signatures, raw bytes (no DER). Signed data = serialized DataContainer with separable items replaced by SHA2-256 hashes, concatenated with 8-byte signing timestamp.

**Hashes:** SHA2-256 (32 bytes) for chain integrity and separable item replacement. BLAKE3 (32 bytes) for content-addressing and performance-sensitive hashing. Unqualified "hash" means SHA2-256.

**Key serialization:** Raw 32-byte Ed25519 public keys on-chain. Private key seeds encrypted with XChaCha20-Poly1305 via Argon2id-derived key (memory=64MB, iterations=3, parallelism=1).

**Scope:** One signature type (Ed25519), two hash types (SHA2-256, BLAKE3). Cryptographic agility preserved via type-code system for future additions.

### Deliverable 0D: Economic Rules Specification — [specs/EconomicRules.md](specs/EconomicRules.md) ✓ 2026-03-06



Deterministic arithmetic rules that all nodes must agree on:

**Recording fee:** `fee_shares = ceil(data_bytes * fee_rate * total_shares / total_coins)`. All arbitrary-precision integer, division last, ceil rounds toward positive infinity. 5 worked examples.

**Coin display:** `user_coins = user_shares * total_coins / total_shares`. Display only, truncate to 9 decimal places (nanocoin).

**Expiration:** Mode 1 (hard cutoff): shares removed after `expiry_period`. Mode 2 (age tax): `tax_fraction = 1 - 2^(-(age - tax_start_age) / tax_doubling_period)`. Tax computed at block recording time. Worked examples for both modes.

**Late recording:** Past-deadline agreements MAY be recorded if (a) source shares unspent and (b) no explicit refutation recorded. Window bounded by share expiration. Wallets SHOULD warn and offer one-tap refutation.

### Deliverable 0E: Conformance Test Vectors — [specs/conformance/vectors.json](specs/conformance/vectors.json) ✓ 2026-03-06



JSON file with hand-computed test vectors: VBC (30+), GMP integers (15+), rationals (10+), timestamps (5+), SHA2-256 hashes, Ed25519 signatures with timestamp concatenation, minimal genesis block (binary + JSON), minimal assignment agreement, minimal block with correct hash, recording fee calculations (5), expiration calculations (both modes). These are the ground truth for compatibility.

### Acceptance Criteria

A developer unfamiliar with the project can read documents 0A–0D and test vectors 0E and build Phases 1–2 without consulting any other document. Every byte specified. Every arithmetic operation deterministic. Every crypto operation specified at byte level.

---

## Phase 1: Foundation (Weeks 5–10) — ✓ 2026-03-06

Build `ao-types` and `ao-crypto` crates plus the genesis block creator, building against Phase 0 specifications.

### Deliverables

**ao-types** — [src/ao-types/](src/ao-types/) ✓: VBC codec (signed/unsigned). DataItem binary + JSON codec. Type code registry (37 codes). BigInt/Rational encoding via `num-bigint`/`num-rational`. Timestamp type. Recording fee arithmetic with ceil rounding. Separable item identification (`is_separable`). 39 tests (including 5 proptest property tests).

**ao-crypto** — [src/ao-crypto/](src/ao-crypto/) ✓: Ed25519 via `ring` 0.17 (switched from `ed25519-dalek` — see [lessons/wrong-test-vector.md](lessons/wrong-test-vector.md)). SHA2-256 and BLAKE3. Separable-item hash-substitution. Sign/verify DataItem pipeline per WireFormat.md §6.2. 13 tests. Key-never-reuse tracking deferred to Phase 2 UTXO layer (requires persistent state).

**ao-cli** — [src/ao-cli/](src/ao-cli/) ✓: `ao keygen` (Ed25519 keypair generation), `ao genesis` (complete genesis block per WireFormat.md §6.1), `ao inspect` (binary DataItem → JSON/hex).

**Tests:** 52 tests total. All Phase 0E conformance vectors pass. Proptest VBC round-trips across full i64/u64 range. Round-trip serialization for every DataItem type. Sign/verify round-trips. Cross-compilation: ao-types verified for aarch64-unknown-linux-gnu; ao-crypto/ao-cli need C cross-compiler (deferred to GitHub Actions CI in Phase 2).

### Acceptance Criteria — all met

All conformance vectors pass. Genesis block binary round-trip produces identical bytes. Genesis block JSON round-trip produces identical binary. Fee arithmetic matches Phase 0D examples exactly.

---

## Phase 2: Single-Chain Recorder — TⒶ¹ (Weeks 11–20)

Build `ao-chain` and `ao-recorder`, plus complete CLI tools.

### Deliverables

**ao-chain** — [src/ao-chain/](src/ao-chain/) ✓: Genesis loading/validation with issuer signature verification and chain ID hash check. SQLite UTXO store (sequence ID → pubkey, amount, block, timestamp, status). Block construction with sequence numbering, hash chaining (PREV_HASH), fee deduction from shares_out, blockmaker signature. Assignment validation: participant signatures with timestamp ordering, UTXO availability and expiration check, recording bid ≥ chain fee rate, single-use key enforcement, deadline with late-recording rules, balance equation (givers = receivers + fee). Expiration sweep Mode 1 (hard cutoff). Refutation tracking. 10 unit tests + 7 integration tests.

**ao-recorder** — [src/ao-recorder/](src/ao-recorder/) ✓: Axum 0.8 HTTP server with lib + bin structure. `GET /chain/{id}/info` (chain metadata), `GET /chain/{id}/utxo/{seq_id}` (UTXO lookup), `GET /chain/{id}/blocks?from=&to=` (block range as JSON), `POST /chain/{id}/submit` (validate + record assignment), `GET /chain/{id}/events` (SSE block notifications with keep-alive), `GET /chain/{id}/ws` (WebSocket block notifications). TOML config (host, port, db_path, genesis_path, blockmaker_seed). Broadcast channel for real-time fan-out. 10 integration tests (including SSE and WebSocket).

**ao-cli** — [src/ao-cli/](src/ao-cli/) ✓: 9 commands — `ao keygen`, `ao genesis`, `ao inspect` (Phase 1), plus `ao balance` (UTXO query with coin display), `ao assign` (build assignment with iterative fee estimation), `ao accept` (sign + submit authorization), `ao refute` (build refutation DataItem), `ao history` (block range summary), `ao export` (blocks as JSON).

**Tests:** 82 tests total. Edge cases: expired UTXO rejection, double-spend rejection, key reuse rejection, timestamp ordering enforcement, multi-receiver assignment with fee convergence, two-block chain flow with UTXO state transitions, late recording allowed/rejected with refutation, before-deadline refutation bypass. HTTP API tests: chain info, UTXO lookup, block retrieval, assignment submission, invalid JSON, double-spend via API, SSE/WebSocket real-time notifications.

**Deployment** ✓: [Dockerfile](Dockerfile) (multi-stage, non-root, bookworm-slim). [ao-recorder.service](ao-recorder.service) (systemd hardened). [GitHub Actions CI](../.github/workflows/ci.yml) (build + test + clippy on x86_64, cross-build aarch64 with gcc-aarch64-linux-gnu).

**Remaining:** Multi-chain hosting. 72-hour Pi stress test.

### Acceptance Criteria

Two CLI users on different machines transfer shares through a single AOR. 72 hours on Pi without memory growth. 100K assignments validate from genesis in <10s on Pi 5. Late recording edge cases all correct.

---

## Phase 3: Vendor and Consumer Apps — AOS + AOE (Weeks 21–28)

React PWA with vendor and consumer views, using the API contract from Phase 0A.

### Deliverables

**Browser key management:** Ed25519 via Web Crypto API. Private keys in IndexedDB, encrypted per Phase 0C. Backup/import. QR code display.

**AOE (consumer):** Balance dashboard. Chain discovery (URL/QR). Assignment flow with fee review. SSE real-time updates. GPS vendor map.

**AOS (vendor):** Profile as separable items. Incoming assignment monitor. Share float display with limit warnings. Price card.

**Shared:** Transaction history. Settings. Offline queue. Service worker caching.

### Acceptance Criteria

Two phones complete assignment in <3s. PWA installs on iOS Safari and Android Chrome. Accurate GPS vendor position. Cached balances in airplane mode.

---

## Phase 4: Market Making — AOI + Exchange (Weeks 29–38)

Exchange agent infrastructure with automated trading. This is where most business-logic complexity lives.

### Deliverables

**AOI view:** Portfolio dashboard. Automated trading rules (bid/ask ladders, float-sensitive pricing, position limits). Order book. Auto-execution. ROI tracking.

**Exchange infrastructure:** Exchange listing as separable item (bid/ask for other issues). AOR publishes JSON index of issues and known exchange agents.

**Two-party exchange:** Alice sends CCC to Charlie; Charlie's AOI sends BCG to Alice. Two independent single-chain assignments. Charlie absorbs settlement risk.

**MQTT:** AOR publishes block notifications to per-chain topics. AOI subscribes. Optional `rumqttd` embedded broker.

**Referral fees:** Fee structures in assignment metadata. Net-of-fees display.

### Acceptance Criteria

Automated BCG trades without intervention. 5-AOI, 3-chain market reaches equilibrium within 200 transactions. CCC→BCG through Charlie in <10s. MQTT handles 100 msg/s on Pi.

---

## Phase 5: Validation and Trust — AOV (Weeks 39–44)

### Deliverables

**ao-validator:** Monitors AOR servers. Periodic rolled-up hash across monitored chains. Local storage + optional external anchor (Bitcoin OP_RETURN or equivalent). Alteration alerts via MQTT/webhook.

**Chain integrity API:** `GET /validate/{chain_id}` — last validated height, timestamp, anchor ref. AOE trust indicator.

**AOR cross-reference:** Validator endorsement in chain info responses.

**Vendor credentials:** Verifiable credential references as separable items (URL + content hash). Hash-match indicator in AOE. W3C DID compatibility considered.

### Acceptance Criteria

Detects simulated alteration within one poll interval. Rolled-up hash independently verifiable. 30 days on Pi without memory growth.

---

## Phase 6: Atomic Multi-Chain Exchange — TⒶ² (Weeks 45–54)

Full CAA (Conditional Assignment Agreement) protocol. Can be deferred indefinitely if Phase 4's exchange-agent model proves sufficient.

### Deliverables

**CAA in ao-types:** Agreement with ordered chain list, escrow period, per-chain terms, recording proof slots. State machine: proposed → signed → recording → binding → finalized, with timeout → expired.

**Escrow in ao-chain:** UTXO state: escrowed-pending-CAA. Deadline enforcement. Auto-release on timeout.

**CAA coordinator in ao-recorder:** Ouroboros recording: chain 1 escrow → proof to chain 2 → chain 2 escrow → binding proof back to chain 1. HTTP relay between AORs. MQTT notifications.

**Recovery:** Exponential backoff retries. Escrow release on deadline. No permanently locked shares.

**AOE flow:** "Give 1 BCG to Bob (costs 12 CCC via Charlie)" with progress indicator and failure explanation.

### Acceptance Criteria

Three-party two-chain CAA completes in <30s. Server failure causes correct escrow release. Chaos testing: no share loss or double-spend under random kills and partitions.

---

## Not Covered

**TⒶ³ (multiple competing recorders)** and **TⒶ⁴ (underwriters/error checkers)** — conceptual only in the 2018 docs. Would need Phase 0-equivalent specification work.

**Regulatory compliance** — commodity-backed tokens sit in an unclear space. Architecture targets gift card / loyalty point frameworks (most favorable category). Real deployment needs jurisdiction-specific counsel.

**Pilot deployment** — the roadmap produces working software. Finding the first community willing to try it is a business problem, not a software deliverable.
