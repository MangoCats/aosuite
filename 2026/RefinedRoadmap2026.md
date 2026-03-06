# Refined Roadmap 2026

This document refines the original [Roadmap 2026](Roadmap2026.html) after a thorough re-reading of all original design documents. The core technology choices remain sound. The refinement addresses structural problems in the original roadmap: missing specification work, underspecified wire formats, unresolved protocol ambiguities, and the absence of architecture documentation as a deliverable. The most significant change is the addition of a substantial architecture phase before any code is written.

## Critique of the Original Roadmap

The original [Roadmap 2026](Roadmap2026.html) is a well-structured plan with realistic scope per phase and good instinct about ordering (single chain first, exchange agents before atomic CAA). However, six problems emerged from careful reading of the source documents:

**1. No specification resolution phase.** The 2018 design documents are distributed across 30+ HTML files with numerous ambiguities that would stall implementation. The VBC encoding does not provide worked examples for negative numbers. The GMP rational fraction wire format has a tricky nested-size layout that is easy to get wrong. The separability bit position in type codes is explicitly flagged as "under reconsideration." Signature encoding format (DER vs. raw bytes) is never specified. The RSA padding scheme is never named. The `byteCodeDefinitions.json` file is called the "single source of truth" for type codes, but its schema is not documented anywhere. Jumping into Phase 1 coding without resolving these would mean making ad-hoc decisions that are hard to change later.

**2. Deterministic arithmetic is identified as critical but not treated as a deliverable.** The share/coin system depends on every node computing identical results for fee deductions, age-tax computations, and coin display values. The formulas involve arbitrary-precision integer division with rounding. The rounding rules are never specified. For a blockchain that must achieve consensus, this is not an implementation detail — it is a protocol specification.

**3. Late-recording ambiguity is a design decision, not a bug to discover.** An assignment agreement past its recording deadline can still be recorded unless explicitly refuted or the source shares were spent elsewhere. This creates a window where a giver must actively monitor and refute expired agreements. This is a deliberate design choice in [AssignmentAgreement.html](../docs/html/AssignmentAgreement.html), but it has significant implications for wallet UX, key lifecycle, and the escrow period after expiration. It needs to be resolved in architecture, not discovered during implementation.

**4. The existing C++ codebase is a useful reference but a potential trap.** The ~15,600 lines of C++/Qt implement the serialization layer using the 2018 crypto choices (ECDSA brainpool-256, RSA-3072, SHA3-512 via OpenPGP/libgcrypt). The 2026 roadmap correctly switches to Ed25519 and BLAKE3, but the implications ripple through the entire type-code registry, test vectors, and wire format. Blindly following the C++ code's byte layouts while using different algorithms would produce subtle incompatibilities. The architecture phase must produce a clean, self-consistent specification for the 2026 format.

**5. The timeline compresses too aggressively at the start.** Phase 1 (6 weeks for ao-types + ao-crypto) is achievable only if every wire format question is already answered. Without a specification phase, those 6 weeks will be spent half-coding and half-designing, producing code that may need rework when later design decisions conflict. The 48-week total is reasonable if the first few weeks are spent on architecture.

**6. No architecture documentation as a deliverable.** The 2018 documents are design fiction and requirements sketches. They are excellent for understanding intent, but they are not implementation specifications. A new developer (or an AI assistant) cannot build from them without constant interpretation. The refined roadmap treats architecture documentation as a first-class deliverable that enables everything that follows.

## Technology Stack: Refinements

The original stack choices are mostly confirmed. Adjustments based on current ecosystem state:

