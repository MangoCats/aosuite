# Development Roadmap

This is a living document, updated as development progresses. Phase descriptions will be revised as earlier phases reveal better approaches, and completed phases will be condensed to reflect what was actually built rather than what was planned.

Six phases over approximately 54 weeks, starting with architecture and specification before any code is written. Each phase produces a working, demonstrable system before the next begins.

## Technology Stack

| Component | Choice | Notes |
|-----------|--------|-------|
| Language | Rust (stable) | Memory safety for crypto, cross-compile to ARM. Core serialization signatures avoid `std`-only types (see §Design Note below). |
| Signatures | `ring` 0.17 | Switched from `ed25519-dalek` — see [lessons/wrong-test-vector.md](lessons/wrong-test-vector.md). |
| Hashes | SHA2-256 + BLAKE3 (`sha2`, `blake3`) | Mature, audited, `no_std`. SHA2-256 is default; BLAKE3 where explicitly specified. |
| Big integers | `num-bigint` + `num-rational` | Pure Rust, adequate for AO's scale. Benchmark fee calculation paths. |
| HTTP server | Axum 0.8+ | Stable, tokio ecosystem. |
| MQTT | `rumqttc` | Consider `rumqttd` embedded broker for simple deployments. |
| Storage | `rusqlite` | Synchronous, wrap with `spawn_blocking` if needed. |
| Client UI | React PWA (TypeScript) | Cross-platform, no app store, offline capable. |
| Client crypto | Web Crypto API (primary) | Ed25519 in Chrome, Edge, Firefox. `tweetnacl-js` as Safari fallback. |
| Wallet encryption | Argon2id + XChaCha20-Poly1305 | For private key storage in browser and CLI. |
| Testing | `cargo test` + `proptest` + conformance vectors | Property-based + hand-computed ground truth in JSON. |

### Design Note: `no_std` and Portability

The original roadmap specified `no_std` for core crates (`ao-types`, `ao-crypto`). This was dropped as a hard requirement because:

1. **No current or planned consumer needs it.** All Rust targets (AOR server, AOV validator, CLI tools) require `std`. The browser client uses JavaScript with Web Crypto API, not Rust compiled to WASM.
2. **The Meshtastic low-bandwidth goal is served by the compact wire format, not by running Rust on LoRa nodes.** A mesh relay forwards opaque bytes; it doesn't parse DataItems.
3. **The `no_std` + `alloc` feature-flag machinery adds ongoing maintenance cost** (conditional compilation on `serde_json`, platform-specific RNG injection, less-tested `num-bigint` paths) for no current benefit.

To preserve the *option* of future `no_std` extraction without paying that cost now, core serialization functions in `ao-types` keep their signatures free of `std`-only types: inputs are `&[u8]` + `usize`, outputs go to `&mut Vec<u8>` (which lives in `alloc`, not `std`), and return types are primitives or crate-local enums. The only `std`-bound module is `json.rs` (via `serde_json`), which would be feature-gated in a `no_std` build. If an embedded consumer emerges (hardware wallet, mesh validator), extracting a minimal `ao-wire` crate from these functions would be straightforward.

## Phase Overview

| Phase | Weeks | Deliverables | Status | Outstanding |
|-------|-------|-------------|--------|-------------|
| 0: Architecture & Specification | 1–4 | Specs, test vectors | ✓ | — |
| 1: Foundation | 5–10 | `ao-types` + `ao-crypto`, genesis CLI | ✓ | — |
| 2: Single-Chain Recorder (TⒶ¹) | 11–20 | `ao-chain` + `ao-recorder`, full CLI | ✓ | 72-hr Pi stress test; 100K-assignment Pi benchmark |
| 3: Vendor + Consumer Apps | 21–28 | React PWA with AOS + AOE views | ✓ | Two-device <3s assignment test; iOS/Android install test; Lighthouse PWA audit |
| 4: Market Making + Exchange | 29–38 | AOI view, exchange agents, MQTT | ✓ | 5-AOI 3-chain equilibrium sim; 100 msg/s MQTT on Pi; 24-hr stability test |
| 5: Validation + Trust (AOV) | 39–44 | Validator, anchors, credentials | ✓ | 30-day Pi memory stability test |
| 6: Atomic Multi-Chain (TⒶ²) | 45–54 | Full CAA escrow protocol | — | All deliverables |

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

