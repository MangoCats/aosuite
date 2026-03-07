# System Architecture — Deliverable 0A

This document defines the actor model, system topology, data flows, and security model for the Assign Onward A1 protocol (single-recorder chains). It is the implementation-grade reference for Phases 1–2.

Related specs: [WireFormat.md](WireFormat.md) (0B), [CryptoChoices.md](CryptoChoices.md) (0C), [EconomicRules.md](EconomicRules.md) (0D).

---

## 1. Actors

### 1.1 Asset Organizer (AOE / AOS / AOI)

End-user wallet software. Manages private keys, proposes and accepts assignments, displays balances. Three presentation modes share one core:

| Mode | Use Case | Authorization |
|------|----------|---------------|
| **AOE** (consumer) | Pay vendors, view balance, discover chains | Per-transaction (passphrase / biometric) |
| **AOS** (vendor) | Accept payments, manage inventory, set prices | Per-transaction |
| **AOI** (investor) | Automated market making, exchange arbitrage | Standing rules, no per-transaction prompt |

**Capabilities:** Generate Ed25519 key pairs. Sign assignment agreements. Submit signed agreements to a Recorder. Query chain state. Store encrypted private keys locally (Argon2id + XChaCha20-Poly1305).

**Constraints:** Never sends private keys over any network. Cannot verify chain integrity independently — relies on Recorder responses and (optionally) Validator attestations. Treats all Recorder responses as unverified until cross-checked.

### 1.2 Recorder (AOR)

Single authoritative server per chain. Combines the 2018 design's separate "underwriter" and "chainmaker" roles into one entity. In A1, there is exactly one Recorder per chain — no forks, no competing block proposals.

**Capabilities:** Validate assignment agreements (signatures, UTXO availability, timestamps, key uniqueness, fee sufficiency). Construct and sign blocks. Serve chain state queries. Publish block notifications via SSE, WebSocket, and MQTT.

**Constraints:** Never sees or handles private keys — only public keys and signatures. Cannot forge assignments (lacks private keys of participants). Cannot silently rewrite history (Validators monitor chain hashes). Can censor by refusing to record valid agreements — mitigated by chain portability (users can migrate to a new Recorder).

**Trust position:** The Recorder is a trusted-but-auditable authority. Users trust it to record honestly; Validators verify that it does. This is appropriate for small community chains where the Recorder operator is a known entity.

### 1.3 Validator (AOV)

Independent auditor that monitors one or more Recorder chains. Periodically fetches blocks, recomputes hashes, and compares against its own state.

**Capabilities:** Detect chain alteration (modified or deleted blocks). Anchor chain hashes to external systems (e.g., Bitcoin OP_RETURN, public log). Publish attestations of chain integrity. Alert on detected tampering.

**Constraints:** Can only *invalidate* (detect tampering), never *validate* (prove absence of tampering). A clean Validator report means "no tampering detected so far," not "this chain is trustworthy." Cannot modify chain data. Cannot access private keys.

### 1.4 Checker (Future: TⒶ³/TⒶ⁴)

Earns bounties by finding errors in recorded blocks. Not implemented in A1. Noted here for forward compatibility — the type-code system reserves space for checker-related data items.

---

## 2. System Topology

### 2.1 A1 Deployment

```
┌──────────┐     HTTPS      ┌──────────┐     HTTPS      ┌──────────┐
│  AOE/AOS │◄──────────────►│   AOR    │◄──────────────►│   AOV    │
│ (wallet) │                │(recorder)│                │(validator)│
└──────────┘                └──────────┘                └──────────┘
      ▲                      │  │  │                          │
      │ SSE/WS               │  │  │ MQTT                     │
      └──────────────────────┘  │  └──────────────┐           │
                                │                  ▼           │
                           SQLite DB         ┌──────────┐     │
                           (local)           │  MQTT    │     │
                                             │  Broker  │     │
                                             └──────────┘     │
                                                  ▲           │
                                                  │ MQTT      │
                                             ┌────┴─────┐     │
                                             │   AOI    │◄────┘
                                             │(investor)│ HTTPS
                                             └──────────┘
```