| Component | Original Choice | Refined Choice | Notes |
|-----------|----------------|----------------|-------|
| Signatures | `ed25519-dalek` | `ed25519-dalek` 2.x | The 2.0 release (late 2023) introduced breaking changes and a maintenance controversy, but the crate is now stable under the dalek-cryptography org. Pin to a specific version and audit. `ring` is a viable backup if dalek stalls again. |
| Hashes | SHA2-256 + BLAKE3 | SHA2-256 + BLAKE3 | Confirmed. `sha2` and `blake3` crates are mature, audited, `no_std`-compatible. Drop SHA3-512 from the 2018 spec entirely — BLAKE3 is faster with equivalent security. |
| Big integers | `num-bigint` + `num-rational` | `num-bigint` + `num-rational` | Confirmed. Slower than GMP but pure Rust, cross-compiles cleanly, adequate for AO's scale. Performance-critical paths (fee calculation) should be benchmarked. |
| HTTP | Axum | Axum 0.8+ | Stable, mature, well-supported in the tokio ecosystem. No concerns. |
| MQTT | `rumqttc` | `rumqttc` | Actively maintained. Consider `rumqttd` (embedded broker in Rust) as an alternative to requiring a separate Mosquitto install for simple deployments. |
| Storage | `rusqlite` | `rusqlite` | Synchronous, but fine for a single-recorder model where write throughput is low. Wrap with `tokio::task::spawn_blocking` if needed. |
| Client crypto | Web Crypto API + `tweetnacl-js` | Web Crypto API (primary) | Ed25519 is now supported in Web Crypto API in Chrome, Edge, and Firefox. Safari support may still require a polyfill. `tweetnacl-js` remains as fallback but should not be the primary path. |
| Testing | `cargo test` + `proptest` | `cargo test` + `proptest` + **conformance vectors** | Add a set of hand-computed conformance test vectors (in JSON) produced during the architecture phase. These are the ground truth for cross-implementation compatibility. |

## Revised Phase Structure

| Phase | Weeks | What |
|-------|-------|------|
| 0: Architecture & Specification | 1–4 | Resolve ambiguities, produce implementation-grade specs, generate test vectors |
| 1: Foundation | 5–10 | `ao-types` + `ao-crypto` crates, genesis CLI |
| 2: Single-Chain Recorder (TⒶ¹) | 11–20 | `ao-chain` + `ao-recorder`, full CLI tools |
| 3: Vendor + Consumer Apps | 21–28 | React PWA with AOS + AOE views |
| 4: Market Making + Exchange | 29–38 | AOI view, exchange agents, MQTT, automated trading |
| 5: Validation + Trust (AOV) | 39–44 | Validator, anchor proofs, vendor credentials |
| 6: Atomic Multi-Chain (TⒶ²) | 45–54 | Full CAA escrow protocol |

The total extends to approximately 54 weeks. The additional 6 weeks compared to the original are invested in architecture (4 weeks) and more realistic timelines for Phase 2 (recorder + full CLI) and Phase 4 (market making is the most complex business logic in the system).

## Phase 0: Architecture & Specification (Weeks 1–4)

This phase produces documents, not code. Every ambiguity surfaced by a careful reading of the 2018 design documents is resolved in writing. The output is a set of specification documents that a competent developer could implement from without needing to interpret the original narrative documents or reverse-engineer the C++ code.

### Deliverable 0A: System Architecture Document

A single document consolidating the architectural decisions scattered across Architecture.html, CorePrinciples.html, Philosophy.html, BlockchainBasics.html, and ProofOfWork.html into a coherent implementation architecture. It must cover:

**Actor model:** Precise definitions of each actor (Asset Organizer, Recorder, Validator, Checker) with their trust boundaries, the data they hold, the operations they perform, and the messages they exchange. The 2018 Architecture.html describes this but mixes aspirational A2/A3/A4 features with concrete A1 requirements. The architecture document separates what is built now (A1) from what is designed for later.

**System topology:** How AOR servers, AOV validators, AOE/AOS/AOI clients, and MQTT brokers connect. Which connections are HTTP, which are WebSocket, which are MQTT. Where TLS is required vs. optional. How discovery works (genesis block URLs, exchange agent directories).

**Data flow diagrams:** The complete lifecycle of a share assignment from proposal through recording, for both single-chain (A1) and the simplified exchange-agent model. Sequence diagrams, not prose.

**Security model:** Where private keys live and never leave. What each component can and cannot do if compromised. The recorder never sees private keys. The validator can only invalidate, never validate. The exchange agent absorbs settlement risk in Phase 4; trustless CAA removes that in Phase 6.