**Recording fee:** `fee_shares = ceil(data_bytes × FEE_RATE_num × SHARES_OUT / FEE_RATE_den)`. All arbitrary-precision integer, division last, ceil rounds toward positive infinity. 5 worked examples.

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

**ao-types** — [src/ao-types/](src/ao-types/) ✓: VBC codec (signed/unsigned). DataItem binary + JSON codec. Type code registry (37 codes). BigInt/Rational encoding via `num-bigint`/`num-rational`. Timestamp type with signed i64 and 2126 design horizon. Recording fee arithmetic with ceil rounding. Separable item identification (`is_separable`). 42 tests (including 5 proptest property tests).

**ao-crypto** — [src/ao-crypto/](src/ao-crypto/) ✓: Ed25519 via `ring` 0.17 (switched from `ed25519-dalek` — see [lessons/wrong-test-vector.md](lessons/wrong-test-vector.md)). SHA2-256 and BLAKE3. Separable-item hash-substitution. Sign/verify DataItem pipeline per WireFormat.md §6.2. 13 tests. Key-never-reuse tracking deferred to Phase 2 UTXO layer (requires persistent state).

**ao-cli** — [src/ao-cli/](src/ao-cli/) ✓: `ao keygen` (Ed25519 keypair generation), `ao genesis` (complete genesis block per WireFormat.md §6.1), `ao inspect` (binary DataItem → JSON/hex).

**Tests:** 55 tests total. All Phase 0E conformance vectors pass. Proptest VBC round-trips across full i64/u64 range. Round-trip serialization for every DataItem type. Sign/verify round-trips. Cross-compilation: ao-types verified for aarch64-unknown-linux-gnu; ao-crypto/ao-cli need C cross-compiler (deferred to GitHub Actions CI in Phase 2).

### Acceptance Criteria — all met

All conformance vectors pass. Genesis block binary round-trip produces identical bytes. Genesis block JSON round-trip produces identical binary. Fee arithmetic matches Phase 0D examples exactly.

---

## Phase 2: Single-Chain Recorder — TⒶ¹ (Weeks 11–20)

Build `ao-chain` and `ao-recorder`, plus complete CLI tools.

### Deliverables

**ao-chain** — [src/ao-chain/](src/ao-chain/) ✓: Genesis loading/validation with issuer signature verification and chain ID hash check. SQLite UTXO store (sequence ID → pubkey, amount, block, timestamp, status). Block construction with sequence numbering, hash chaining (PREV_HASH), fee deduction from shares_out, blockmaker signature. Assignment validation: participant signatures with timestamp ordering, UTXO availability and expiration check, recording bid ≥ chain fee rate, single-use key enforcement, deadline with late-recording rules, balance equation (givers = receivers + fee). Expiration sweep Mode 1 (hard cutoff). Refutation tracking. 10 unit tests + 10 integration tests.

**ao-recorder** — [src/ao-recorder/](src/ao-recorder/) ✓: Axum 0.8 HTTP server with lib + bin structure. Multi-chain hosting: `GET /chains` (list hosted chains), `POST /chains` (create chain from genesis JSON at runtime). Per-chain endpoints: `GET /chain/{id}/info`, `GET /chain/{id}/utxo/{seq_id}`, `GET /chain/{id}/blocks?from=&to=`, `POST /chain/{id}/submit`, `GET /chain/{id}/events` (SSE), `GET /chain/{id}/ws` (WebSocket). Per-chain state (store, blockmaker key, broadcast channel) behind `RwLock<HashMap>`. TOML config supports single-chain (backward-compatible) and `[[chains]]` array for multi-chain startup, plus optional `data_dir` for file-backed dynamic chain creation. 14 integration tests.

