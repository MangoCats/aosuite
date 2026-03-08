# Development Roadmap

This is a living document. Completed phases are condensed to reflect what was actually built. Active work is tracked below.

Phases 0–6 (54 weeks) produced working protocol software: wire format, crypto, chain engine, recorder, PWA, exchange agents, validator, and atomic multi-chain escrow. Post-protocol work (N1–N11) closed deployment gaps identified by three deployment stories. Current work tracks pilot readiness (Tier 1), adoption features (Tier 2), maturity (Tier 3), and strategic items (Tier 4).

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
| Client crypto | Web Crypto API | Ed25519 in Chrome 113+, Edge, Firefox 129+, Safari 17+. No fallback library needed. |
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
| 6: Atomic Multi-Chain (TⒶ²) | 45–54 | Full CAA escrow protocol | ✓ | Chaos testing with random kills; PWA CAA UI (deferred) |

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

**ao-types** — [src/ao-types/](src/ao-types/) ✓: VBC codec (signed/unsigned). DataItem binary + JSON codec. Type code registry (56 codes). BigInt/Rational encoding via `num-bigint`/`num-rational`. Timestamp type with signed i64 and 2126 design horizon. Recording fee arithmetic with ceil rounding. Separable item identification (`is_separable`). 42 tests (including 5 proptest property tests).

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

**ao-recorder** — [src/ao-recorder/](src/ao-recorder/) ✓: Axum 0.8 HTTP server with lib + bin structure. Multi-chain hosting: `GET /chains` (list hosted chains), `POST /chains` (create chain from genesis JSON at runtime). Per-chain endpoints: `GET /chain/{id}/info`, `GET /chain/{id}/utxo/{seq_id}`, `GET /chain/{id}/blocks?from=&to=`, `POST /chain/{id}/submit`, `POST /chain/{id}/refute` (record refutation), `GET /chain/{id}/events` (SSE), `GET /chain/{id}/ws` (WebSocket). Per-chain state (store, blockmaker key, broadcast channel) behind `RwLock<HashMap>`. TOML config supports single-chain (backward-compatible) and `[[chains]]` array for multi-chain startup, plus optional `data_dir` for file-backed dynamic chain creation. 14 integration tests.

**ao-cli** — [src/ao-cli/](src/ao-cli/) ✓: 9 commands — `ao keygen`, `ao genesis`, `ao inspect` (Phase 1), plus `ao balance` (UTXO query with coin display), `ao assign` (build assignment with iterative fee estimation), `ao accept` (sign + submit authorization), `ao refute` (submit refutation to recorder), `ao history` (block range summary), `ao export` (blocks as JSON).

**Tests:** 34 tests at Phase 2 completion (ao-types, ao-crypto, ao-chain, ao-recorder); see Phase 6 for cumulative totals. Edge cases: expired UTXO rejection, double-spend rejection, key reuse rejection, timestamp ordering enforcement, multi-receiver assignment with fee convergence, two-block chain flow with UTXO state transitions, late recording allowed/rejected with refutation, before-deadline refutation bypass. HTTP API tests: chain info, UTXO lookup, block retrieval, assignment submission, invalid JSON, double-spend via API, SSE/WebSocket real-time notifications.

**Deployment** ✓: [Dockerfile](Dockerfile) (multi-stage, non-root, bookworm-slim). [ao-recorder.service](ao-recorder.service) (systemd hardened). [GitHub Actions CI](../.github/workflows/ci.yml) (build + test + clippy on x86_64, cross-build aarch64 with gcc-aarch64-linux-gnu).

**Remaining:** 72-hour Pi stress test (requires hardware).

### Acceptance Criteria

Two CLI users on different machines transfer shares through a single AOR. 72 hours on Pi without memory growth. 100K assignments validate from genesis in <10s on Pi 5. Late recording edge cases all correct.

---

## Phase 3: Vendor and Consumer Apps — AOS + AOE (Weeks 21–28) — ✓

React PWA with vendor and consumer views. Front-loaded protocol-compatibility risk (simulation harness, TypeScript core, crypto layer) before building UI.

### Deliverables

**3A: Protocol Simulation Harness** — `sims/` ✓: Rust simulation binary exercising genesis, assignments, SSE, error cases against a live ao-recorder. Produced reference JSON fixtures for TypeScript conformance.

**3B: TypeScript Core Data Layer** — `ao-pwa/src/core/` ✓: VBC codec, DataItem JSON serialization, BigInt/Rational encoding (native JS BigInt), timestamp conversion, recording fee calculation. All conformance vectors pass.

**3C: TypeScript Crypto Layer + Wallet** — `ao-pwa/src/crypto/` ✓: Ed25519 keygen/sign/verify via Web Crypto API, SHA-256, BLAKE3 (WASM), separable-item hash substitution, wallet encryption (Argon2id in Web Worker), IndexedDB key storage.

**3D: API Client + Skeleton React UI** — `ao-pwa/` ✓: Vite + React 19 + TypeScript. Fetch wrappers for all recorder endpoints, SSE/WebSocket wrappers, Zustand stores (wallet, chain, offline queue), routing, key manager, chain info display, balance dashboard.

**3E: Assignment Flow + Vendor/Consumer Views** — ✓: Builder with iterative fee estimator, off-band exchange (recorder-as-relay, QR enhancement). AOE: balance dashboard with SSE, chain discovery, assignment flow, transaction history. AOS: incoming assignment monitor, share float display, price card, vendor profile. Shared: GPS vendor map (Leaflet/OpenStreetMap), settings, QR scanner.

**3F: PWA Polish** — ✓: PWA manifest + icons, service worker (vite-plugin-pwa/Workbox), offline assignment queue with Background Sync, cached balances in IndexedDB.

### Acceptance Criteria

Two devices complete an assignment in <3s from submit to SSE confirmation. PWA installs on iOS Safari and Android Chrome. Cached balances display in airplane mode. Remaining: two-device latency test, iOS/Android install test, Lighthouse audit (all require hardware).

---

## Phase 4: Market Making — AOI + Exchange (Weeks 29–38) — ✓

Exchange agent infrastructure with automated trading. All deliverables in `2026/src/` — sims framework is an independent consumer.

### Deliverables

**4A: MQTT Block Publishing** — ✓: `rumqttc` integration in ao-recorder. Optional `[mqtt]` config section. Block publication to `{topic_prefix}/{chain_id}/blocks` after block construction. Graceful degradation — recorder runs normally without MQTT.

**4B: Standalone Exchange Agent** — [src/ao-exchange/](src/ao-exchange/) ✓: Config-driven daemon with trading rules engine (configurable spreads, float-sensitive pricing, position limits). Agent loop monitors chains via SSE, matches incoming shares against rules, executes two-leg trades. CLI: `ao-exchange run config.toml`, `ao-exchange status config.toml`.

**4C: AOI Investor View** — ✓: Investor mode in PWA with multi-chain portfolio dashboard, exchange status with current rates, trade history with filters.

**4D: Referral Fees + Exchange Discovery** — ✓: Optional `REFERRAL_FEE` in PARTICIPANT containers. Exchange index API (`GET /chains` with `exchange_agents`, `POST /chain/{id}/exchange-agent`). `EXCHANGE_LISTING` separable item (code 37).

**4E: Acceptance Testing** — ✓: 5-AOI, 3-chain equilibrium simulation. Cross-chain latency test. MQTT throughput test. 24-hour stability run.

### Acceptance Criteria

Automated BCG trades without intervention. 5-AOI, 3-chain market reaches equilibrium within 200 transactions. CCC→BCG through Charlie in <10s. MQTT handles 100 msg/s on Pi. Remaining: Pi MQTT throughput test, 24-hr stability test (require hardware).

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