**Non-negotiable principles:** Extracted from CorePrinciples.html and stated as testable assertions. No proof of work. Single-use keys for share transfers. Mutual consent. Timestamped signatures. Share expiration. Separable items. Cryptographic agility. Open source.

### Deliverable 0B: Wire Format Specification

A precise, byte-level specification of the on-chain binary format, resolving all ambiguities in the 2018 documents. Must include:

**VBC encoding:** Complete specification with worked examples for positive integers (0, 1, 63, 64, 127, 128, 8191, 8192), negative integers (-1, -64, -65), and boundary values near the 10-byte limit. Specify explicitly: bit 0 is sign (0=positive, 1=negative), bits 1-6 of byte 0 carry the least significant 6 bits of the magnitude, bit 7 is continuation. Continuation bytes carry 7 bits each (bits 0-6), with bit 7 as continuation. Total capacity: 63 bits of magnitude plus sign = signed 64-bit integer. A continuation flag set on byte 10 is an error. Provide a table of byte sequences for at least 20 test values.

**DataItem structure:** Type code (VBC) followed by size (VBC, for variable-length items) or nothing (for fixed-length items) followed by data. For container items, data is a sequence of child DataItems. Specify the complete type code registry with columns: code (decimal and hex VBC bytes), name, size mode (fixed/variable/VBC-as-data), fixed size in bytes (if applicable), separability flag, and human description. This replaces the undocumented `byteCodeDefinitions.json` with a proper specification table.

**GMP integer encoding:** VBC byte count, then that many bytes big-endian, MSB of first byte is sign (0=positive, 1=negative), magnitude in remaining bits. Specify: zero is encoded as zero bytes (byte count = 0). Positive values whose magnitude has MSB set must prepend 0x00. Provide worked examples for 0, 1, -1, 127, 128, -128, 2⁶⁴, and a large value representative of initial share counts (~2⁸⁶).

**Rational fraction encoding:** Outer VBC (total remaining bytes), inner VBC (numerator byte count), numerator bytes (GMP format), denominator bytes (GMP format, size = outer - inner - sizeof(inner VBC)). Specify: denominator is always positive (sign carried in numerator only). Denominator of zero is an error. Provide worked examples for 1/2, -3/7, and a share-fraction representative of a typical fee calculation.

**Timestamp encoding:** 8 bytes big-endian signed integer. Value = Unix timestamp in seconds × 189,000,000. This gives ~5.29ns resolution. Specify leap-second handling: timestamps must be monotonically increasing per actor; if wall clock produces a duplicate or earlier value, increment by 1. Provide worked examples for epoch, a date in 2026, and the rollover date.

**Block structure:** Complete byte-level layout of a signed block under A1: block hash, then signed block contents (blockmaker signature + public key + block contents), where block contents = previous block hash + first ID sequence number + page count + pages. Each page = page index + ID sequence offset + assignment agreement. Provide a worked example of a minimal block containing one assignment.

### Deliverable 0C: Cryptographic Choices Document

Nail down every cryptographic decision for the 2026 implementation:

**Signature algorithm:** Ed25519 as specified in RFC 8032. Keys are 32 bytes (seed) / 32 bytes (public). Signatures are 64 bytes. No DER wrapping — raw bytes only. The signature in a DataItem is: type code (1 byte, indicating Ed25519) + 64 bytes signature + 8 bytes timestamp (appended to the data before signing, included in the DataItem after signing). Specify exactly what bytes are signed: the serialized DataContainer with all separable items replaced by their hashes, concatenated with the 8-byte signing timestamp.

**Hash algorithms:** SHA2-256 (32 bytes) for chain integrity and separable item replacement. BLAKE3 (32 bytes) for content-addressing and performance-sensitive hashing (block ID computation, Merkle trees if introduced later). Specify: when the spec says "hash" without qualification, SHA2-256 is meant. BLAKE3 is used only where explicitly specified. The separable-item replacement hash is always SHA2-256.

**Key serialization:** Ed25519 public keys are 32 bytes, stored raw (no ASN.1, no PEM). Private keys (seeds) are 32 bytes, stored encrypted in wallet files, never on-chain. Specify the wallet encryption scheme: the private key seed is encrypted with XChaCha20-Poly1305 using a key derived from the user's passphrase via Argon2id (memory=64MB, iterations=3, parallelism=1).