**ao-cli** — [src/ao-cli/](src/ao-cli/) ✓: 9 commands — `ao keygen`, `ao genesis`, `ao inspect` (Phase 1), plus `ao balance` (UTXO query with coin display), `ao assign` (build assignment with iterative fee estimation), `ao accept` (sign + submit authorization), `ao refute` (build refutation DataItem), `ao history` (block range summary), `ao export` (blocks as JSON).

**Tests:** 102 tests total (42 ao-types + 13 ao-crypto + 21 ao-chain + 15 ao-recorder + 11 ao-validator). Edge cases: expired UTXO rejection, double-spend rejection, key reuse rejection, timestamp ordering enforcement, multi-receiver assignment with fee convergence, two-block chain flow with UTXO state transitions, late recording allowed/rejected with refutation, before-deadline refutation bypass. HTTP API tests: chain info, UTXO lookup, block retrieval, assignment submission, invalid JSON, double-spend via API, SSE/WebSocket real-time notifications.

**Deployment** ✓: [Dockerfile](Dockerfile) (multi-stage, non-root, bookworm-slim). [ao-recorder.service](ao-recorder.service) (systemd hardened). [GitHub Actions CI](../.github/workflows/ci.yml) (build + test + clippy on x86_64, cross-build aarch64 with gcc-aarch64-linux-gnu).

**Remaining:** 72-hour Pi stress test (requires hardware).

### Acceptance Criteria

Two CLI users on different machines transfer shares through a single AOR. 72 hours on Pi without memory growth. 100K assignments validate from genesis in <10s on Pi 5. Late recording edge cases all correct.

---

## Phase 3: Vendor and Consumer Apps — AOS + AOE (Weeks 21–28)

React PWA with vendor and consumer views, using the API contract from Phase 0A. Divided into sub-phases that front-load protocol-compatibility risk before building UI.

### Phase 3A: Protocol Simulation Harness (Week 21)

Rust simulation binary (`ao-sims`) that proves the full assignment flow end-to-end before introducing a language boundary. Spins up an ao-recorder in-process, creates a genesis chain, generates keypairs, builds/signs/submits assignments via HTTP, and verifies block responses. Produces reference request/response JSON fixtures for TypeScript conformance testing.

**Deliverable:** `sims/` — simulation binary exercising: genesis creation, single assignment, multi-receiver assignment, fee estimation, SSE block notification, error cases (double-spend, expired UTXO, bad signature). JSON fixtures written to `sims/fixtures/`.

**Acceptance:** All simulated assignments succeed against a live ao-recorder. Fixture files capture every request/response pair.

### Phase 3B: TypeScript Core Data Layer (Week 22)

Port `ao-types` to TypeScript as a standalone library with zero React dependency. Must pass all conformance vectors from `specs/conformance/vectors.json`.

**Deliverable:** `ao-pwa/src/core/` — VBC codec, DataItem JSON serialization, BigInt/Rational encoding (native JS BigInt), timestamp conversion, recording fee calculation (ceil division), separable item identification. Full conformance test suite in Vitest.

**Acceptance:** All vectors.json test cases pass. JSON round-trip produces identical output to `ao-types::json`.

### Phase 3C: TypeScript Crypto Layer + Wallet (Week 23)

Port `ao-crypto` signing pipeline to TypeScript using Web Crypto API. Encrypt/decrypt private keys with Argon2id (WASM) + XChaCha20-Poly1305.

**Deliverable:** `ao-pwa/src/crypto/` — Ed25519 keygen/sign/verify via Web Crypto API, SHA-256 via `crypto.subtle`, BLAKE3 via WASM, separable-item hash substitution, wallet encryption with Argon2id in Web Worker. IndexedDB storage for encrypted keys.