### 2.2 Connection Types

| Connection | Protocol | TLS | Purpose |
|------------|----------|-----|---------|
| Wallet → Recorder | HTTPS (REST) | Required | Submit agreements, query state |
| Recorder → Wallet | SSE or WebSocket | Required | Real-time block notifications |
| Recorder → MQTT Broker | MQTT 5.0 | Required in production | Block publication to per-chain topics |
| AOI → MQTT Broker | MQTT 5.0 | Required in production | Subscribe to block events |
| AOI → Recorder | HTTPS (REST) | Required | Submit agreements, query state |
| Validator → Recorder | HTTPS (REST) | Required | Fetch blocks for verification |
| Wallet ↔ Wallet | Out-of-band | N/A | Exchange public keys, negotiate agreements |

TLS is required for all network connections in production. Development/testing may use plaintext for debugging.

### 2.3 Chain Discovery

A chain is identified by the SHA2-256 hash of its genesis block. Discovery mechanisms:

1. **Direct URL:** `https://recorder.example.com/chain/{genesis_hash}/info` — returns chain metadata (name, coin label, current height, fee rate).
2. **QR code:** Encodes the chain info URL. Scanned by AOE at point of sale.
3. **MQTT topic:** `ao/chain/{genesis_hash}/blocks` — live block feed.

No global directory. Chains are discovered through social channels (vendor signage, web links, word of mouth).

---

## 3. Data Flows

### 3.1 Single-Chain Assignment (A1)

Alice (giver) transfers shares to Bob (receiver) through Recorder R.

```
Alice (AOE)              Bob (AOS)              Recorder (AOR)
    │                        │                        │
    │  1. Negotiate off-band │                        │
    │◄──────────────────────►│                        │
    │                        │                        │
    │  2. GET /chain/{id}/info                        │
    │───────────────────────────────────────────────►  │
    │  ◄─── chain metadata (fee rate, height)         │
    │                        │                        │
    │  3. Build agreement    │                        │
    │  (amounts, keys, fee,  │                        │
    │   deadline)            │                        │
    │                        │                        │
    │  4. Sign agreement     │                        │
    │  (Alice's giver sig)   │                        │
    │                        │                        │
    │  5. Send partial agreement to Bob               │
    │───────────────────────►│                        │
    │                        │                        │
    │                        │  6. Verify terms       │
    │                        │  7. Generate fresh key │
    │                        │  8. Sign agreement     │
    │                        │     (Bob's receiver sig)
    │                        │                        │
    │  9. Receive fully signed agreement              │
    │◄───────────────────────│                        │
    │                        │                        │
    │  10. POST /chain/{id}/submit                    │
    │───────────────────────────────────────────────►  │
    │                        │                        │
    │                        │  11. Validate:         │
    │                        │    - All signatures    │
    │                        │    - UTXO available    │
    │                        │    - Timestamps valid  │
    │                        │    - Key not reused    │
    │                        │    - Fee sufficient    │
    │                        │    - Deadline not past │
    │                        │                        │
    │                        │  12. Construct block   │
    │                        │    - Assign seq IDs    │
    │                        │    - Deduct fee        │
    │                        │    - Chain hash        │
    │                        │    - Sign block        │
    │                        │                        │
    │  13. ◄── SSE: new block event                   │
    │                        │  14. ◄── SSE: new block│
    │                        │                        │
    │  15. Verify block      │  16. Verify block      │
    │      contains our      │      contains our      │
    │      assignment        │      assignment        │
```

**Steps 1–4:** Off-band negotiation. Alice and Bob agree on amount and terms. Alice builds the agreement structure, signs her portion (giver signature includes timestamp). The agreement includes Alice's source key (by sequence ID), Bob's fresh receiver public key, share amounts, recording fee bid, and deadline.

**Steps 5–9:** Bob receives the partial agreement, verifies the terms are acceptable, generates a fresh Ed25519 key pair for receiving, signs his portion (receiver signature includes timestamp), and returns the fully-signed agreement. Alice may also need to sign for her change key (a fresh key receiving her remaining shares).