**Deprecated algorithms:** ECDSA brainpool-256, RSA-3072, and SHA3-512 from the 2018 spec are NOT supported in the 2026 implementation. Cryptographic agility is preserved by the type-code system (new algorithms get new type codes), but the initial implementation supports exactly one signature type (Ed25519) and two hash types (SHA2-256, BLAKE3).

### Deliverable 0D: Economic Rules Specification

Specify the deterministic arithmetic rules that all nodes must agree on:

**Recording fee formula:** Given `fee_rate` (from genesis parameters, in coins per byte), `total_shares` (current shares outstanding), `total_coins` (from genesis, constant), and `data_bytes` (size of the recorded assignment), the fee in shares is: `fee_shares = ceil(data_bytes * fee_rate * total_shares / total_coins)`. Specify: the `ceil()` rounds toward positive infinity (the recorder always gets at least as many shares as the fee rate implies). All arithmetic is arbitrary-precision integer; the division is the last operation. Provide 5 worked examples with concrete numbers.

**Coin display formula:** `user_coins = user_shares * total_coins / total_shares`. This is for display only and may use rational arithmetic. Specify rounding for display: truncate to 9 decimal places (nanocoin precision).

**Expiration:** Two modes, configured per-chain in genesis. Mode 1 (hard cutoff): shares not reassigned within `expiry_period` seconds of their recording timestamp are removed from `total_shares` at the next block. Mode 2 (age tax): at assignment time, shares older than `tax_start_age` have a tax deducted. Tax formula: `tax_fraction = 1 - 2^(-(age - tax_start_age) / tax_doubling_period)`. Taxed shares are retired. Specify: tax is computed on the source shares' age at the time the new block is recorded (not when the agreement was signed). Provide worked examples.

**Late recording:** An assignment agreement whose recording deadline has passed MAY still be recorded if and only if: (a) no giver's shares have been spent in a conflicting assignment, AND (b) no giver has recorded an explicit refutation referencing the agreement's hash. If both conditions hold, the late agreement is recorded normally. Specify: the escrow-like window between deadline expiry and potential late recording is bounded by share expiration — once the source shares expire, late recording becomes impossible. Wallets SHOULD warn givers about unexpired unrecorded agreements and offer one-tap refutation.

### Deliverable 0E: Conformance Test Vectors

A JSON file containing hand-computed test vectors for:

- VBC encoding/decoding (30+ values including edge cases)
- GMP integer encoding/decoding (15+ values)
- Rational fraction encoding/decoding (10+ values)
- Timestamp encoding (5+ values)
- SHA2-256 hashes of known inputs
- Ed25519 signatures of known inputs (using RFC 8032 test vectors plus AO-specific cases with timestamp concatenation)
- A complete minimal genesis block in both binary (hex) and JSON representation
- A complete minimal assignment agreement in both formats
- A complete minimal block containing one assignment, with the block hash computed correctly
- Recording fee calculations for 5 scenarios
- Share expiration calculations for both modes

These vectors are the ground truth. Any implementation that passes all conformance vectors is compatible. The vectors are maintained in version control alongside the code and updated whenever the specification changes.

### Acceptance Criteria (Phase 0)

A developer unfamiliar with the project can read the four specification documents (0A through 0D) and the test vectors (0E) and understand, without consulting any other document, exactly what to build for Phases 1 and 2. Every byte of the wire format is specified. Every arithmetic operation is deterministic. Every cryptographic operation specifies its inputs and outputs at the byte level.

## Phase 1: Foundation (Weeks 5–10)

Build the `ao-types` and `ao-crypto` crates, plus the genesis block creator. This phase is identical in scope to the original roadmap's Phase 1, but now builds against the resolved specifications from Phase 0 rather than interpreting the 2018 documents on the fly.

### Deliverables