**Acceptance:** Sign a DataItem in TypeScript, submit to a running ao-recorder, get a 200 back. Encrypt/decrypt round-trip. RFC 8032 test vector passes.

### Phase 3D: API Client + Skeleton React UI (Week 24)

Connect to the recorder and display chain state. Vite + React 19 + TypeScript project scaffold.

**Deliverable:** `ao-pwa/` — Vite project with: fetch wrappers for all recorder endpoints, SSE/WebSocket wrappers, Zustand stores (wallet, chain, offline queue), routing (AOE/AOS mode toggle), key manager (generate, backup, import), chain info display, balance dashboard.

**Acceptance:** App runs in browser, connects to a local ao-recorder, displays chain info and UTXO balances. Key generation and encrypted storage works.

### Phase 3E: Assignment Flow + Vendor/Consumer Views (Weeks 25–27)

Full assignment flow with off-band negotiation, plus AOE consumer and AOS vendor views.

**Deliverables:**

**Assignment flow:** Builder, iterative fee estimator, off-band exchange (recorder-as-relay for MVP, QR as enhancement), sign + submit. Both giver and receiver sign.

**AOE (consumer):** Balance dashboard with SSE updates. Chain discovery (URL entry + QR scan). Assignment flow with fee review. Transaction history.

**AOS (vendor):** Incoming assignment monitor (SSE-driven). Share float display with expiry warnings. Price card. Vendor profile as separable items.

**Shared:** GPS vendor map (Leaflet/OpenStreetMap). Settings. QR scanner component.

**Acceptance:** Two devices complete an assignment in <3s from submit to SSE confirmation.

### Phase 3F: PWA Polish — Offline, Install, Service Worker (Week 28)

**Deliverable:** PWA manifest + icons, service worker via vite-plugin-pwa (Workbox), offline assignment queue with Background Sync, cached balances in IndexedDB, Lighthouse PWA audit.

**Acceptance:** PWA installs on iOS Safari and Android Chrome. Cached balances display in airplane mode. Fully-signed assignments queued offline submit on reconnect.

---

## Phase 4: Market Making — AOI + Exchange (Weeks 29–38)

Exchange agent infrastructure with automated trading. This is where most business-logic complexity lives. All deliverables are in `2026/src/` — the sims framework is an independent consumer of these products and maintained separately.

### Phase 4A: MQTT Block Publishing (Weeks 29–30)

Add MQTT support to ao-recorder for efficient real-time block event delivery to exchange agents.

**Deliverables:**

**rumqttc integration:** Add `rumqttc` dependency to ao-recorder. Optional `[mqtt]` section in TOML config: `broker_url`, `client_id`, `topic_prefix` (default `ao/chain`), optional TLS paths.

**Block publication:** After block construction in `submit_assignment`, publish `BlockInfo` JSON to `{topic_prefix}/{chain_id}/blocks`. Non-blocking — MQTT failure does not fail the HTTP response.

**Graceful degradation:** If MQTT broker is unavailable or not configured, recorder runs normally (SSE/WebSocket still work). Log warning on connection failure, retry with exponential backoff.

**Acceptance:** MQTT-connected exchange agent receives block notifications within 100ms. 100 msg/s sustained on localhost.

### Phase 4B: Standalone Exchange Agent (Weeks 31–32)

Extract exchange agent logic from sims into a reusable, config-driven daemon that can run independently against live recorders.

**Deliverables:**

**`ao-exchange` crate:** [src/ao-exchange/](src/ao-exchange/) — lib + bin. TOML config specifying: chains (recorder URL, chain ID), trading pairs (chain A → chain B, rate, spread, position limits), key files (encrypted seeds per chain).

**Trading rules engine:** Configurable bid/ask spreads, float-sensitive pricing (wider spreads when inventory is low), position limits per chain, minimum/maximum trade sizes.