**Steps 10–12:** Alice submits the fully-signed agreement to the Recorder via HTTPS POST. The Recorder performs all validation checks. If valid, it constructs a new block containing the assignment, assigns sequence IDs to new keys, deducts the recording fee (shares retired), chains the block hash to the previous block, and signs the block.

**Steps 13–16:** The Recorder broadcasts the new block via SSE/WebSocket to connected clients and publishes to the MQTT topic. Alice and Bob verify the block contains their assignment.

### 3.2 Exchange via Agent (Phase 4)

Alice pays Charlie (exchange agent) CCC to receive BCG from Charlie. Two independent A1 assignments on separate chains:

1. Alice → Charlie: 12 CCC on CCC chain
2. Charlie → Alice: 1 BCG on BCG chain

Charlie's AOI monitors both chains and executes step 2 automatically upon confirming step 1. Charlie absorbs settlement risk (time gap between the two recordings). This is simpler and faster than atomic CAA escrow (Phase 6) but requires trust in the exchange agent.

### 3.3 Self-Assignment (Share Refresh)

A share holder assigns all shares from their current key to a fresh key they control. Prevents expiration. Costs a recording fee. Identical flow to 3.1 except the giver and receiver are the same person.

---

## 4. Recorder API (A1)

### 4.1 Endpoints

| Method | Endpoint | Request | Response |
|--------|----------|---------|----------|
| GET | `/chains` | — | Array of hosted chains: chain_id, symbol, height, exchange_agents |
| POST | `/chains` | Genesis JSON + optional blockmaker_seed | Chain info (201 Created) |
| GET | `/chain/{id}/info` | — | Chain metadata: name, coin label, genesis hash, current height, total shares, total coins, fee rate, expiration parameters, validator endorsements |
| GET | `/chain/{id}/blocks?from={height}&to={height}` | — | Array of blocks in JSON. Paginated (max 1000 per request). |
| GET | `/chain/{id}/utxo/{seq_id}` | — | UTXO status: key, share amount, block recorded, spent/unspent/escrowed/expired |
| POST | `/chain/{id}/submit` | Signed AUTHORIZATION JSON | Block info: height, hash, timestamp, shares_out, first_seq, seq_count |
| POST | `/chain/{id}/refute` | `{"agreement_hash": "<hex>"}` | 200 OK (idempotent) |
| GET | `/chain/{id}/events` | — | SSE stream of new block notifications |
| GET | `/chain/{id}/ws` | — | WebSocket upgrade for bidirectional block notifications |
| POST | `/chain/{id}/exchange-agent` | Exchange agent registration JSON | 200 OK |

Phase 6 adds CAA endpoints — see [AtomicExchange.md](AtomicExchange.md) §4.3.

`{id}` is the hex-encoded SHA2-256 hash of the genesis block (64 characters).

All responses use `Content-Type: application/json`. The body limit is 256 KB.

### 4.2 Error Response Format

All error responses return a JSON object with an `error` field:

```json
{"error": "human-readable error description"}
```

HTTP status codes: 400 (bad request / validation failure), 404 (chain or resource not found), 409 (conflict, e.g., chain already hosted), 500 (internal error).

---

## 5. Security Model

### 5.1 Key Custody

| Component | Keys Held | Storage |
|-----------|-----------|---------|
| AOE/AOS (browser) | User's Ed25519 private key seeds | IndexedDB, encrypted with Argon2id + XChaCha20-Poly1305 |
| AOI (server) | Automated trading key seeds | Encrypted file or HSM |
| AOR | Recorder's block-signing key | Encrypted file, loaded into memory at startup |
| AOV | Validator's attestation key | Encrypted file |

**Invariant:** Private keys never leave the device that generated them. The Recorder never sees user private keys — it receives only public keys and signatures.

### 5.2 Compromise Analysis