**ao-types crate (`no_std`):** VBC codec. DataItem and DataContainer with serde support (custom binary serializer + JSON via `serde_json`). Type code registry generated from the Phase 0 specification table (not from the C++ `byteCodeDefinitions.json`, though that serves as a cross-reference). GMP-compatible big integer and rational encoding/decoding using `num-bigint` and `num-rational`. Timestamp type. Share and coin arithmetic with the deterministic rounding rules from Phase 0D. Separable item identification (by type code bitmask) and hash-substitution.

**ao-crypto crate (`no_std`):** Ed25519 key pair generation, signing, and verification via `ed25519-dalek` 2.x. SHA2-256 and BLAKE3 hashing. The "sign a DataContainer" operation that performs separable-item substitution, serializes, appends timestamp, and signs. Signature verification that reverses this process. Key-never-reuse tracking utilities.

**ao-cli (partial):** `ao genesis` — creates a genesis block per Phase 0 spec. `ao keygen` — generates an Ed25519 key pair. `ao inspect` — reads a binary block and prints JSON.

**Test suite:** All Phase 0E conformance vectors pass. Property-based tests (`proptest`) for VBC round-trips across the full i64 range. Round-trip serialization for every DataItem type. Signature sign/verify round-trips. Cross-compilation to aarch64 passes `cargo test`.

### Acceptance Criteria

Every conformance test vector passes. A genesis block round-trips through binary serialization to identical bytes. A genesis block round-trips through JSON and back to identical binary. Recording fee arithmetic matches the Phase 0D worked examples exactly.

## Phase 2: Single-Chain Recorder — TⒶ¹ (Weeks 11–20)

Build `ao-chain` and `ao-recorder`, plus the complete CLI toolset for assignment and balance checking. This phase is expanded from 8 to 10 weeks because it now includes full CLI tools (which the original split across phases) and more thorough integration testing.

### Deliverables

**ao-chain crate:** Genesis block loading and validation. UTXO database in SQLite: sequence ID → public key, share amount, recording block, spent/unspent/expired status. Block construction with correct sequence numbering, hash chaining, recording fee deduction, and blockmaker signature. Assignment agreement validation per the Phase 0 spec: all signatures verified, share availability checked against UTXO, timestamps ordered per R8, recording bid sufficient, no key reuse, recording deadline checked (including the late-recording rules from Phase 0D). Share expiration sweep per the configured mode (hard cutoff or age tax).

**ao-recorder binary:** Axum HTTP server with endpoints as specified in the original roadmap (`/chain/{id}/info`, `/chain/{id}/blocks`, `/chain/{id}/utxo/{seq_id}`, `/chain/{id}/submit`, `/chain/{id}/events` SSE). Configuration via TOML. Multi-chain hosting on a single instance. WebSocket endpoint for bidirectional client communication (`/chain/{id}/ws`).

**ao-cli (complete):** `ao assign` — constructs an assignment agreement, signs as giver, outputs partial agreement. `ao accept` — countersigns as receiver, submits to AOR. `ao refute` — records a refutation of an expired agreement. `ao balance` — queries AOR for unspent shares by public key. `ao history` — shows assignment history for a key or chain. `ao export` — exports a chain's blocks to a file for backup or transfer.

**Integration tests:** In-process AOR server. Genesis with 10 shares. Assign between generated keys through HTTP API. Verify UTXO updates. Verify recording fees produce correct coin inflation. Verify double-spend rejection. Verify expired share sweep. Verify late-recording acceptance and refutation. Verify timestamp ordering enforcement. 72-hour stress test on Pi (1 assignment/second).

**Deployment artifacts:** Dockerfile. `systemd` unit file. GitHub Actions CI for x86_64 and aarch64 builds. Cross-compilation via `cross-rs`.

### Acceptance Criteria

Two CLI users on different machines transfer shares through a single AOR server. The AOR runs on a Raspberry Pi for 72 hours under load without memory growth. A chain with 100,000 assignments validates from genesis in under 10 seconds on a Pi 5. Late recording and refutation work correctly in all edge cases specified in Phase 0D.

## Phase 3: Vendor and Consumer Apps — AOS + AOE (Weeks 21–28)

Identical in scope to the original roadmap's Phase 3. Build the React PWA with vendor (AOS) and consumer (AOE) views. The architecture from Phase 0A provides the API contract between the PWA and the recorder, eliminating design-while-coding.

