# Assign Onward — Project Summary

## What It Is

Assign Onward is a federated blockchain system designed for small businesses and local economies. Instead of one global chain, it envisions millions of tiny independent blockchains — each representing a specific real-world business or asset. Bob's Curry Goat plates are one chain. Rita's Mango Futures are another. Dave's Bike Rentals are a third. These chains interoperate through market makers and exchange agents who hold inventory in multiple chains and bridge value between them.

The design is the opposite of Bitcoin in almost every way: no proof of work, no global consensus, no mining, no energy waste. Security comes from economic stake (underwriters put up their own shares as collateral), reputation (web of trust), and transparency (all non-separable chain data is public). The system is designed so a single person can run a blockchain on a Raspberry Pi in a hall closet.

## The Design Documents (2018)

The project is documented through ~38 HTML files in `docs/html/`, written in 2018. The central narrative is [IslandLife.html](../docs/html/IslandLife.html), a detailed story set on a Caribbean island where small businesses adopt the system. The characters map to system roles:

| Character | Role | Chain/App |
|-----------|------|-----------|
| Bob | Vendor/issuer | BCG (Bob's Curry Goat), uses AOS |
| Alice | Tourist/consumer | Uses AOE to discover vendors and pay |
| Charlie | Cash-exchange agent | CCC (Charlie's Cruise Credits, pegged to USD), runs AOI |
| Eddie | Early investor/venue operator | First BCG investor |
| Gene | System administrator | Sets up AOR servers, genesis blocks |
| Faythe | Server host | Runs AOR backup from her closet |
| Ted | Regional intermediary | TCC (Ted's Caribbean Credits) across islands |
| Victor | Forensic auditor | Runs AOV validator |
| Walter | Governance/oversight | High-roller investor, keeps ecosystem honest |
| Ziggy | Aggressive exchange agent | ZIC, needs oversight to stay honest |
| Larry/Mal/Nick | Attackers | Social engineering, network, physical theft |
| Oscar | Incompetent issuer | Over-issues, crashes his own chain value |
| Quincy | Outside algorithmic trader | Demonstrates how fees resist HFT |

The technical specifications are spread across documents covering the protocol ([ProtocolAO1.html](../docs/html/ProtocolAO1.html)), requirements ([Requirements.html](../docs/html/Requirements.html)), wire format ([VariableByteCoding.html](../docs/html/VariableByteCoding.html)), economics ([SharesCoins.html](../docs/html/SharesCoins.html), [Fees.html](../docs/html/Fees.html)), security ([Attacks.html](../docs/html/Attacks.html), [Trouble.html](../docs/html/Trouble.html)), and more.

## Non-Negotiable Principles

These are hard requirements from [CorePrinciples.html](../docs/html/CorePrinciples.html) — any implementation must honor all of them:

1. **No proof of work.** Trust comes from economic stake and reputation, never from mining.
2. **Full transparency** of all non-separable blockchain data.
3. **Single-use keys** for share transfers. Each public key receives shares exactly once and gives them away exactly once.
4. **Mutual consent.** Both offer and acceptance required — nobody can force shares onto a key.
5. **Timestamped signatures.** Every signature includes the time it was made.
6. **Share expiration.** No immortal shares. Lost keys must not permanently lock value.
7. **Separable items.** Content can be stripped from blocks without breaking chain integrity (for censorship compliance).
8. **Cryptographic agility.** The algorithm set is extensible via type codes.
9. **Open source, MIT license.**
10. **Scalability through federation** — many small chains, not one big one.
11. **Wire format thrift.** Messages must be compact enough to traverse not only wired internet and 5G, but also low-bandwidth networks such as [Meshtastic](https://meshtastic.org/) LoRa mesh and similar constrained links.

## How the Economics Work

**Genesis:** A chain starts with a genesis block creating a large pool of shares (~2⁸⁶) and declaring a fixed coin count (~2⁸³) for display purposes. All shares initially belong to the issuer.

**Transfers:** Atomic assignment — all shares under a key move at once. If you want to send half, the other half goes to a fresh key you control. This makes double-spend detection trivial: one boolean per key (spent or not).

**Fees:** A per-byte recording fee in shares is deducted from each transfer. These shares are retired (destroyed), reducing total shares outstanding. Since coins = `your_shares * total_coins / total_shares`, retiring shares causes passive coin inflation for all remaining holders. The recorder earns value by providing a service, not by mining.

**Expiration:** Shares expire if not refreshed (self-assigned to a new key) within a configurable period. Expired shares are retired. This handles lost keys gracefully — abandoned value returns to the community over time.

**Exchange:** Exchange agents (like Charlie) hold inventory in multiple chains. Alice pays Charlie 12 CCC, Charlie sends Alice 1 BCG. Two independent single-chain assignments. Charlie absorbs the settlement risk and earns a margin. A more sophisticated atomic cross-chain protocol (CAA) is designed but deferred.

## What's Been Built

**C++ codebase (~15,600 lines, 2018–2021):** Implements only the data serialization layer — VBC encoding, DataItem types, hash wrappers, key structures, GMP integer storage, and a JSON codec. No protocol logic, no networking, no user-facing apps. Uses the 2018 crypto choices (ECDSA brainpool-256, RSA-3072, SHA3-512 via OpenPGP). Tests are GUI-based (Qt widgets), not automated. Last commit September 2021.

**No Rust code exists yet.** The 2026 reboot starts from scratch.

The C++ code is useful as a reference for the binary wire format and the `byteCodeDefinitions.json` type code registry, but it cannot be ported directly because the cryptographic algorithms have changed.

## The 2026 Technology Stack

| Layer | Choice | Notes |
|-------|--------|-------|
| Language | Rust (edition 2024) | Memory safety for crypto, cross-compile to ARM |
| Signatures | Ed25519 via `ring` 0.17 | Replaces ECDSA brainpool-256 and RSA-3072 |
| Hashes | SHA2-256 + BLAKE3 | SHA3-512 dropped entirely |
| Type codes | 44 codes (1–39, -1, -2, 64–68) | Compact registry, extensible via type-code dispatch |
| Big integers | `num-bigint` + `num-rational` | Pure Rust, replaces GMP |
| Serialization | `serde` with custom binary + JSON | Preserves VBC binary on-chain format |
| HTTP server | Axum 0.8+ | Async, lightweight, tokio ecosystem |
| Pub/sub | MQTT via `rumqttc` (+ optional `rumqttd` embedded broker) | Lighter than RabbitMQ, runs on Pi |
| Storage | SQLite via `rusqlite` | Single-file, embedded, adequate for small chains |
| Client UI | React PWA (TypeScript) | Cross-platform, no app store, offline capable |
| Client crypto | Web Crypto API (Ed25519 now in all major browsers) | `tweetnacl-js` as Safari fallback only |
| Wallet encryption | Argon2id + XChaCha20-Poly1305 | For private key storage in browser and CLI |
| Testing | `cargo test` + `proptest` + conformance vectors | Property-based + hand-computed ground truth |

## The Roadmap

See [ROADMAP.md](ROADMAP.md) for the full plan. Summary:

| Phase | Weeks | Deliverables |
|-------|-------|-------------|
| 0: Architecture & Specification | 1–4 | System architecture doc, wire format spec, crypto choices doc, economic rules spec, conformance test vectors |
| 1: Foundation | 5–10 | `ao-types` + `ao-crypto` Rust crates, genesis CLI tool |
| 2: Single-Chain Recorder (TⒶ¹) | 11–20 | `ao-chain` + `ao-recorder` (Axum HTTP), complete CLI, SQLite UTXO |
| 3: Vendor + Consumer Apps | 21–28 | React PWA with AOS (vendor) and AOE (consumer) views |
| 4: Market Making + Exchange | 29–38 | AOI (investor) view, exchange agents, MQTT, automated trading |
| 5: Validation + Trust | 39–44 | AOV validator, anchor proofs to external chains, vendor credentials |
| 6: Atomic Multi-Chain (TⒶ²) | 45–54 | Full CAA escrow protocol with timeout recovery |

Phase 0 resolves the numerous ambiguities in the 2018 documents (VBC negative encoding, GMP zero representation, separability bit position, signature byte format, deterministic fee rounding rules, late-recording semantics) in writing before coding begins.

## Known Risks and Hard Problems

**Technical:**
- `rumqttc` has sporadic maintenance (18-month gap between releases). `rumqttd` embedded broker is the hedge.
- `cross-rs` is stale on crates.io. GitHub Actions aarch64 runners are the better cross-compilation path from Windows.
- Pi 5 needs SSD (not SD card) and active cooling for reliable server operation.
- `ed25519-dalek` was replaced by `ring` 0.17 early in Phase 1 — see [lessons/wrong-test-vector.md](lessons/wrong-test-vector.md).

**Non-technical (the real hard problems):**
- **Cold start.** The system only delivers value when vendors, consumers, and exchange agents are all using it in the same area simultaneously. Classic two-sided marketplace problem.
- **Regulatory.** Exchange agents likely qualify as money transmitters. Commodity-backed tokens sit in an unclear regulatory space — closer to gift cards than securities, but the line is blurry. Jurisdiction-specific legal counsel needed for any real deployment.
- **Competition.** Square, Venmo, M-Pesa already solve "pay the vendor without cash" in most markets. AO's advantage is in contexts where those systems are unavailable, unreliable, or too expensive.

## Key Implementation Subtleties

Things that are easy to get wrong:

- **VBC encoding:** Sign bit in bit 0, continuation flag in MSB, LSB-first across bytes. The 2018 docs lack negative number examples — Phase 0 must resolve this with test vectors.
- **Separable item substitution:** Before signing, walk the DataItem tree, identify separable items by type-code bitmask, replace each with its SHA2-256 hash. Getting this wrong breaks all signature verification.
- **Deterministic arithmetic:** Fee calculations use `ceil()` rounding on arbitrary-precision integers. The division must be the last operation. Every node must compute the identical result.
- **Late recording:** An expired assignment can still be recorded if the source shares haven't been spent or explicitly refuted. Bounded by share expiration. Wallets must warn users.
- **Timestamps:** Unix seconds × 189,000,000 (~5.29ns resolution), 8 bytes big-endian. Must be strictly monotonic per actor — bump by 1 if wall clock hasn't advanced.
- **GMP rational fractions:** Nested VBC size encoding (outer total, inner numerator size, denominator size by subtraction). Sign only in numerator. Zero denominator is an error.

## Repository Layout

```
aosuite/
├── 2026/                          # Development folder (new work goes here)
│   ├── ROADMAP.md                 # Development roadmap
│   └── ProjectSummary.md          # This document
├── docs/html/                     # Original 2018 design documents (read-only)
│   ├── IslandLife.html            # Central narrative / use cases
│   ├── CorePrinciples.html        # Non-negotiable assertions
│   ├── ProtocolAO1.html           # A1 protocol spec
│   ├── Requirements.html          # Formal requirements R1–R8
│   ├── Architecture.html          # Actor model, transaction lifecycle
│   ├── SharesCoins.html           # Share/coin math
│   ├── Fees.html                  # Recording fee formulas
│   ├── VariableByteCoding.html    # VBC wire format
│   ├── MultichainExchange.html    # CAA atomic exchange protocol
│   ├── 2026review.html            # Technology feasibility review
│   └── ... (~28 more docs)
├── apps/                          # Original C++ codebase (read-only reference)
│   ├── CodeForm/byteCodeDefinitions.json  # Type code registry
│   ├── BlockDef/riceyCodes.cpp    # VBC encoding reference implementation
│   ├── DataItems/                 # 60 data type classes
│   └── ...
└── README.md
```

Everything outside `2026/` is read-only reference material.