| Compromised Component | Impact | Mitigation |
|------------------------|--------|------------|
| User's wallet device | Attacker can spend user's shares | Per-transaction passphrase/biometric. Share expiration limits exposure window. |
| Recorder server | Can censor (refuse valid assignments). Can attempt history rewrite. | Validator detects rewrites. Users can export chain and migrate to new Recorder. Censorship is visible (refused submissions return errors). |
| Validator | Can falsely claim chain is valid | Multiple independent Validators. Validator can only invalidate, never validate — false "clean" report is indistinguishable from honest one, but does not create new risk. |
| MQTT broker | Can drop or delay block notifications | Does not affect chain integrity. Clients can poll Recorder directly. Stale notifications are detectable via block height. |
| TLS CA | Man-in-the-middle on Recorder connections | Certificate pinning for known Recorders. Out-of-band genesis block hash verification. |

### 5.3 Double-Spend Prevention

In A1 (single Recorder), double-spend prevention is trivial:

1. Each public key receives shares exactly once (assigned a unique sequence ID).
2. Each sequence ID has a binary state: unspent or spent.
3. The Recorder checks this state before including an assignment in a block.
4. Since there is exactly one Recorder, there are no forks and no race conditions.

A compromised Recorder could theoretically record conflicting assignments, but this would be detected by any Validator (or any client replaying the chain from genesis).

---

## 6. Non-Negotiable Principles as Testable Assertions

| # | Principle | Testable Assertion |
|---|-----------|-------------------|
| 1 | No proof of work | Block construction requires only the Recorder's signature, no hash puzzle. Block validation requires only signature verification and state checks. |
| 2 | Full transparency | All non-separable block data is served to any requester without authentication. No access control on chain reads. |
| 3 | Single-use keys | A public key that appears as a receiver in any recorded block MUST NOT appear as a receiver in any other recorded block on the same chain. Enforced by Recorder; verified by Validator. |
| 4 | Mutual consent | An assignment is valid only if it contains valid signatures from ALL givers and ALL receivers. The Recorder rejects partially-signed agreements. |
| 5 | Timestamped signatures | Every signature includes an 8-byte timestamp. The signed data is `hash(serialized_agreement_with_separable_substitution) || timestamp`. |
| 6 | Share expiration | Shares not refreshed within the chain's expiration period are retired in the next expiration sweep. No configuration permits immortal shares. |
| 7 | Separable items | Removing a separable item and replacing it with its SHA2-256 hash does not invalidate any signature in the block. |
| 8 | Cryptographic agility | Every signature and hash on-chain is prefixed with a type code identifying the algorithm. New algorithms can be added by assigning new type codes without breaking existing data. Initial choices: Ed25519 (signatures), SHA2-256 (chain integrity hashing), BLAKE3 (content-addressing, first identified alternate hash). All protocol code must treat algorithm selection as a type-code dispatch, never hard-code a single algorithm. |
| 9 | Open source | All server and client code is MIT-licensed. No proprietary components required for operation. |
| 10 | Federation | Each chain is fully independent. No shared state between chains. Cross-chain exchange is mediated by agents, not by protocol. |
| 11 | Wire format thrift | A minimal assignment agreement (1 giver, 1 receiver, no separable items) serializes to under 256 bytes in binary format. |

---

## 7. Cryptographic Agility

Algorithm agility is a core architectural requirement, not an afterthought. The type-code system ([WireFormat.md](WireFormat.md) §3) is the mechanism.

### 7.1 Type-Code Dispatch

Every cryptographic output on-chain (signature, hash, public key) is a DataItem with a type code identifying the algorithm. Verification code MUST dispatch on the type code to select the implementation. This means:

- A block can contain Ed25519 signatures alongside a future Ed448 or post-quantum signature — the verifier handles both via type-code dispatch.
- SHA2-256 hashes and BLAKE3 hashes can coexist in the same chain, each identified by its type code.
- Adding a new algorithm requires only: (1) assign a type code, (2) implement encode/decode/verify for that code, (3) update Recorder acceptance policy.

**Implementation rule:** All protocol code MUST treat algorithm selection as a type-code lookup. No function should hard-code `Ed25519_Verify(...)` — it should dispatch on the type code: `verify_signature(type_code, key, data, sig)`. This applies to signing, hashing, and key generation.