### Deliverables

**Key management in browser:** Ed25519 via Web Crypto API (with `tweetnacl-js` fallback for Safari if needed). Private keys in IndexedDB, encrypted with Argon2id + XChaCha20-Poly1305 per Phase 0C. Key backup/import as encrypted files. Public key display as QR code.

**AOE (consumer) view:** Balance dashboard across chain issues. Chain discovery via URL or QR scan. Assignment flow: select issue, enter amount, scan/paste recipient key, review (including recording fee), confirm with passphrase, submit, display confirmation. SSE subscription for real-time balance updates. GPS-aware vendor map.

**AOS (vendor) view:** Vendor profile (name, location, hours, photo) published as separable items. Incoming assignment monitor with daily totals. Share float display with configurable limit warnings. Price card for new share asks.

**Shared components:** Transaction history. Settings. Offline mode with queued assignments. Service worker caching.

### Acceptance Criteria

Same as original roadmap: two phones complete a share assignment in under 3 seconds. PWA installs on iOS Safari and Android Chrome. Vendor map shows accurate GPS position. App shows cached balances in airplane mode.

## Phase 4: Market Making — AOI + Exchange (Weeks 29–38)

This phase includes more explicit specification of the exchange agent's trust model. The simplified exchange-agent-mediated model (Charlie holds inventory, absorbs settlement risk) is the primary deliverable. This is the phase that makes the system economically interesting, and it is where most of the business-logic complexity lives.

### Deliverables

**AOI view:** Portfolio dashboard across multiple chain issues. Automated trading rules engine per [AssignOnwardInvestor.html](../docs/html/AssignOnwardInvestor.html): bid ladders, ask ladders, float-sensitive pricing, position limits. Order book display. Automatic trade execution. ROI tracking per issue.