**Agent loop:** Monitor chains via SSE (MQTT when available). Scan for incoming shares (UTXO polling or SSE-driven). Match against trading rules. Execute two-leg trades automatically. Log all executions.

**Position management:** Track current holdings per chain. Enforce position limits. Auto-rebalance when holdings drift outside configured bands.

**CLI:** `ao-exchange run config.toml` — starts the daemon. `ao-exchange status config.toml` — shows current positions and pending trades.

**Acceptance:** Exchange agent runs unattended, executes BCG↔CCC trades automatically. Handles concurrent requests. Recovers from recorder restarts.

### Phase 4C: AOI Investor View in PWA (Weeks 33–34)

Add investor view to the React PWA for monitoring and configuring exchange agents.

**Deliverables:**

**Investor view mode:** Add 'investor' to the view toggle in store and Header. AOI view shows multi-chain portfolio, not single-chain detail.

**Portfolio dashboard:** Connect to multiple recorders (configured URLs). Display holdings table: chain symbol, shares held, coin value, % of float, expiry status.

**Exchange status:** Active trading pairs with current rates and position levels. Spread indicators. Recent execution log.

**Trade history:** Chronological log of all exchange-mediated trades. Filter by chain, counterparty, time range.

**Acceptance:** AOI view displays accurate multi-chain portfolio from 2+ recorders. Updates in real-time via SSE.

### Phase 4D: Referral Fees + Exchange Discovery (Weeks 35–36)

Protocol extensions for fee structures and exchange agent discovery.

**Deliverables:**

**Referral fees:** Optional `REFERRAL_FEE` item in PARTICIPANT containers — specifies a fraction of the recording fee directed to a referral key. Net-of-fees display in ConsumerView.

**Exchange index API:** Extend `GET /chains` response with optional `exchange_agents` array listing registered agents and their trading pairs. Exchange agent registers with recorder on startup via new `POST /chain/{id}/exchange-agent` endpoint.

**On-chain exchange listing:** EXCHANGE_LISTING separable item type (code 37) — a container with chain symbols and exchange rates, attached to assignments for transparency and auditability.

**Acceptance:** Consumer can discover available exchange agents for a chain. Referral fees deducted correctly and visible in transaction detail.

### Phase 4E: Acceptance Testing + Equilibrium (Weeks 37–38)

Full integration testing against all Phase 4 acceptance criteria.

**Deliverables:**

**Equilibrium simulation:** 5-AOI agents, 3 chains (BCG, CCC, MMF), 200 random consumer transactions. Verify market reaches equilibrium (prices stabilize, exchange agents maintain inventory).

**Cross-chain latency test:** CCC→BCG through Charlie in <10s consistently (p99).

**MQTT throughput:** 100 msg/s sustained on Pi 5 with 3 chains and 5 exchange agents.

**Long-run stability:** 24-hour simulation without memory growth or deadlock.

### Acceptance Criteria

Automated BCG trades without intervention. 5-AOI, 3-chain market reaches equilibrium within 200 transactions. CCC→BCG through Charlie in <10s. MQTT handles 100 msg/s on Pi.

---

## Phase 5: Validation and Trust — AOV (Weeks 39–44) — ✓ 2026-03-07

Specification: [specs/ValidationAndTrust.md](specs/ValidationAndTrust.md) ✓ 2026-03-07

### Design Principle: Built-In, Not Required

External anchors, W3C credential references, and validator endorsements are **supported by all relevant software modules** but **never required for operation**. A chain with zero validators, zero anchors, and zero credentials is a valid, functional chain. These features reduce risk when available — the software makes them easy to adopt, surfaces their results clearly, and degrades gracefully when absent. No user action is needed to benefit from trust signals published by others (validators, anchor operators, credential issuers).

This principle applies across all modules:

| Module | Trust feature supported | User activation required? |
|--------|------------------------|--------------------------|
| **ao-types** | Type codes for validator attestations (64–68), credential references (38–39) | No — codes exist in registry, parsed automatically |
| **ao-chain** | Credential references in vendor profiles; validator attestation containers | No — stored if present, ignored if absent |
| **ao-recorder** | Validator endorsement cache in chain info; credential refs in blocks | No — endorsements served when validators are configured; credentials recorded when submitted |
| **ao-validator** | Rolled-up hash verification; file-based anchoring; HTTP attestation API | Operator configures chains to monitor; anchoring is opt-in per deployment |
| **ao-pwa** | Trust indicator display (validator dots, credential hash-match); W3C VC URL fetch | No — indicators appear automatically when data is present; hidden when absent |
| **ao-cli** | Inspect/export validator attestations and credential refs | No — displayed when present in block data |

### Deliverables

**5A: Specification** — [specs/ValidationAndTrust.md](specs/ValidationAndTrust.md) ✓: Chain integrity verification (rolled-up hash), validator protocol (polling, state transitions, HTTP API), external anchoring (file backend, pluggable architecture), on-chain type codes (validator 64–68, credential 38–39), vendor credentials (URL + content hash, separable), W3C VC/DID compatibility mapping, trust indicator display spec, scope boundaries.

**5B: ao-validator crate** — [src/ao-validator/](src/ao-validator/) ✓: Validator daemon with periodic polling. Block verification against rolled-up hash per ValidationAndTrust.md §1. Chain status tracking (ok / unreachable / alert). SQLite state store with validated height and rolled hash per chain. Recorder HTTP client for block fetching. 11 tests.

**5C: Validator HTTP API** — ✓ `GET /validate` (all chains), `GET /validate/{chain_id}` (single chain: validated height, rolled hash, status, alert message, latest anchor). JSON responses per ValidationAndTrust.md §2.3.

**5D: Alert system** — ✓ Structured logging (tracing) for all state transitions. Optional webhook (HTTP POST, fire-and-forget) for alteration/unreachable/recovered events per ValidationAndTrust.md §2.5.

**5E: External anchoring** — ✓ File-based anchor backend: append-only JSON lines, `publish()` and `verify()` operations. Anchor records include chain ID, height, rolled hash, timestamp, backend-specific locator. Pluggable backend trait for future Bitcoin OP_RETURN, transparency log, or IPFS backends. Anchoring is operator-invoked, not automatic — frequency is a deployment decision per ValidationAndTrust.md §3.4.

**5F: Recorder validator integration** — ✓ `poll_validators()` background task in ao-recorder polls configured validator endpoints, caches endorsements per chain, serves them in `GET /chain/{id}/info` responses. Cache is best-effort — poisoned lock or unreachable validator results in stale/absent data, never a recorder failure.

**5G: Vendor credentials on-chain** — ✓ `CREDENTIAL_REF` (code 38) and `CREDENTIAL_URL` (code 39) as separable items in `VENDOR_PROFILE`. Structure: URL + SHA2-256 content hash. Client-side verification: fetch URL, compare hash. No JSON-LD parsing, no DID resolution, no issuer signature verification — hash-match only per ValidationAndTrust.md §5.3.

**5H: PWA trust indicators** — ✓ Validator endorsement display: green (verified, ≤1 block behind), amber (lagging), red (alert/unreachable). Credential hash-match indicator: green check (match), red warning (mismatch/unreachable), grey dash (none). `TrustIndicator` component in `ChainDetail`, conditionally rendered when data present, hidden when absent.

**5I: Integration testing** — ✓ Verifier tests: single block round-trip, multi-block chain verification, tampered hash detection. End-to-end: validator polls recorder, detects simulated alteration within one poll interval, alert fires.

### Acceptance Criteria

Detects simulated alteration within one poll interval. Rolled-up hash independently verifiable. File anchor round-trip (publish + verify). Validator endorsements appear in chain info when configured, absent when not. Credential hash-match works in PWA. All trust indicators degrade gracefully to hidden/grey when upstream data is absent. 30 days on Pi without memory growth.

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