## Phase 6: Atomic Multi-Chain Exchange — TⒶ² (Weeks 45–54) — ✓ 2026-03-07

Specification: [specs/AtomicExchange.md](specs/AtomicExchange.md) ✓ 2026-03-07

### Deliverables

**6A: Specification** — [specs/AtomicExchange.md](specs/AtomicExchange.md) ✓: CAA wire format (10 new type codes 69–78 including COORDINATOR_BOND, all inseparable), ouroboros recording protocol for N chains, escrow state machine (proposed → signed → recording → binding → finalized / expired), escrow rules (deadline enforcement, auto-release, no partial binding), recording proof structure and verification, binding submission protocol, client-side coordination, timeout and recovery, idempotent submission.

**6B: CAA type codes in ao-types** — ✓: `CAA` (69), `CAA_COMPONENT` (70), `CHAIN_REF` (71), `ESCROW_DEADLINE` (72), `CHAIN_ORDER` (73), `RECORDING_PROOF` (74), `CAA_HASH` (75), `BLOCK_REF` (76), `BLOCK_HEIGHT` (77), `COORDINATOR_BOND` (78). Size categories, type names, separability tests all updated.

**6C: Escrow in ao-chain** — [src/ao-chain/src/caa.rs](src/ao-chain/src/caa.rs) ✓: `Escrowed` UTXO status. `caa_escrows` and `caa_utxos` SQL tables. `validate_caa_submit()` (full CAA validation: component matching, UTXO checks, per-component signatures, overall signatures, recording proof verification, balance equation). `validate_caa_bind()` (binding proof validation). `run_escrow_sweep()` (auto-release expired escrows). `compute_caa_hash()` (deterministic canonical hash). Error variants: `UtxoEscrowed`, `InvalidCaa`, `CaaNotFound`, `CaaAlreadyExists`, `CaaExpired`. 6 unit tests.

**6D: CAA recorder endpoints** — ✓: `POST /chain/{id}/caa/submit` (escrow recording with idempotent re-submission), `POST /chain/{id}/caa/bind` (finalize with binding proofs), `GET /chain/{id}/caa/{caa_hash}` (escrow status query). Recording proof generation signed by blockmaker key. Transaction-safe escrow with rollback on failure. `known_recorders` config for cross-chain proof verification.

**6E: CAA coordinator in ao-exchange** — [src/ao-exchange/src/caa.rs](src/ao-exchange/src/caa.rs) ✓: `execute_caa()` async function orchestrating the full ouroboros sequence. Builds signed CAA with per-component and overall signatures. Iterative fee convergence. Submits to chains in order, collects recording proofs, submits binding proofs back to earlier chains. 6 unit tests.

**6F: CLI commands** — ✓: `ao caa-status` (query CAA escrow status on a chain).

**6G: Integration tests** — ✓: CAA submit and status query across two independent chains with escrowed UTXO verification and idempotent re-submission. 1 integration test.

**Tests:** 192 Rust tests (42 ao-types + 13 ao-crypto + 44 ao-chain unit + 12 ao-chain integration + 21 ao-exchange + 21 ao-recorder unit + 23 ao-recorder integration + 11 ao-validator + 5 ao-relay) + 262 PWA tests = 454 total. 0 clippy warnings.

### Acceptance Criteria