**Exchange agent infrastructure:** Exchange listing data structure (separable item on agent's chain advertising bid/ask for other issues). Lightweight directory: AOR servers publish a JSON index of hosted issues and known exchange agents. AOE apps discover exchange agents through this directory.

**Two-party exchange (simplified CAA):** Alice sends CCC to Charlie on the CCC chain; Charlie's AOI automatically sends BCG to Alice on the BCG chain. Two independent single-chain assignments, not atomically linked. Charlie absorbs settlement risk. This matches the Island Life narrative and delivers a working exchange without the complexity of Phase 6.

**MQTT inter-recorder messaging:** AOR servers publish new block notifications to per-chain MQTT topics. AOI instances subscribe to all chains they trade in. Consider bundling `rumqttd` as an embedded broker option for simple single-server deployments alongside the Mosquitto option for multi-server setups.

**Referral fee tracking:** Fee structures recorded as metadata in assignment agreements. Net-of-fees returns in AOI display.

### Acceptance Criteria

Same as original roadmap: automated BCG trades without human intervention. Simulated 5-AOI, 3-chain market reaches price equilibrium within 200 transactions. End-to-end CCC→BCG purchase through Charlie in under 10 seconds. MQTT broker handles 100 messages/second on Pi.

## Phase 5: Validation and Trust — AOV (Weeks 39–44)

Identical in scope to the original roadmap's Phase 5.

### Deliverables

**ao-validator binary:** Monitors configured AOR servers. Periodically computes rolled-up hash of recent blocks across all monitored chains. Stores locally, optionally publishes to an external anchor (Bitcoin OP_RETURN, or any tamper-evident public log). Alerts on detected history alteration via MQTT and webhook.

**Chain integrity API:** `GET /validate/{chain_id}` returns last validated block height, timestamp, and anchor reference. AOE displays trust indicator per chain.

**AOR cross-reference:** AOR servers include validator endorsement in chain info responses. Clients can verify independently.

**Vendor credential linking:** Verifiable credential references as separable items (URL + content hash). AOE displays credentials on vendor profile with hash-match indicator. Consider W3C DID compatibility for credential identifiers.

### Acceptance Criteria

Same as original: detects simulated history alteration within one polling interval. Rolled-up hash independently verifiable. Validator runs 30 days on Pi without memory growth.

## Phase 6: Atomic Multi-Chain Exchange — TⒶ² (Weeks 45–54)

Identical in scope to the original roadmap's Phase 6. The full CAA (Conditional Assignment Agreement) protocol from [MultichainExchange.html](../docs/html/MultichainExchange.html). This phase can be deferred indefinitely if the exchange-agent model from Phase 4 proves sufficient.

### Deliverables

**CAA data structures in ao-types:** Conditional assignment agreement extending the basic agreement with ordered chain list, escrow period, per-chain terms, recording proof slots. State machine: proposed → signed → recording → binding → finalized, with timeout transitions to expired.

**Escrow support in ao-chain:** New UTXO state: escrowed-pending-CAA. Escrow deadline enforcement. Automatic release on timeout.

**CAA coordinator in ao-recorder:** Ouroboros recording sequence: record on chain 1 (escrow), forward with proof to chain 2, chain 2 records (escrow), binding proof back-recorded on chain 1. HTTP POST relay between AOR servers. MQTT notifications at each step.

**Timeout and recovery:** Exponential backoff retries. Escrow release on deadline. No shares permanently locked by failed CAA.

**AOE multi-chain transaction flow:** Transparent UX: "Give 1 BCG to Bob (costs 12 CCC via Charlie)" with progress indicator and failure explanation.

### Acceptance Criteria

Same as original: three-party two-chain CAA completes in under 30 seconds. Server failure causes correct escrow release. Chaos testing confirms no share loss or double-spend under random process kills and network partitions.

## Summary of Changes from Original Roadmap

| Change | Rationale |
|--------|-----------|
| Added Phase 0 (4 weeks) | Resolves specification ambiguities before coding begins. Produces architecture docs as first-class deliverables. Generates conformance test vectors that define compatibility. |
| Phase 1 shifted to weeks 5–10 | Now builds against resolved specs. Same scope, higher confidence. |
| Phase 2 expanded to 10 weeks | Includes complete CLI toolset (was split across original phases). Late-recording and refutation mechanics add complexity. |
| Total extended to ~54 weeks | 6 additional weeks for architecture and more realistic Phase 2 + Phase 4 timelines. Each phase still produces a working, demonstrable system. |
| Conformance test vectors as a deliverable | The 2018 docs lack test vectors. Without them, any second implementation (or even a refactor of the first) risks silent incompatibility. |
| Explicit late-recording specification | This was a design decision hiding in AssignmentAgreement.html. Now specified upfront with wallet UX implications. |
| Dropped SHA3-512 entirely | BLAKE3 replaces it. Simpler to support two hash algorithms than three, especially when SHA3-512 was only ever used via OpenPGP in the C++ code. |
| Wallet encryption specified | The original roadmap mentioned "encrypted with a user-chosen passphrase" but did not name algorithms. Now specified: Argon2id + XChaCha20-Poly1305. |
| `rumqttd` as optional embedded broker | Reduces deployment complexity for simple setups. Not a replacement for Mosquitto in multi-server topologies. |

## What This Roadmap Does Not Cover

The following are explicitly deferred, consistent with the original roadmap's "Phase 7 and Beyond":

**TⒶ³ (multiple competing recorders)** and **TⒶ⁴ (underwriters and error checkers)** remain unspecified. The 2018 design documents describe these at the conceptual level only. Specification work equivalent to Phase 0 would be needed before implementation.

**Regulatory compliance.** The 2026 Review correctly identifies this as the other hard problem alongside cold start. The regulatory landscape for commodity-backed tokens remains unsettled. Any real deployment needs jurisdiction-specific legal counsel. The architecture is designed to be compatible with commodity-backed gift card / loyalty point frameworks (the most favorable regulatory category), but this is not a legal opinion.

**Pilot deployment and cold-start strategy.** The roadmap produces working software. Finding the first island where Eddie, Bob, Charlie, and Gene all show up willing to try it remains, as the 2026 Review noted, the central unanswered question. A pilot deployment plan should be developed in parallel with Phase 3 or 4, but it is a business plan, not a software deliverable.
