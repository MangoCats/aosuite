# Validation and Trust — Deliverable 5A

Specifies the validator protocol, chain integrity verification, external anchoring, and vendor credential references for Phase 5 of the Assign Onward A1 protocol.

Related specs: [Architecture.md](Architecture.md) (0A), [WireFormat.md](WireFormat.md) (0B), [CryptoChoices.md](CryptoChoices.md) (0C).

**Design principle:** The system must function at acceptable risk without external anchors or W3C credentials. These features reduce risk when available but are never required for operation. A chain with zero validators and zero anchors is a valid chain. Trust comes from reputation and economic stake; validation infrastructure adds confidence, not necessity.

---

## 1. Chain Integrity Verification

### 1.1 Rolled-Up Hash

The validator maintains a cumulative hash per monitored chain — the **rolled-up hash** — that makes any alteration of chain history detectable.

```
rolled_hash(0) = SHA-256([0; 32] || block_hash(0))       # genesis
rolled_hash(n) = SHA-256(rolled_hash(n-1) || block_hash(n))  # block n
```

Where `block_hash(n)` is the `SHA256` item embedded in `BLOCK` at height `n` — the SHA2-256 hash of the `BLOCK_SIGNED` encoding.

**Properties:**
- Deterministic: two honest observers processing the same blocks produce identical rolled hashes at every height.
- Tamper-evident: modifying, inserting, or deleting any block changes all subsequent rolled hashes.
- Incremental: verification can resume from any previously validated height without replaying from genesis.

### 1.2 Block Verification

For each block fetched from a recorder, the validator:

1. Deserializes the JSON into a `DataItem` tree.
2. Locates the `SHA256` (code 3) child of `BLOCK` (code 11) — the claimed block hash.
3. Locates the `BLOCK_SIGNED` (code 12) child.
4. Recomputes `SHA-256(BLOCK_SIGNED encoding)` and compares against the claimed hash.
5. Updates the rolled hash: `rolled = SHA-256(rolled || verified_block_hash)`.

A mismatch at step 4 constitutes an **alteration alert** — the recorder has served a block whose claimed hash does not match its content.

### 1.3 Batch Verification