**Current status:** The initial implementation uses only Ed25519 and SHA2-256, with a single code path per function. This is acceptable as a starting point — but the interfaces must be structured so that adding a second algorithm is a dispatch addition, not a rewrite.

### 7.2 Initial Algorithm Set

| Function | Primary | First Alternate | Future Candidates |
|----------|---------|-----------------|-------------------|
| Signatures | Ed25519 (type 1–2) | *(reserved)* | Ed448, ML-DSA (post-quantum) |
| Chain integrity | SHA2-256 (type 3) | BLAKE3 (type 4) | SHA2-384, SHA2-512 |

BLAKE3 is identified as the first alternate hash: structurally different from SHA2 (ChaCha-based ARX vs Merkle-Damgård), providing a hedge if the SHA2 family is ever compromised. See [CryptoChoices.md](CryptoChoices.md) §3 for the full rationale.

### 7.3 Backward Compatibility

Adding a new algorithm MUST NOT invalidate existing transaction logs. Specifically:

1. **Existing blocks remain valid.** A chain that used Ed25519 for blocks 0–10,000 continues to verify identically after Ed448 support is added. The verifier dispatches on the type code found in each block.
2. **No forced migration.** Participants are not required to adopt new algorithms. A giver with an Ed25519 key can sign an assignment even after the Recorder begins accepting Ed448 signatures.
3. **Genesis parameters unchanged.** Algorithm availability is a software capability, not a genesis parameter. A chain created before Ed448 support was added benefits from it automatically when the Recorder software is upgraded.
4. **Chain replay determinism.** Replaying a chain from genesis produces identical results regardless of which algorithms the replaying software supports, as long as it supports all algorithms actually used in the chain.

### 7.4 Interoperability and Algorithm Selection

When multiple algorithms are available, participants and recorders must agree on which to use. The selection rules:

1. **Signer's choice.** The signer selects the signature algorithm. The resulting `AUTH_SIG` contains a type-coded `ED25519_SIG` (or future `ED448_SIG`, etc.). The verifier accepts any algorithm it supports.
2. **Recorder acceptance policy.** The Recorder maintains a set of accepted signature and hash algorithms. It rejects submissions using algorithms outside this set. The accepted set is a software configuration, not an on-chain parameter. A Recorder SHOULD accept all algorithms it can verify; it MUST accept at least one signature algorithm and SHA2-256 for chain integrity.
3. **Overlap requirement for transactions.** A transaction is valid if every signature in it uses an algorithm the Recorder accepts. For cross-chain CAA transactions, each chain's Recorder independently verifies the signatures relevant to its component — there is no requirement that all chains accept the same algorithm set.
4. **Hash algorithm for chain integrity.** The block hash chain (`PREV_HASH` → `SHA256` in `BLOCK`) uses the hash algorithm established in the chain's first block. Changing the chain integrity hash requires a chain migration (new genesis). Content-addressing hashes (separable item substitution) can use any supported hash algorithm, identified by type code.
5. **Algorithm deprecation.** A Recorder MAY stop accepting new signatures with a deprecated algorithm while continuing to verify existing blocks that used it. This is a one-way ratchet: once deprecated, an algorithm is not re-enabled. Deprecation is announced via Recorder configuration and SHOULD be communicated to clients via the chain info endpoint.

---

## 8. Scope Boundaries

**In scope for A1 (Phases 1–2):** Single-recorder chains. Assignment agreements. Block construction. UTXO tracking. Fee deduction. Share expiration. CLI and HTTP API.

**Deferred to Phase 4:** Exchange agents. MQTT pub/sub. AOI automated trading. Referral fees.

**Deferred to Phase 5:** Validators. External anchoring. Vendor credentials. See [ValidationAndTrust.md](ValidationAndTrust.md).

**Deferred to Phase 6:** CAA atomic multi-chain escrow. Multiple competing recorders.

**Out of scope entirely:** TⒶ³ (multiple competing recorders without CAA). TⒶ⁴ (underwriter/checker bounty system). Global chain directory. Regulatory compliance framework.