Escrowed shares cannot be spent in regular assignments ✓. Expired escrow correctly returns shares to Unspent ✓. Idempotent CAA submission ✓. Recording proof verification against known recorder keys ✓. Remaining: chaos testing with random kills and partitions; PWA CAA UI (deferred — Phase 4's exchange-agent model sufficient for end users).

---

## Next Steps: Deployment-Driven Gaps

Phases 0–6 produced working protocol software. The three updated deployment stories — [Tourism Vendors](../docs/html/TourismVendors.html), [Island UBI](../docs/html/IslandUBI.html), and [Farming Cooperatives](../docs/html/FarmingCooperatives.html) — identify concrete gaps between what's built and what a pilot deployment requires. Gaps are prioritized by how many stories they unblock and how close the existing code is to closing them.

### N1: Exchange Agent Auto-Trade Loop — *all three stories* ✓

The ao-exchange daemon now supports autonomous trading via a request→deposit→execute flow:

1. **Trade Request API** (`POST /trade`): Consumer requests a trade specifying sell/buy symbols and amount. Agent generates fresh deposit key (buy chain) and receive key (sell chain), validates against pair limits and inventory, returns both key pairs with seeds. Trade requests expire after configurable TTL (default 5 minutes).

2. **Deposit Detection**: Polling loop (`check_deposits()`) tracks `next_seq_id` per chain. When new UTXOs appear, matches pubkeys against pending trade deposits. Registered UTXOs are immediately available for the reverse-leg execution.

3. **Auto-Execution**: Matched deposits trigger `execute_trade()` on the sell chain, sending shares to the consumer's receive key with an attached EXCHANGE_LISTING (see N6). Trades are logged with full context. Insufficient inventory is rejected at request time.

4. **HTTP Status API** (`GET /status`): Returns current trading pairs, positions, and pending trade count. Exchange daemon serves Axum HTTP alongside the polling loop.

**Implementation:** [trade.rs](src/ao-exchange/src/trade.rs) (PendingTrade, TradeManager), [engine.rs](src/ao-exchange/src/engine.rs) (`request_trade`, `check_deposits`), [main.rs](src/ao-exchange/src/main.rs) (Axum HTTP + polling loop). 3 trade manager tests + 4 transfer tests.

**Remaining:** SSE-driven detection (upgrade from polling), MQTT integration, multi-recorder failover.

**Unblocks:** Charlie bridging CCC↔BCG (Tourism), Mako bridging ENRA↔TGS (UBI), Wanjiku bridging MPC↔M-Pesa (Cooperative).

### N2: PWA End-to-End Assignment Flow — *Tourism + UBI* ✓

The PWA now supports the full browser-based sign-and-submit flow with persistent wallet, UTXO discovery, and real-time vendor notification:

1. **Wallet Persistence**: Ed25519 seed, public key, and label stored in localStorage. Generate new wallet or import existing seed. Survives page refreshes and browser restarts. Recorder URL also persisted.

2. **UTXO Balance Scanner**: Consumer clicks "Scan UTXOs" to discover unspent UTXOs on the selected chain matching their wallet pubkey. Displays total balance and individual UTXOs with a picker for multi-UTXO wallets.

3. **Transfer with Change**: Consumer selects UTXO, enters amount (or blank for full UTXO minus fee). Two-receiver assignment: recipient + change key. 3-round iterative fee convergence. Optional recipient pubkey field (generates fresh key if blank).

4. **Vendor SSE Monitor**: VendorView subscribes to SSE block events on the selected chain. Incoming blocks display in real-time with block height, assignment count, and seq range. Genesis creation collapsed into an expandable section.

**Implementation:** [useStore.ts](src/ao-pwa/src/store/useStore.ts) (localStorage persistence), [ConsumerView.tsx](src/ao-pwa/src/components/ConsumerView.tsx) (wallet + UTXO scanner + transfer), [VendorView.tsx](src/ao-pwa/src/components/VendorView.tsx) (SSE monitor + genesis creator). 68 PWA tests passing, TypeScript clean, 70KB gzipped production build.

**Remaining:** Two-device <3s latency test (requires hardware), iOS/Android PWA install test, Lighthouse audit.

**Unblocks:** Alice paying Bob at Sandy Ground (Tourism), Nalu paying Tia at her store (UBI).

### N3: QR Code Chain Discovery — *Tourism + UBI* ✓

QR-based chain discovery for tourist/consumer onboarding:

1. **QR Code Display**: `QrCode` component renders chain info URLs (`{recorder}/chain/{id}/info`) as QR codes on canvas. Available in ChainDetail — vendors show this on signage, laminated cards, or screen. Uses `qrcode` npm package.

2. **QR Scanner**: `QrScanner` component uses `getUserMedia` camera access + `jsqr` library for real-time QR decode. Full-screen overlay with video preview. Available in Settings panel.

3. **Auto-Discovery**: Scanned chain URLs are parsed to extract recorder base URL and chain ID. App automatically connects to the recorder and selects the chain. Also handles plain recorder URLs.

**Implementation:** [QrCode.tsx](src/ao-pwa/src/components/QrCode.tsx), [QrScanner.tsx](src/ao-pwa/src/components/QrScanner.tsx), [Settings.tsx](src/ao-pwa/src/components/Settings.tsx) (scan trigger + URL parsing), [ChainDetail.tsx](src/ao-pwa/src/components/ChainDetail.tsx) (QR display toggle).

**Remaining:** Print-optimized QR generation for physical signage, deep-link PWA install flow.

**Unblocks:** Tourist onboarding on launch ride (Tourism), customer onboarding at Tia's store (UBI).

### N4: PWA Offline Assignment Queue — *UBI + Cooperative* ✓

Offline transaction support for low-connectivity environments:

1. **Service Worker**: Upgraded cache strategy — stale-while-revalidate for app shell, immutable caching for hashed assets, network-only for API calls. App loads fully offline after first visit.

2. **IndexedDB Queue**: `offlineQueue.ts` module stores signed authorizations in IndexedDB when the recorder is unreachable. Schema: chain ID, recorder URL, authorization JSON, queued timestamp, status (pending/submitted/failed).

3. **Auto-Submit**: `flushPending()` retries queued assignments when connectivity returns. Triggered by `online` event listener and 30-second polling interval. Successfully submitted assignments marked as complete; server errors marked as failed with error message.

4. **Queue Indicator**: Yellow banner in ConsumerView shows count of queued assignments and connectivity status. Transfer flow catches network errors (`TypeError` from fetch) and queues automatically.

**Implementation:** [sw.js](src/ao-pwa/public/sw.js) (service worker), [offlineQueue.ts](src/ao-pwa/src/core/offlineQueue.ts) (IndexedDB queue), [ConsumerView.tsx](src/ao-pwa/src/components/ConsumerView.tsx) (queue integration + indicator).

**Remaining:** Background Sync API integration (Chrome-only, progressive enhancement), cached balance display from IndexedDB.

**Unblocks:** Tia's store on Likiep during satellite outages (UBI), Ouma recording deliveries at the collection point with spotty Safaricom coverage (Cooperative).

### N5: Vendor Profile + Location Beacon — *Tourism* ✓

Vendor discovery with GPS location and map display:

1. **Vendor Profile API**: `POST /chain/{id}/profile` and `GET /chain/{id}/profile` endpoints on ao-recorder. In-memory storage (`VendorProfile` struct with optional name, description, lat, lon). Profiles included in `GET /chains` listing via `vendor_profile` field.

2. **Profile Editor**: VendorView includes a profile editor with business name, description, and GPS coordinates. "GPS" button uses `navigator.geolocation` for one-tap location capture. Saved to recorder via POST.

3. **Vendor Map**: ConsumerView displays a Leaflet/OpenStreetMap map showing all chains with vendor profiles that include lat/lon. Circle markers with popup showing symbol and name. Map appears automatically when location data is available, hidden otherwise.

**Implementation:** [lib.rs](src/ao-recorder/src/lib.rs) (`VendorProfile` struct, profile endpoints, chain listing integration), [VendorMap.tsx](src/ao-pwa/src/components/VendorMap.tsx) (Leaflet map), [VendorView.tsx](src/ao-pwa/src/components/VendorView.tsx) (profile editor), [ConsumerView.tsx](src/ao-pwa/src/components/ConsumerView.tsx) (map display), [client.ts](src/ao-pwa/src/api/client.ts) (profile API methods).

**Remaining:** Profile persistence across recorder restarts (currently in-memory only), vendor profile as on-chain VENDOR_PROFILE separable item.

**Unblocks:** Tourist discovering Bob's grill, Patrice's jewelry, Lucia's kayaks on the beach (Tourism).

### N6: EXCHANGE_LISTING On-Chain Structure — *all three stories* ✓

**EXCHANGE_LISTING children defined and implemented:**
- `CHAIN_SYMBOL` (21): counterpart chain symbol (UTF-8 bytes)
- `AMOUNT` (6): counterpart share amount (VBC-encoded BigInt)
- `NOTE` (32): agent label (UTF-8 bytes)
- Rate is implicit from the assignment's sell amount and the listing's counterpart amount.

`build_exchange_listing()` in [transfer.rs](src/ao-exchange/src/transfer.rs) constructs the separable container. `execute_trade()` attaches it to every agent-initiated assignment via the new `separable_items` parameter on `execute_transfer()` and `build_assignment()`.

**Registration protocol extended:** `ExchangeAgentEntry` now includes `contact_url` (trade request endpoint), `registered_at` (set by recorder on POST), and `ttl` (default 3600s). `ExchangePairEntry` includes `spread`, `min_trade`, `max_trade`. Expired agents are filtered from `GET /chains` listings and cleaned up on each registration POST. PWA TypeScript interfaces updated to match.

**Tests:** 4 transfer tests (listing structure, separability, assignment with/without separable items) + 1 recorder integration test (extended registration fields).

**Unblocks:** Transparent exchange pricing for tourists (Tourism), tax audit trail for Tia's remittances (UBI), cooperative accounting for middleman price verification (Cooperative).

### N7: Cooperative Metadata Conventions — *Cooperative* ✓

Convention document specifying how cooperatives encode structured metadata using existing AO separable types:

1. **NOTE (type 32) key:value format**: `type:delivery`, `crop:tomatoes`, `weight_kg:180`, `grade:A`, `lot:2026-W10-012`, `location:Riuki Collection Point`. Human-readable, machine-parseable, one field per line.

2. **Record types**: Delivery (farmer→cooperative), sale (cooperative→buyer), cost allocation (expense distribution), advance/credit (pre-season advances).

3. **DESCRIPTION (type 34)**: Free-text extended notes — inspector comments, damage assessments, quality observations.

4. **DATA_BLOB (type 33)**: Binary attachments with MIME-type prefix — photos of deliveries, weighbridge receipts. Separable: binary content can be stripped while preserving hash for verification.

5. **Provenance chain**: Lot identifiers linking delivery and sale records across assignments, combined with EXCHANGE_LISTING for cross-chain provenance.

**Specification:** [specs/CooperativeMetadata.md](specs/CooperativeMetadata.md)

**Unblocks:** Structured accounting for Riuki Cooperative (Cooperative), credit-building from documented production history (Cooperative).

### N8: Binary Attachments with MIME Metadata — *Cooperative + Tourism* ✓

Photo and document attachments on assignments, using the existing DATA_BLOB (type 33) separable item with standardized MIME metadata.

#### Wire Format

DATA_BLOB payload: `[MIME type as UTF-8, NUL-terminated][raw binary content]`. Examples: `image/jpeg\0<JPEG bytes>`, `application/pdf\0<PDF bytes>`. NUL delimiter is unambiguous (MIME types are ASCII). The entire DATA_BLOB is separable — when stripped, only the SHA-256 hash remains on-chain.

#### Recorder Blob Store

1. **Content-addressed storage**: `BlobStore` struct in [blob.rs](src/ao-recorder/src/blob.rs). Blobs stored at `data_dir/blobs/{sha256hex}` with atomic write (temp file + rename). Idempotent — re-uploading the same content returns the same hash without duplicating files.

2. **Upload endpoint**: `POST /chain/{id}/blob` accepts `application/octet-stream` body. Validates MIME delimiter, enforces MIME allowlist (`image/*`, `application/pdf`), checks size limit (configurable, default 5 MB), enforces per-chain storage quota (configurable, default 100 MB). Returns `{"hash": "..."}`.

3. **Retrieval endpoint**: `GET /chain/{id}/blob/{hash}` returns raw content with `Content-Type` from MIME prefix. Security headers: `X-Content-Type-Options: nosniff`, `Content-Security-Policy: default-src 'none'; img-src 'self'; style-src 'none'; script-src 'none'`, `Content-Disposition: inline` (images) or `attachment` (other). `Cache-Control: public, max-age=31536000, immutable`. Chain isolation enforced — blobs only readable by the chain that uploaded them.

4. **Error handling**: `BlobError` enum with `TooLarge`, `NoMimeDelimiter`, `InvalidMime`, `MimeNotAllowed`, `QuotaExceeded`, `IoError`, `InvalidHash` variants. Maps to HTTP status codes (413, 400, 404, 500).

#### PWA Upload UI

1. **AttachmentPicker component**: [AttachmentPicker.tsx](src/ao-pwa/src/components/AttachmentPicker.tsx). File picker with `accept="image/*,application/pdf"` and `capture="environment"` for mobile camera. Image thumbnail previews via `URL.createObjectURL`. Remove button per attachment. Configurable max files (default 5).

2. **Client-side compression**: Images over 1 MB compressed via `OffscreenCanvas` to max 2048px longest edge. WebP preferred, JPEG fallback. Implemented in [blob.ts](src/ao-pwa/src/core/blob.ts) `compressImage()`.

3. **Two-phase upload**: In ConsumerView's transfer flow, blobs are uploaded to the recorder via `client.uploadBlob()` before submitting the assignment. Attachments cleared on successful transfer.

4. **BlobViewer component**: [BlobViewer.tsx](src/ao-pwa/src/components/BlobViewer.tsx). Fetches blob by hash, renders inline image or download link. Cleans up object URLs on unmount.

5. **Blob utilities**: [blob.ts](src/ao-pwa/src/core/blob.ts) — `parseBlobPayload()`, `buildBlobPayload()` for MIME+NUL+content wire format encoding/decoding.

**Implementation:** [blob.rs](src/ao-recorder/src/blob.rs) (BlobStore + handlers), [lib.rs](src/ao-recorder/src/lib.rs) (routes + AppState), [main.rs](src/ao-recorder/src/main.rs) (init), [config.rs](src/ao-recorder/src/config.rs) (blob settings), [blob.ts](src/ao-pwa/src/core/blob.ts) (utilities), [client.ts](src/ao-pwa/src/api/client.ts) (uploadBlob/getBlob), [AttachmentPicker.tsx](src/ao-pwa/src/components/AttachmentPicker.tsx), [BlobViewer.tsx](src/ao-pwa/src/components/BlobViewer.tsx), [ConsumerView.tsx](src/ao-pwa/src/components/ConsumerView.tsx) (integration). 11 Rust unit tests + 5 Rust integration tests + 5 PWA unit tests.

#### Security Mitigations

1. **Disk exhaustion prevention**: Per-chain blob quota (`blob_quota_per_chain`, default 100 MB) prevents any single chain from consuming unbounded disk. Individual blob size capped (`max_blob_bytes`, default 5 MB). Both configurable in `recorder.toml`.

2. **Stored XSS prevention**: Blob GET responses include `X-Content-Type-Options: nosniff` (prevents browser content sniffing), `Content-Security-Policy: default-src 'none'; script-src 'none'` (blocks all script execution), and `Content-Disposition: attachment` for non-image types (forces download instead of render).

3. **MIME allowlist**: Only `image/*` and `application/pdf` MIME types accepted on upload. Rejects `text/html`, `application/javascript`, and all other types that could enable stored XSS.

4. **Cross-chain isolation**: Blob ownership tracked per chain. `GET /chain/{A}/blob/{hash}` returns 404 if the blob was uploaded by chain B, preventing privacy leakage between independent chains on the same recorder.

5. **Temp file cleanup**: Stale `.tmp_*` files from prior crashes are automatically cleaned up when BlobStore initializes, preventing gradual disk waste from interrupted uploads.

#### Configuration

```toml
# recorder.toml
max_blob_bytes = 5242880           # max single blob size (default 5 MB)
blob_quota_per_chain = 104857600   # per-chain storage quota (default 100 MB)
```

**Remaining:** On-chain DATA_BLOB DataItem integration in `buildAssignment` (blobs are uploaded but not yet referenced as separable children in the assignment structure). Blob retention/pruning background task. Offline blob queue in IndexedDB.

#### Future: Asymmetric Blob Quota Auto-Tuning

When multi-recorder deployment reveals heterogeneous blob usage across many chains, consider an asymmetric auto-tuner: starts at the configured quota, **shrinks slowly** toward observed usage (EWMA of monthly upload rate + p95 blob size with 2x headroom), but **never grows without human intervention**. An operator must explicitly raise the quota via config or admin API. This prevents attackers from training the tuner upward while letting idle chains tighten automatically. Not needed now — the fixed per-chain quota + operator alerting (`disk_warn_percent`) covers the near-term threat model. Build only if the gap between "one global default" and "explicit per-chain config" proves real in production.

**Unblocks:** Ouma photographing tomato deliveries (Cooperative), crop damage documentation for insurance (Cooperative), vendor product photos (Tourism).

### N9: Server Operations Dashboard — *all three stories* ✓ (partial)

Evaluate and improve the sysop experience for recorder and validator deployment. Goal: a non-expert operator (Mako on Likiep, Wanjiku in Kiambu) can set up, monitor, and maintain the system with minimal technical support.

#### Subcommands — ✓

Three new subcommands for setup and diagnostics:

1. **`ao-recorder init [output.toml]`** — ✓ Generates a starter TOML config with sensible defaults, fresh blockmaker Ed25519 keypair, and `data/` directory. Prints public key and next-step instructions. Refuses to overwrite existing config.

2. **`ao-recorder doctor [config.toml]`** — ✓ Post-install diagnostic checklist: binary runs, config parseable, blockmaker seed valid, data directory writable, port available, SQLite functional, chain databases accessible, disk space adequate. Prints `[OK]`/`[FAIL]` per check, exits non-zero on failure.

3. **`ao-recorder bench [config.toml]`** — ✓ Hardware benchmark: inserts 1000 synthetic blocks into in-memory SQLite, measures throughput and RSS. Also benchmarks Ed25519 sign+verify (1000 ops). Reports blocks/sec, ops/sec, memory growth. Establishes baseline for capacity planning.

4. **`ao-recorder --version`** — ✓ Prints version string.

#### Runtime Monitoring — ✓

1. **Health endpoint** — ✓ `GET /health` returns JSON: `status` ("ok" | "degraded" | "error"), `uptime_seconds`, `version`, per-chain health (chain_id, symbol, block_height, last_block_age_seconds, utxo_count, db_size_bytes), system metrics (ram_used_bytes, ram_available_bytes, cpu_load_percent, disk_free_bytes, disk_used_bytes), capacity estimates. Status degrades on no chains or low disk. Uses `sysinfo` crate for cross-platform system metrics.

2. **Dashboard page** — ✓ `GET /dashboard` serves a self-contained HTML page with 10-second auto-refresh from `/health`. Dark theme, responsive grid. Status dot (green/amber/red), RAM and disk gauges with color thresholds, chain table with symbol, height, last block age (color-coded: amber >24h, red >7d), UTXO count, DB size. Context-sensitive help panel with troubleshooting guidance. No JavaScript framework — plain HTML + fetch + CSS. [dashboard.rs](src/ao-recorder/src/dashboard.rs).

3. **ChainStore health queries** — ✓ `count_utxos()`, `db_file_size()`, `last_block_timestamp()` added to ao-chain ChainStore for health endpoint consumption.

#### Operational Alerts — ✓

Background task (`run_alerts`) checks every 60 seconds:

1. **Disk space** — ✓ Warning when data directory filesystem drops below 10% free. Error at 5%. Configurable via `[alerts]` TOML section.

2. **Stale chain detection** — ✓ Warning when a chain has not recorded a block within configurable threshold (default 24 hours).

3. **Memory baseline** — ✓ Process RSS + system memory logged on startup and every hour (configurable). Establishes baseline for detecting leaks.

4. **Webhook integration** — ✓ `[alerts] webhook_url` fires HTTP POST with JSON payload (`event`, `message`, `timestamp`) for disk_warning, disk_critical, and chain_stale events. Fire-and-forget with 10s timeout.

**Config**: `[alerts]` section with `disk_warn_percent`, `disk_error_percent`, `stale_chain_seconds`, `memory_log_interval_seconds`, `webhook_url`.

#### Sysop Guide — ✓

[SysopGuide.md](SysopGuide.md): comprehensive operations guide readable start-to-finish. Covers: what the recorder is and its resource profile, installation (Pi/Debian/Windows/source), first-time setup (init → doctor → start → verify), monitoring (dashboard, health API, logs), common problems and solutions (unreachable, degraded, error, stale chains, UTXO growth, memory growth, port conflicts), disk management (growth rates, blob storage, backup procedures), TLS (Caddy and nginx examples), benchmarking, full configuration reference, and operational checklists (daily/weekly/monthly). Context-sensitive help in the dashboard links to corresponding sysop guide sections.

**Implementation:** [health.rs](src/ao-recorder/src/health.rs) (health endpoint, alerts, system metrics), [dashboard.rs](src/ao-recorder/src/dashboard.rs) (HTML dashboard), [main.rs](src/ao-recorder/src/main.rs) (subcommand dispatch: init, doctor, bench), [config.rs](src/ao-recorder/src/config.rs) (AlertsConfig, dashboard flag), [store.rs](src/ao-chain/src/store.rs) (count_utxos, db_file_size, last_block_timestamp). 5 health unit tests.

**Remaining:**
- Platform installers (.deb via cargo-deb for Pi aarch64 + x86_64, .msi via cargo-wix for Windows) — requires CI pipeline setup
- Prometheus metrics endpoint — now tracked as N22
- `tracing-subscriber` JSON formatter option
- Embedded validator co-deployment (`[validator]` config section)
- Per-platform quick-start guide documents
- Hardware-dependent acceptance tests: dashboard on Pi, real disk/stale alerts in production

**Unblocks:** Mako maintaining Tia's recorder on Likiep (UBI), Wanjiku monitoring Riuki cooperative infrastructure (Cooperative), any operator diagnosing issues without developer assistance (All three).

#### Acceptance Criteria

- `ao-recorder doctor` catches and clearly reports: missing data directory, port conflict, invalid config, unwritable paths ✓.
- `/health` endpoint returns system metrics via `sysinfo` crate ✓.
- `ao-recorder bench` produces throughput numbers for baseline capacity ✓.
- Dashboard page loads and auto-refreshes without JavaScript errors ✓.
- Sysop guide covers installation through troubleshooting ✓.
- Remaining: `.deb`/`.msi` install tests on target hardware, Prometheus metrics, real production alert testing.

### N10: Security Hardening — *All Three* ✓ Done (F3 deferred)

Network-layer security fixes identified by the [security audit](SECURITY_AUDIT.md). The core protocol (signatures, shares, fees, timestamps, serialization) is solid; these items harden the HTTP/deployment harness.

#### Implemented

**F1/F4 — Authentication & Rate Limiting** ✓: `security.rs` module with `ApiKeys` middleware (checks `Authorization: Bearer <key>`) and per-IP token-bucket `RateLimiter`. Config fields: `api_keys`, `read_rate_limit`, `write_rate_limit`. Applied as Axum middleware layers in `build_router_with_config()`. Rate limiter cleanup runs every 60s. 5 unit tests.

**F2 — Connection Limits** ✓: `connection_semaphore: Option<Arc<Semaphore>>` on `AppState`, enforced in SSE and WebSocket handlers. Config field: `max_connections` (0 = unlimited). Excess connections get 503. `PermitStream` wrapper holds permit for stream lifetime. Lag notifications: SSE sends `event: lagged`, WebSocket sends `{"event":"lagged","skipped":N}`.

**F5 — BigInt Exchange Arithmetic** ✓: `compute_sell_amount` now uses `BigRational` (exact arithmetic, no f64 precision loss). Rates scaled to 10^9 rational. Truncates toward zero (exchange keeps remainder). 8 tests including large-amount and edge cases.

**F6 — PREV_HASH Validation** ✓: `verify_block_batch()` in ao-validator tracks `prev_block_hash` across iterations, compares with `extract_prev_hash()`. Mismatches produce clear error. 6 verifier tests.

**F7 — Lock Error Handling** ✓: All `.expect()` on mutex locks replaced with `map_err` returning 500. All `.expect("system clock")` replaced with `.unwrap_or_default()`. `current_wall_timestamp()` helper. Transaction rollback failures logged via `tracing::error!`.

**F8 — Validator URL Validation** ✓: `load_config()` validates all validator URLs at startup. Non-HTTPS URLs rejected for non-local hosts unless `allow_insecure_validators = true`. Local hosts (localhost, 127.*, [::1]) always allowed over HTTP.

**F9 — Block Size Guard** ✓: `MAX_BLOCK_DESERIALIZE_SIZE` (1 MB) check before deserializing blocks in GET `/blocks`. Oversized blocks return error instead of risking OOM.

**F10 — Error Sanitization** ✓: `RecorderError::into_response()` returns generic messages ("invalid request", "internal error", etc.). Details logged at DEBUG/ERROR level only.

#### Deferred

**F3 — CAA Recorder Trust**: Signed recorder identity deferred to when multi-recorder topology is implemented. Current config-based `known_recorders` is adequate for single-recorder deployments.

### N11: Multi-Device Wallet Sync — *All Three* ✓

Specification: [specs/WalletSync.md](specs/WalletSync.md). User guide: [WalletSyncGuide.md](WalletSyncGuide.md).

Five sub-phases (S1–S5) delivering multi-device key sync for AO's single-use key model:

**S1: UTXO Validation on Connect** — ✓: `validateKeysOnChain()` queries recorder for each held key's UTXO status on startup and on SSE block events. Stale keys marked spent. Unknown-device spend alerts displayed in ConsumerView.

**S2: Wallet State Model + IndexedDB Migration** — ✓: `walletDb.ts` (449 lines). `ao-wallet` IndexedDB with `keys`, `peers`, `config` object stores. `KeyEntry` model with chain/seq/status/sync tracking. Device identity (persistent 16-byte random ID). `migrateFromLocalStorage()` imports single-seed wallet on first load. `chainBalance()` aggregates unspent keys. Seed encryption via Argon2id + XChaCha20-Poly1305 per CryptoChoices.md §4. `PassphrasePrompt` component for setup and session unlock. 13 tests.

**S3: QR Key Transfer** — ✓: `walletSync.ts` (298 lines). `buildSyncPayload()` / `buildFullExportPayload()` assemble key/spent entries. `importSyncPayload()` with idempotent key import. `AnimatedQrCode` component for multi-frame QR (>1800 bytes). `WalletSync` component with export/scan/file-import modes. Header sync badge shows unsynced count. File-based fallback for desktop-to-desktop. 8 + 12 tests.

**S4: Paired-Device Push Relay** — ✓: `pairing.ts` (212 lines) — X25519 key agreement → HKDF-SHA256 → `relay_key`. AES-GCM relay encryption. Wallet ID = SHA-256(relay_key) truncated (browser-compatible; spec notes BLAKE3 alternative). `relayClient.ts` (278 lines) — WebSocket relay client with auto-reconnect, KEY_ACQUIRED/KEY_SPENT/HEARTBEAT sync messages, sequence replay protection. `PairedDevices` component with QR pairing ceremony, peer list, unpair. **ao-relay** crate (316 lines) — Axum WebSocket forwarder, per-wallet broadcast channels, 72-hour message retention, background cleanup. App.tsx wires RelayClient lifecycle to paired device state. 11 + 5 tests.

**S5: User Guide** — ✓: [WalletSyncGuide.md](WalletSyncGuide.md) — single-device basics, QR sync, paired devices, lost device recovery, backup recommendations, troubleshooting.

**Implementation:** [walletDb.ts](src/ao-pwa/src/core/walletDb.ts), [walletSync.ts](src/ao-pwa/src/core/walletSync.ts), [pairing.ts](src/ao-pwa/src/core/pairing.ts), [relayClient.ts](src/ao-pwa/src/core/relayClient.ts), [WalletSync.tsx](src/ao-pwa/src/components/WalletSync.tsx), [AnimatedQrCode.tsx](src/ao-pwa/src/components/AnimatedQrCode.tsx), [PairedDevices.tsx](src/ao-pwa/src/components/PairedDevices.tsx), [PassphrasePrompt.tsx](src/ao-pwa/src/components/PassphrasePrompt.tsx), [ao-relay/](src/ao-relay/). Integration in App.tsx (migration + passphrase + relay lifecycle), Header.tsx (sync badge), Settings.tsx (paired devices), ConsumerView.tsx (IndexedDB key storage + UTXO validation + spend alerts), useStore.ts (multi-key state, passphrase, relay). 49 N11-specific tests (13 walletDb + 8 walletSync + 12 animatedQr + 11 pairing + 5 ao-relay).

**Remaining:** Two-device end-to-end workflow test (requires hardware). Cloud vault backup (WalletSync.md §5, deferred).

**Unblocks:** Husband and wife sharing a coin wallet from separate phones (All three). Tourist accessing coins from both phone and tablet (Tourism). Tia checking balances from shop desktop and personal phone (UBI). Cooperative members syncing transaction keys across field devices (Cooperative).

### Priority Summary

| # | Gap | Stories | Effort | Depends On | Status |
|:-:|-----|---------|--------|------------|--------|
| **Foundation (deployment gaps)** | | | | | |
| N1 | Exchange agent auto-trade | All three | Medium | — | ✓ Done |
| N2 | PWA end-to-end assignment | Tourism, UBI | Medium | — | ✓ Done |
| N3 | QR code chain discovery | Tourism, UBI | Small | N2 | ✓ Done |
| N4 | Offline assignment queue | UBI, Coop | Medium | N2 | ✓ Done |
| N5 | Vendor profile + location | Tourism | Small–Medium | N2 | ✓ Done |
| N6 | EXCHANGE_LISTING structure | All three | Small | N1 | ✓ Done |
| N7 | Cooperative metadata conventions | Coop | Small | — | ✓ Done |
| N8 | Binary attachments (photo/doc) | Coop, Tourism | Medium | N2, N7 | ✓ Done |
| N9 | Server operations dashboard | All three | Medium–Large | — | ✓ Partial |
| N10 | Security hardening | All three | Medium | — | ✓ Done (F3 deferred) |
| N11 | Multi-device wallet sync | All three | Medium–Large | N2, N3 | ✓ Done |
| **Tier 1: Pilot blockers** | | | | | |
| N12 | Transfer confirmation screen | All three | Small | N2 | ✓ Done |
| N13 | Wallet backup/restore UX | All three | Small | N11 | ✓ Done |
| N14 | Offline balance cache | All three | Small | N11 | ✓ Done |
| N15 | Transaction history + CSV export | All three | Medium | N2 | ✓ Done |
| N16 | Vendor profile persistence + on-chain | Tourism | Small | N5 | ✓ Done |
| N17 | On-chain blob linking + blob fees | Coop, Tourism | Medium | N8 | — |
| **Tier 2: Adoption** | | | | | |
| N18 | Payment notifications (chime) | Tourism, UBI | Small | N2 | — |
| N19 | Printable QR signage (PDF + PNG) | Tourism | Small | N3 | — |
| N20 | Sales reporting | Tourism, Coop | Medium | N15 | — |
| N21 | SSE deposit detection (exchange) | All three | Small | N1 | — |
| N22 | Prometheus metrics | All three | Medium | N9 | — |
| N23 | Mobile UI polish | All three | Medium | — | — |
| **Tier 3: Maturity** | | | | | |
| N24 | Multi-chain vendor dashboard | Tourism | Medium | N16 | — |
| N25 | Refutation UI (power user) | All three | Medium | N15 | — |
| N26 | Exchange trade history + P&L | All three | Medium | N21 | — |
| N27 | CAA escrow UI in PWA | All three | Large | N11 | — |
| N28 | Credential issuance UI | All three | Medium | — | — |
| N29 | External anchoring (off-disk) | All three | Medium | — | — |
| N30 | BLOB_POLICY in genesis | Coop, Tourism | Medium | N17 | — |
| N31 | Blob pruning + audit endpoints | Coop, Tourism | Medium | N30 | — |
| **Tier 4: Strategic** | | | | | |
| N32 | Cooperative metadata UI | Coop | Large | N17 | — |
| N33 | Hot-standby recorder | All three | Large | N10 (F3) | — |

N1–N8 complete. N9 partially complete (installers remaining; Prometheus now tracked as N22). N10 complete except F3 (deferred). N11 complete. N12–N33 derived from [specs/UnmetNeedsReport.md](specs/UnmetNeedsReport.md) and [specs/BlobRetentionReport.md](specs/BlobRetentionReport.md).

Remaining hardware-dependent acceptance tests from earlier phases: two-device <3s latency, iOS/Android PWA install, Pi stress tests, Lighthouse PWA audit.

---

## Tier 1: Pilot-Ready UX

These items must ship before handing the system to real vendors and consumers. All are small-to-medium effort with no external dependencies. Analysis: [specs/UnmetNeedsReport.md](specs/UnmetNeedsReport.md) §1–2, [specs/BlobRetentionReport.md](specs/BlobRetentionReport.md).

### N12: Transfer Confirmation Screen — *All Three* ✓

Transfer flow split into build → preview → confirm in ConsumerView.tsx. `handleBuild()` constructs the assignment with fee convergence and displays a confirmation panel. `handleConfirm()` signs and submits only after user review.

**Confirmation panel shows:** amount sending (shares), recording fee, change returned (if any), recipient pubkey (truncated, or "New key"), attachment count. "Edit" returns to form; "Confirm & Send" submits. `PendingTransfer` interface holds built state between phases.

**Implementation:** Pure refactor of [ConsumerView.tsx](src/ao-pwa/src/components/ConsumerView.tsx). No new dependencies or files.

### N13: Wallet Backup/Restore UX — *All Three* ✓

Encrypted file-based wallet backup using existing `buildFullExportPayload()`. Backup file format: JSON envelope with PBKDF2-SHA256 + AES-256-GCM encrypted payload (salt + IV + ciphertext). Export prompts for backup password (8+ chars, confirmed), downloads `.json` file. Import accepts file + password, decrypts and merges keys via `importSyncPayload()`. `WalletBackup` component in Settings panel with export/import modes. Backup age tracking in IndexedDB (`lastBackupAt`): yellow warning when no backup exists or >30 days stale. Cloud backup deferred (WalletSync.md §5).

**Implementation:** [backup.ts](src/ao-pwa/src/core/backup.ts) (encrypt/decrypt), [WalletBackup.tsx](src/ao-pwa/src/components/WalletBackup.tsx) (UI), [walletDb.ts](src/ao-pwa/src/core/walletDb.ts) (`lastBackupAt` config field), [Settings.tsx](src/ao-pwa/src/components/Settings.tsx) (integration). 4 tests (round-trip, wrong password, large payload, salt uniqueness).

### N14: Offline Balance Cache — *All Three* ✓

Cached balance from IndexedDB displayed immediately on app load, before any UTXO scan. Zustand store holds `cachedBalance` (stringified bigint) and `lastValidatedAt` (Unix ms). On chain selection, `walletDb.chainBalance()` loads the cached sum. After successful UTXO validation, both fields refresh. UI shows "verified {time}" or "unverified"/"stale" badges.

**Implementation:** [useStore.ts](src/ao-pwa/src/store/useStore.ts) (`cachedBalance`, `lastValidatedAt` fields), [ConsumerView.tsx](src/ao-pwa/src/components/ConsumerView.tsx) (load effect, validation refresh, cached balance display with staleness badges).

### N15: Transaction History + CSV Export — *All Three* ✓

Scrollable transaction list with date, direction, amount, counterparty (truncated pubkey), blob indicator, and block height. Parses blocks via `getBlocks()` API, matches participants against wallet keys (receivers by pubkey, givers by seq_id). Separate `ao-tx-history` IndexedDB database caches parsed records for offline access with incremental scan cursor. CSV export with date, time, direction, shares, coins, counterparty, block height, seq_id, blob flag. `Exporter` interface supports future SA-BAS/OFX/XBRL-CSV formats.

**Implementation:** [txHistory.ts](src/ao-pwa/src/core/txHistory.ts) (block parser, IndexedDB cache, CSV exporter, `Exporter` interface), [TransactionHistory.tsx](src/ao-pwa/src/components/TransactionHistory.tsx) (UI), [ConsumerView.tsx](src/ao-pwa/src/components/ConsumerView.tsx) (integration). 10 tests (block parsing: received/sent/self-transfer/multi-block/timestamp/blob detection; CSV: header/coins/multi-seq).

### N16: Vendor Profile Persistence + On-Chain Records — *Tourism* ✓

Vendor profiles persist in per-chain SQLite via `vendor_profile` table. API handlers read/write SQLite (in `spawn_blocking`) with in-memory cache for chain listing. Profiles loaded from SQLite on recorder startup. PWA `buildVendorProfile()` constructs `VENDOR_PROFILE` (type 36, separable) container; `buildAssignment()` extended with optional `separableItems` parameter for on-chain profile recording. VendorView editor unchanged — persistence is transparent.

**Implementation:** [store.rs](src/ao-chain/src/store.rs) (`vendor_profile` table, get/set methods), [lib.rs](src/ao-recorder/src/lib.rs) (SQLite-backed handlers, `set_vendor_profile_cache`), [main.rs](src/ao-recorder/src/main.rs) (startup profile loading), [assignment.ts](src/ao-pwa/src/core/assignment.ts) (`buildVendorProfile`, separable items parameter).

### N17: On-Chain Blob Linking + Pre-Substitution Fees — *Coop + Tourism* ✓ Done

Blobs are now linked on-chain as DATA_BLOB separable children in assignments. Recording fees computed on pre-substitution size (full blob payloads). Recorder reconstructs pre-sub size from SHA256 hashes + stored blob sizes for fee validation. Chain-isolated blob size lookups prevent cross-chain fee manipulation. 6 TS + 5 Rust tests added. `vbc::encoded_unsigned_len` + `encoded_signed_len` helpers added. Spec §5.5 hash item size corrected (34→33 bytes).

---

## Tier 2: Adoption Features

### N18: Payment Notifications — *Tourism + UBI* ✓ Done

Web Audio API chime on VendorView SSE payment events. Three sound styles (bell, cash register, ding), volume slider, quiet hours (overnight time window), quick-mute with duration picker (15min/1hr/until quiet-hours-end). Settings persisted in localStorage. Notification ref pattern avoids SSE reconnection on settings change. Web Push deferred.

### N19: Printable QR Signage — *Tourism* ✓ Done

Collapsible "Print QR Signage" section in VendorView. SVG QR code preview with chain symbol, business name (from vendor profile), and "Scan to pay" label. Print/PDF via browser print dialog (opens in new window, auto-closes after print). PNG download at 4x scale (1600px). Size guidance displayed. Filename sanitized.

### N20: Sales Reporting — *Tourism + Coop* ✓ Done

SalesReport component in VendorView. Aggregates received transactions daily/weekly/monthly with totals (coins), transaction count, average per-tx. UTC-based period keys (daily YYYY-MM-DD, weekly ISO YYYY-Wnn, monthly YYYY-MM). Date range picker (default 30 days). Incremental block scanning with IndexedDB cache (reuses N15 txHistory). CSV export with coin amounts and averages.

### N21: SSE Deposit Detection for Exchange — *All Three* ✓ Done

SSE-based deposit detection in ao-exchange. Per-chain SSE listener tasks subscribe to recorder `/chain/{id}/events`, notify main loop via mpsc channel on new blocks, triggering immediate `check_deposits`. Falls back to polling on SSE disconnect with automatic reconnection. Config field `deposit_detection` validated ("sse" default, "polling" legacy). Buffer management: 64KB cap prevents memory exhaustion, partial-event retention avoids dropped events across chunks. SSE comments (keep-alive) stripped. Silent exit on all-listeners-dead returns error instead of Ok. 12 new tests (SSE parsing, config defaults/validation).

### N22: Prometheus Metrics — *All Three*

Standard monitoring integration for ao-recorder and ao-validator.

**Deliverables:**
- `prometheus` crate dependency. `GET /metrics` endpoint (optional, behind feature flag).
- Instruments: block submissions (counter + latency histogram), blob uploads (counter + size histogram), validation runs (counter + result label), SSE connections (gauge), active chains (gauge).
- Example Grafana dashboard JSON in `2026/ops/`.

### N23: Mobile UI Polish — *All Three*

Mobile-first CSS audit and touch optimization.

**Deliverables:**
- Audit all views at 375px width. Fix horizontal overflow, font scaling, input focus issues.
- Touch targets ≥ 44×44px for all interactive elements.
- iOS Safari: safe area insets, prevent zoom on input focus.
- Lighthouse audit targeting ≥ 90 mobile performance score.

---

## Tier 3: Maturity Features

### N24: Multi-Chain Vendor Dashboard — *Tourism*

Unified view for vendors with multiple product lines (e.g. BCG + RMF).

**Deliverables:**
- Chain selector/summary bar in VendorView showing all vendor-owned chains.
- Per-chain cards: symbol, today's revenue, last transaction time, SSE status.
- Tap card to expand into single-chain detail. Combined revenue summary.

**Depends on:** N16 (persistent profiles to identify owned chains).

### N25: Refutation UI — *All Three* (power user)

PWA interface for disputing fraudulent or late assignments. Shipped behind a settings toggle.

**Deliverables:**
- "Dispute" button on transaction history entries or UTXO detail view.
- Builds refutation DataItem, signs with user's key, submits to recorder.
- Confirmation dialog explaining consequences.

**Depends on:** N15 (transaction history for discoverability).

### N26: Exchange Trade History + P&L Dashboard — *All Three*

Exchange operator tools: trade history, inventory tracking, profit/loss.

**Deliverables:**
- New `ao-exchange` endpoint: `GET /trades?from=&to=` (requires `trade_history` SQLite table in exchange daemon).
- `ExchangeDashboard.tsx`: trade list, inventory levels per chain with low-stock warnings, realized P&L.

**Depends on:** N21 (SSE detection makes trade data more complete).

### N27: CAA Escrow UI in PWA — *All Three*

Peer-to-peer atomic swap interface without exchange intermediary.

**Deliverables:**
- `EscrowView.tsx`: "Propose Swap" form (source chain + amount, destination chain + amount, deadline).
- Pending escrow list with status indicators (STARTED, TRANSFERRED, FINALIZED, CANCELED, TIMED_OUT).
- Accept/Cancel buttons. Timeout countdown. Real-time SSE on both chains.

**Depends on:** CAA protocol (Phase 6, done), `ao-exchange` HTTP API (exists).

### N28: Credential Issuance UI — *All Three*

Face-to-face and friends-of-friends trust model. Self-issued credentials only for now — no centralized authority, no complex DID resolution.

**Deliverables:**
- Credential issuance workflow: review vendor profile → issue credential (sign `CREDENTIAL_REF` with own key) → record on vendor's chain.
- PWA `TrustIndicator` extended to display credential details on tap.
- Revocation: submit revocation entry. `TrustIndicator` checks status.
- Trust model: face-to-face issuer identity. No institutional authority required.

### N29: External Anchoring — *All Three*

Phased approach to off-disk anchor durability. Pluggable anchor trait already exists.

**Deliverables (phased):**
1. **Replicate anchor file** to a second location (S3, SFTP, or second machine). Addresses disk-failure risk.
2. **Public transparency log** or Nostr relay (timestamped, signed). Lower cost than Bitcoin.
3. **Bitcoin OP_RETURN** for high-value chains. Batch multiple chain anchors into single Merkle root per TX.

Phase 1 is the near-term target. Phases 2–3 are later.

### N30: BLOB_POLICY in Genesis — *Coop + Tourism*

Machine-readable blob retention policy as a genesis DataItem child. Non-separable type codes. Design: [specs/BlobRetentionReport.md](specs/BlobRetentionReport.md) §2.2.

**Deliverables:**
- Allocate non-separable type codes for `BLOB_POLICY`, `BLOB_RULE`, and children (`MIME_PATTERN`, `RETENTION_SECS`, `CAPACITY_LIMIT`, `THROTTLE_THRESHOLD`). Exact codes allocated during implementation against type code registry.
- Extend genesis builder (ao-chain, ao-cli, PWA genesis creator) to accept optional `BLOB_POLICY`.
- Recorder `BlobStore` enforces active chain's policy on upload.
- `GET /chain/{id}/blob-policy` endpoint. PWA displays policy in `ChainDetail`.
- Chains without policy: best-effort default, "no retention guarantee" indicator in PWA.

**Depends on:** N17 (on-chain blob linking).

### N31: Blob Pruning + Audit Endpoints — *Coop + Tourism*

Automated lifecycle management for blob storage. Design: [specs/BlobRetentionReport.md](specs/BlobRetentionReport.md) §3–4.

**Deliverables:**
- `blob_meta` SQLite table: `(hash, chain_id, mime, size, uploaded_at)`. Populated on upload; backfilled from filesystem.
- Background pruning task (configurable interval, default daily): evaluates retention rules, deletes expired blobs, returns **410 Gone** for pruned hashes.
- `ao-recorder prune --dry-run` command.
- `HEAD /chain/{id}/blob/{hash}` — existence/age check without body transfer.
- `GET /chain/{id}/blobs/manifest` — paginated JSON metadata list.
- PWA `BlobViewer` handles 410 gracefully.
- Optional `ao-validator` blob audit mode: walk chain blocks, HEAD-check blob hashes, alert on premature pruning.

**Depends on:** N30 (BLOB_POLICY for retention rules).

---

## Tier 4: Strategic

### N32: Cooperative Metadata UI — *Coop*

Specialized PWA view for agricultural cooperative workflows. Not first pilot, but developed and simulated as if live.

**Deliverables:**
- `CooperativeView.tsx`: forms for delivery entry (crop, weight, grade, lot), sale recording, cost allocation, advance payments.
- Records as `NOTE` (type 32) separable items with structured `key:value` content per [specs/CooperativeMetadata.md](specs/CooperativeMetadata.md).
- Provenance viewer: trace lot from farmer delivery to consumer sale.
- Photo attachments via blob infrastructure (N17).
- Sim scenario exercising full cooperative workflow.

**Depends on:** N17 (on-chain blob linking for photo receipts).

### N33: Hot-Standby Recorder — *All Three*

Progressive approach to recorder availability. Quick-restart (systemd auto-restart + SQLite WAL recovery) is the near-term target. Hot standby is later.

**Deliverables (phased):**
1. **Quick-restart resilience (now):** Document and test systemd `Restart=always` + `WatchdogSec`. Verify SQLite WAL recovery after unclean shutdown. Measure restart-to-serving latency.
2. **Hot standby (later):** Second recorder subscribes to primary's SSE, mirrors blocks + blobs. DNS/load-balancer failover on primary failure. No simultaneous writes.
3. **Signed recorder identity (prerequisite for standby):** Implements F3 from N10. Clients verify recorder legitimacy without pre-shared config.

Phase 1 sufficient for pilot scale (single vendor, single Pi). Phase 2 for production deployments.

**Depends on:** N10 F3 (signed recorder identity) for Phase 2.

---

## Deferred

Items identified but not scheduled. Develop as resources become available.

**Fiat on/off-ramp tooling** — Exchange agent documentation, PWA deep-link format for payment pages, API webhooks for fiat receipt confirmation. Targets need identification first.

**LoRa mesh transport** — Meshtastic serial/BLE bridge daemon forwarding AO messages between LoRa radio and local recorder. Aspirational; protocol must remain compact enough for LoRa but dedicated implementation deferred.

**Installer hardware testing** — .deb and .msi install tests on real Debian/Windows/Pi hardware. Will be tested manually as resources become available. 72-hour Pi stress test requires dedicated hardware.

**Web Push notification server** — VAPID key infrastructure for system-level push notifications. Far down roadmap; SSE-while-open + audio chime (N18) covers near-term needs.

**Cloud wallet backup** — Encrypted vault backup per WalletSync.md §5. Requires cloud infrastructure. File-based backup (N13) covers near-term needs.

---

## Not Covered

**TⒶ³ (multiple competing recorders)** and **TⒶ⁴ (underwriters/error checkers)** — conceptual only in the 2018 docs. Would need Phase 0-equivalent specification work.

**Regulatory compliance** — commodity-backed tokens sit in an unclear space. Architecture targets gift card / loyalty point frameworks (most favorable category). Real deployment needs jurisdiction-specific counsel.

**Pilot deployment** — the roadmap produces working software. Finding the first community willing to try it is a business problem, not a software deliverable.