Blocks are fetched in paginated batches (max 1000 per request, matching the recorder's `MAX_BLOCK_RANGE`). Batch verification processes blocks sequentially within each batch, accumulating the rolled hash. Batches can be verified incrementally — a validator resuming after downtime starts from `validated_height + 1` with the stored rolled hash.

---

## 2. Validator Protocol

### 2.1 Polling Model

The validator daemon operates as a periodic poller, not a subscriber. Each poll cycle:

1. For each monitored chain, query `GET /chain/{id}/info` to learn the current recorder height.
2. If recorder height > validated height, fetch blocks in batches via `GET /chain/{id}/blocks?from=N&to=M`.
3. Verify each batch (section 1.2–1.3). On success, update stored state.
4. On verification failure, emit an alteration alert and stop advancing for that chain.

**Poll interval** is configurable (default: 60 seconds). The acceptance criterion is detection of a simulated alteration within one poll interval.

### 2.2 State Transitions

Each monitored chain has a status:

| Status | Meaning |
|--------|---------|
| `ok` | All blocks verified through `validated_height`. No issues. |
| `unreachable` | Recorder did not respond. Previously verified state is preserved. |
| `alert` | Block verification failed. Chain integrity compromised. |

Transitions:

```
ok  ──recorder offline──►  unreachable  ──recorder returns──►  ok
ok  ──hash mismatch──►  alert
unreachable  ──hash mismatch──►  alert
```

Recovery from `alert` requires manual intervention (the validator will not automatically resume past a detected alteration).

### 2.3 Validator HTTP API

The validator exposes its own HTTP endpoints:

| Method | Endpoint | Response |
|--------|----------|----------|
| GET | `/validate` | Array of all monitored chain statuses |
| GET | `/validate/{chain_id}` | Single chain: validated height, rolled hash, status, alert message, latest anchor |

Response format:

```json
{
  "chain_id": "abc123...",
  "validated_height": 4200,
  "rolled_hash": "deadbeef...",
  "last_poll": 1709769600,
  "status": "ok",
  "alert_message": null,
  "latest_anchor": {
    "height": 4000,
    "rolled_hash": "cafebabe...",
    "anchor_ref": "file:/var/ao/anchors.jsonl:4000",
    "anchor_timestamp": 1709769000
  }
}
```

### 2.4 Recorder Endorsement

A recorder can poll one or more validators and include their endorsements in `GET /chain/{id}/info` responses:

```json
{
  "chain_id": "...",
  "block_height": 4205,
  "validators": [
    {
      "url": "https://validator1.example.com",
      "label": "Island Auditors",
      "validated_height": 4200,
      "rolled_hash": "deadbeef...",
      "status": "ok",
      "last_checked": 1709769660
    }
  ]
}
```

The `validators` field is omitted when empty (via `skip_serializing_if`). Clients must treat it as optional. The recorder caches validator responses and refreshes them periodically (default: 60 seconds). The cache is best-effort — a poisoned lock or unreachable validator results in stale or absent data, never a recorder failure.

### 2.5 Alert System

Alerts are dispatched via two channels:

1. **Structured logging** (always active): `tracing::error` for alteration, `tracing::warn` for unreachable, `tracing::info` for recovered.
2. **Webhook** (optional): HTTP POST to a configured URL with JSON payload.

Alert types:

| Type | Trigger | Severity |
|------|---------|----------|
| `Alteration` | Block hash mismatch | Critical — chain integrity compromised |
| `Unreachable` | Recorder connection failed (transition from ok) | Warning |
| `Recovered` | Recorder reachable again (transition from unreachable) | Info |

Webhook delivery is fire-and-forget. Failures are logged but do not block the polling loop.

---

## 3. External Anchoring

### 3.1 Purpose

An external anchor commits a rolled hash to an independent, append-only medium at a specific block height. This creates a **checkpoint** — even if the validator's own database is compromised, the anchor provides an independent reference point.

**Anchoring is not required.** A chain without anchors is fully functional. Anchoring adds defense-in-depth: it raises the cost of undetected tampering from "compromise the validator" to "compromise the validator AND the anchor medium."

### 3.2 Anchor Record

An anchor record contains:

| Field | Type | Description |
|-------|------|-------------|
| `chain_id` | string | Chain being anchored |
| `height` | u64 | Block height at anchor time |
| `rolled_hash` | hex string | Rolled hash at that height |
| `anchor_ref` | string | Backend-specific locator (see 3.3) |
| `anchor_timestamp` | i64 | Unix timestamp when anchor was published |

The `anchor_ref` is an opaque string whose format depends on the backend. Clients can display it but should not parse it.

### 3.3 Anchor Backends

Backends are pluggable. The system ships with one backend; others can be added without protocol changes.

**File backend (shipped):** Appends JSON-lines entries to a local file.

```json
{"chain_id":"abc...","height":4000,"rolled_hash":"deadbeef...","timestamp":1709769000}
```

- `anchor_ref` format: `file:{path}:{height}`
- Verification: read the file, find the entry matching chain_id + height, compare rolled_hash.
- Tamper-evidence: the file's own integrity can be monitored by OS-level tools (inotify, checksums, remote backup). Copying the file to a second machine provides independent verification.

**Future backends (not yet implemented):**

| Backend | anchor_ref format | Trust model |
|---------|-------------------|-------------|
| Bitcoin OP_RETURN | `btc:{txid}:{vout}` | Bitcoin's proof-of-work secures the anchor. Highest cost to forge, but expensive to publish and slow to confirm. |
| Public transparency log | `log:{server}/{entry_id}` | Append-only log server (similar to Certificate Transparency). Lower cost, faster, but trusts the log operator. |
| IPFS | `ipfs:{cid}` | Content-addressed, distributed. Persists as long as at least one node pins the data. |

Adding a backend requires implementing two operations: `publish(chain_id, height, rolled_hash) -> anchor_ref` and `verify(chain_id, height, expected_hash) -> bool`.

### 3.4 Anchor Frequency

Anchoring frequency is a deployment decision, not a protocol parameter. Reasonable strategies:

- **Periodic:** Every N blocks or every T seconds.
- **Threshold:** When validated_height advances past a configured milestone.
- **Manual:** Operator triggers via CLI command.

The current implementation does not include automated anchor scheduling — anchoring is invoked programmatically by the validator operator. This is intentional: anchor backends have varying costs and latencies, and the optimal strategy depends on the deployment.

---

## 4. On-Chain Type Codes

Phase 5 adds type codes for validator attestations and credential references. These extend the registry in [WireFormat.md](WireFormat.md) §3.

### 4.1 Validator Types (Inseparable, |code| 64–68)

These fall in the second inseparable band (|code| 64–95, bit 5 clear: `64 & 0x20 = 0`).

| Code | Name | Size | Description |
|---:|:---|:---|:---|
| 64 | `VALIDATOR_ATTESTATION` | container | Attestation from a validator (children: height, hash, anchor ref, timestamp) |
| 65 | `VALIDATED_HEIGHT` | vbc-value | Block height at which validation was performed |
| 66 | `ROLLED_HASH` | 32 | SHA2-256 rolled hash at the validated height |
| 67 | `ANCHOR_REF` | variable | External anchor reference string (UTF-8) |
| 68 | `ANCHOR_TIMESTAMP` | 8 | Timestamp when anchor was published |

**`VALIDATOR_ATTESTATION` structure:**

```
VALIDATOR_ATTESTATION (64)
├── VALIDATED_HEIGHT (65): block height
├── ROLLED_HASH (66): 32-byte rolled hash
├── ANCHOR_REF (67): backend-specific locator [optional]
└── ANCHOR_TIMESTAMP (68): publication timestamp [optional]
```

Validator attestations are inseparable: removing them from a block would hide evidence of validation, which defeats their purpose. They are not yet embedded in blocks by the current implementation — the type codes are reserved for future use where a recorder includes validator attestations in block metadata.

### 4.2 Credential Types (Separable, |code| 38–39)

These fall in the first separable band (|code| 32–63, bit 5 set: `38 & 0x20 = 32`).

| Code | Name | Size | Description |
|---:|:---|:---|:---|
| 38 | `CREDENTIAL_REF` | container | Reference to an external verifiable credential |
| 39 | `CREDENTIAL_URL` | variable | URL where the credential document can be fetched (UTF-8) |

These are defined in detail in section 5.

---

## 5. Vendor Credentials

### 5.1 Purpose

Vendor credentials allow chain operators to attach references to external trust documents — business licenses, food safety certificates, professional certifications — as separable items in their chain data (typically in a `VENDOR_PROFILE`). Consumers can see that credentials exist and optionally verify them.

**Credentials are not required.** A chain without credentials is valid. Credentials are informational — they help consumers make trust decisions, but the protocol does not enforce their content or validity.

### 5.2 Credential Reference Structure

```
CREDENTIAL_REF (38)
├── CREDENTIAL_URL (39): "https://example.com/certs/food-safety-123.json"
└── SHA256 (3): hash of the credential document at the URL
```

The `SHA256` child is the SHA2-256 hash of the credential document's byte content at the time the credential reference was created. This binds the URL to specific content — if the document at the URL changes, the hash will no longer match.

**Separability:** Both `CREDENTIAL_REF` (38) and `CREDENTIAL_URL` (39) are separable (`|code| & 0x20 != 0`). This means credential references can be stripped from blocks to reduce storage without invalidating signatures. The hash of the stripped credential reference remains in the block, proving that a credential was present at signing time.

### 5.3 Credential Verification

Verification is a client-side operation, not enforced by the recorder or validator:

1. Fetch the document at `CREDENTIAL_URL`.
2. Compute `SHA-256(document_bytes)`.
3. Compare against the `SHA256` child in `CREDENTIAL_REF`.
4. If match: the document at this URL is the same document the vendor referenced when creating the chain.
5. If mismatch: the document has changed since the reference was created. The credential may have been revoked, updated, or the URL reassigned.

The PWA displays a hash-match indicator (green check / red warning) next to each credential. The indicator reflects only hash verification — it does not validate the credential's content, issuer authority, or expiration.

### 5.4 W3C Verifiable Credentials Compatibility

The credential reference system is designed to be compatible with, but not dependent on, the [W3C Verifiable Credentials](https://www.w3.org/TR/vc-data-model/) data model.

**How it maps:**

| AO Concept | W3C VC Concept |
|------------|----------------|
| `CREDENTIAL_URL` | `id` field of a Verifiable Credential |
| `SHA256` hash in `CREDENTIAL_REF` | Content integrity check (similar to VC's `digestSRI`) |
| Chain operator (vendor) | Credential subject |
| External issuer (at URL) | Credential issuer |

**What AO does:**
- Stores a URL + content hash on-chain as a separable item.
- Allows clients to fetch and verify the hash match.
- Makes no assumptions about the document format at the URL.

**What AO does NOT do:**
- Does not parse, validate, or enforce W3C VC JSON-LD structure.
- Does not verify issuer signatures within the VC document.
- Does not check credential expiration or revocation lists.
- Does not implement DID resolution.

**Rationale:** Full W3C VC verification requires a JSON-LD processor, DID resolver, and signature suite implementation — significant complexity for marginal benefit in the target use case (small island businesses). The hash-match model provides the 80% value (did the vendor claim this credential? is the document unchanged?) without the 80% complexity. A future client or middleware can add full VC verification on top of the same on-chain data.

### 5.5 W3C DID Compatibility

Assign Onward does not implement the [W3C DID](https://www.w3.org/TR/did-core/) specification. However, the system's identity model is structurally compatible:

| AO Concept | DID Concept |
|------------|-------------|
| Ed25519 public key | Verification method |
| Chain ID (genesis hash) | DID subject context |
| `VENDOR_PROFILE` separable item | DID document metadata |

A future DID method (`did:ao:{chain_id}:{pubkey_hex}`) could be defined that resolves to the current state of a vendor's profile and credentials on a specific chain. This is deferred — it requires a DID method specification and resolver implementation that are out of scope for Phase 5.

**Key difference:** AO uses single-use keys for share transfers. A DID typically represents a persistent identity. These are reconcilable — the vendor's *identity key* (used to sign the `VENDOR_PROFILE`, not to receive shares) can serve as the DID verification method, while share-receiving keys remain ephemeral.

---

## 6. Trust Indicator Display

### 6.1 Consumer-Facing Trust Signals

The PWA displays trust information in the chain detail view:

**Validator endorsements** (from `GET /chain/{id}/info` → `validators` array):
- Green dot + "verified": validator status is `ok` and validated_height is within 1 block of chain height.
- Amber dot + "N blocks behind": validator status is `ok` but validator is lagging.
- Red dot + status text: validator reports `alert`, `unreachable`, or other non-ok status.

**Credential references** (from vendor profile in chain data):
- Green check: document at URL matches the on-chain hash.
- Red warning: hash mismatch or URL unreachable.
- Grey dash: no credentials attached.

### 6.2 Trust Model Interpretation

The following guidance should be surfaced in user-facing documentation, not enforced by protocol:

- **Multiple independent validators** increase confidence. A chain monitored by three independent validators, each showing green, is more trustworthy than one monitored by a single validator run by the chain operator.
- **Anchor references** provide point-in-time evidence. A validator with file-based anchors provides evidence that the chain existed in a specific state at a specific time. Bitcoin-anchored validators provide stronger evidence (harder to forge the anchor medium).
- **Credentials are claims, not guarantees.** A food safety certificate hash-match means the vendor referenced a specific document. It does not mean the document is genuine, current, or that the issuing authority is trustworthy. Users should apply the same judgment they would to a physical certificate posted on a wall.

---

## 7. Scope and Non-Requirements

### 7.1 In Scope for Phase 5

- Validator daemon with polling, verification, alerting, and HTTP API.
- File-based external anchoring.
- Recorder validator endorsement caching.
- Credential reference type codes and on-chain structure.
- PWA trust indicator display.

### 7.2 Explicitly Deferred

| Feature | Reason | When |
|---------|--------|------|
| Bitcoin OP_RETURN anchoring | Requires Bitcoin RPC integration and transaction fees | Post-Phase 5, as demand warrants |
| Full W3C VC verification | JSON-LD + DID resolver complexity, marginal benefit for target users | If/when a W3C VC ecosystem emerges in the target communities |
| DID method specification | Requires formal DID method registration and resolver implementation | Post-Phase 5, if cross-system identity interop is needed |
| Credential revocation checking | Requires issuer-specific revocation list or status endpoint | Future: when credential issuers provide standardized revocation APIs |
| Automated anchor scheduling | Deployment-specific; operator controls frequency | Configuration enhancement, low priority |
| Validator-to-validator cross-checking | Validators comparing notes to detect split-view attacks | Phase 6+ (requires multi-recorder model) |
