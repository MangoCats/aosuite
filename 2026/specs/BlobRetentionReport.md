# Blob Storage Retention & On-Chain Linking: Gap Analysis and Roadmap Proposal

**Date:** 2026-03-08
**Status:** Draft for review — edit with comments, then distill into ROADMAP tasks.

---

## 1. Current State

### What works today

Blob storage is content-addressed and functional end-to-end:

- **Upload:** PWA `AttachmentPicker` compresses images (WebP/JPEG, max 2048px),
  sends `[MIME NUL content]` to `POST /chain/{id}/blob`, gets back `{"hash": "<sha256>"}`.
- **Storage:** `BlobStore` in `ao-recorder/src/blob.rs` writes to `data_dir/blobs/{hash}`.
  Idempotent (same content = same hash, no duplication). Atomic writes via temp+rename.
- **Retrieval:** `GET /chain/{id}/blob/{hash}` serves with security headers
  (`nosniff`, restrictive CSP, `immutable` cache-control).
- **Guardrails:** Per-chain quota (default 100 MB), per-blob limit (default 5 MB),
  MIME allowlist (`image/*`, `application/pdf`), cross-chain isolation.
- **Tests:** 16 Rust tests + 6 TypeScript tests covering upload, retrieval,
  quota enforcement, MIME validation, cross-chain isolation, temp cleanup.

### Two critical gaps

**Gap A — Blobs are not linked on-chain.**
`ConsumerView.tsx:283-297` uploads blobs alongside the transfer but does NOT embed
blob hashes as `DATA_BLOB` (type 33) separable children in the signed assignment
DataItem. The blob exists on the recorder's filesystem with no on-chain proof that
it was part of a specific transaction. If the recorder prunes or loses the blob,
there is no hash on-chain to even prove "a blob once existed here."

**Gap B — No retention policy or automated pruning.**
`SysopGuide.md:231-252` states: "Blob pruning is not yet implemented." Operators
must manually `rm` files. There is no promise to users about how long their receipt
photos will be available. No automated lifecycle management.

---

## 2. Proposed Design: Storage Longevity Promise

### 2.1 Concept

The chain's genesis block (or a subsequent AMENDMENT block) declares a **Blob
Retention Policy** — a machine-readable promise from the recorder operator about
what will be stored, for how long, and under what capacity constraints. This policy:

- Sets expectations for consumers and vendors ("your receipt photos are kept 7 years")
- Gives the recorder operator a contractual basis for pruning ("videos over 100 MB
  are kept 7 days — after that, the hash remains on-chain but the blob returns 404")
- Is auditable: a validator or any client can verify that blobs promised to be
  available actually are

### 2.2 Policy Structure

A retention policy is a container DataItem (new type code needed, e.g.
`BLOB_POLICY` = 40) with rule children. Each rule specifies a MIME pattern, size
threshold, and retention duration. Rules are evaluated in order; first match wins.

```
BLOB_POLICY (container, type 40)
  ├─ BLOB_RULE (container, type 41)
  │   ├─ MIME_PATTERN: "image/*"         (UTF-8, glob-style)
  │   ├─ MAX_BYTES: 5242880             (5 MB — blobs above this are rejected)
  │   ├─ RETENTION_SECONDS: 220752000   (7 years)
  │   └─ PRIORITY: 1
  ├─ BLOB_RULE
  │   ├─ MIME_PATTERN: "application/pdf"
  │   ├─ MAX_BYTES: 10485760            (10 MB)
  │   ├─ RETENTION_SECONDS: 220752000   (7 years)
  │   └─ PRIORITY: 2
  ├─ BLOB_RULE
  │   ├─ MIME_PATTERN: "*/*"            (catch-all)
  │   ├─ MAX_BYTES: 104857600           (100 MB)
  │   ├─ RETENTION_SECONDS: 604800      (7 days)
  │   └─ PRIORITY: 99
  ├─ CAPACITY_LIMIT: 536870912000       (500 GB total guaranteed capacity)
  ├─ THROTTLE_THRESHOLD: 107374182400   (100 GB — new large blobs rejected below this)
  └─ THROTTLE_MAX_BYTES: 104857600      (100 MB — only blobs above this are throttled)
```

**Semantics:**
- `RETENTION_SECONDS` is a minimum guarantee from upload timestamp. The operator MAY
  keep blobs longer but MUST NOT prune before expiry.
- `CAPACITY_LIMIT` is total blob storage the operator commits to for this chain.
  When used capacity exceeds `CAPACITY_LIMIT`, the operator is not obligated to
  accept new uploads (413 response).
- `THROTTLE_THRESHOLD` + `THROTTLE_MAX_BYTES`: When remaining capacity drops below
  the threshold, blobs larger than `THROTTLE_MAX_BYTES` are rejected. Small blobs
  (receipts, photos) continue to be accepted. This prevents a few large videos from
  crowding out years of receipt images.
- Rules are matched by MIME pattern and size. A blob matching `image/jpeg` at 2 MB
  hits rule 1 (7-year retention). A `video/mp4` at 50 MB hits the catch-all (7-day
  retention).

### 2.3 Where the policy lives

**Option A — Genesis child:** The `BLOB_POLICY` container is a child of the GENESIS
DataItem. Immutable for the life of the chain. Simple, auditable, but inflexible.

**Option B — Amendment block:** A new block type (or assignment with a special
container) that updates the policy. The latest BLOB_POLICY on-chain is the active
one. Allows operators to expand storage commitments over time. Requires chain
validation to enforce that amendments can only relax guarantees (extend retention,
increase capacity), never retroactively shorten them for already-uploaded blobs.

**Recommendation:** Start with **Option A** (genesis child) for simplicity. Add
amendment support later if operators need flexibility. The genesis approach means the
policy is signed by the chain creator and visible to everyone from block 0. An
operator who needs different retention characteristics creates a new chain.

### 2.4 On-chain blob references (closing Gap A)

When a consumer submits a transfer with attachments:

1. Upload each blob to recorder, receive `{hash}` per blob.
2. Build the assignment DataItem with `DATA_BLOB` (type 33) separable children.
   Each `DATA_BLOB` child contains the full `[MIME NUL content]` payload.
3. During signing, `substituteSeparable()` replaces each `DATA_BLOB` with its
   SHA2-256 hash — exactly as the spec already defines.
4. The signed assignment goes on-chain with the blob hashes embedded.
5. Anyone can later verify: fetch blob from recorder, hash it, compare to on-chain hash.

This is how the system was *designed* to work (type 33 is already separable, the
substitution code exists in both Rust and TypeScript). The TODO at
`ConsumerView.tsx:283` is the only missing piece on the PWA side; `buildAssignment`
needs to accept `DATA_BLOB` children.

**Wire format impact:** None. DATA_BLOB is already a defined type code (33), already
separable, already handled by `substituteSeparable()`. The assignment just gets
additional children before signing.

**Chain size impact:** Each blob reference adds 34 bytes on-chain (2-byte type code +
32-byte SHA2-256 hash after separable substitution). A transfer with 5 photos adds
170 bytes — negligible.

---

## 3. Automated Pruning

### 3.1 Pruning logic

A background task in `ao-recorder` (e.g. runs hourly or daily):

```
for each blob file in data_dir/blobs/:
    hash = filename
    chain_id = blob_owners[hash]
    upload_time = file mtime (or tracked in a metadata sidecar/SQLite table)
    policy = active BLOB_POLICY for chain_id
    rule = first matching rule (by MIME, size)
    if now - upload_time > rule.retention_seconds:
        delete blob file
        update chain_usage accounting
        log: "Pruned blob {hash} from chain {chain_id}, age {days}d, policy rule {priority}"
```

### 3.2 Metadata tracking

Current `BlobStore` tracks `chain_usage` and `blob_owners` in memory (rebuilt from
filesystem on startup). Pruning needs additional metadata per blob:

- **Upload timestamp** (not just file mtime, which can be unreliable after backup/restore)
- **MIME type** (needed for rule matching; currently stored inline in the blob file)
- **Size** (filesystem stat, already available)

Options:
- **SQLite table** (`blob_meta`): `hash TEXT PK, chain_id TEXT, mime TEXT, size INT,
  uploaded_at INT`. Authoritative. Survives backup/restore. Adds ~100 bytes per blob
  to the database.
- **Sidecar JSON**: `{hash}.meta` files alongside blobs. Simple but doubles file count.

**Recommendation:** SQLite table. The recorder already uses SQLite for chain data.
One more table is natural.

### 3.3 Retrieval after pruning

When a blob has been pruned:
- `GET /chain/{id}/blob/{hash}` returns **410 Gone** (not 404), with a JSON body:
  `{"error": "pruned", "policy_rule": "image/* ≤5MB: 7 years", "pruned_at": "..."}`
- The on-chain hash remains. Anyone with a copy of the original blob can verify it
  independently.
- The PWA `BlobViewer` should handle 410 gracefully: "This attachment was stored for
  7 years per chain policy and has since been pruned. The on-chain hash
  `abc123...` can be used to verify any copy."

---

## 4. Auditability

### 4.1 Validator blob audit

A validator (or any interested party) can audit blob retention compliance:

1. Walk the chain's blocks, collect all `DATA_BLOB` hashes from assignments.
2. For each hash, `GET /chain/{id}/blob/{hash}`.
3. Compute `SHA2-256(response_body)` and compare to on-chain hash (integrity check).
4. Check that the blob's age does not exceed its retention rule (freshness check).
5. Report: blobs that should be available but return 404/410 = policy violation.

**Data transfer cost:** Downloading every blob to audit is expensive. Two mitigations:

- **HEAD requests:** Add `HEAD /chain/{id}/blob/{hash}` that returns `Content-Length`
  and a custom header `X-AO-Uploaded-At` without transferring the body. Sufficient
  for existence/age checks. The auditor only downloads (and re-hashes) a random
  sample for integrity verification.
- **Manifest endpoint:** `GET /chain/{id}/blobs/manifest` returns a JSON list of
  `{hash, mime, size, uploaded_at, retention_expires_at}` for all blobs on the chain.
  Auditors compare this against on-chain blob hashes to find discrepancies.

### 4.2 Consumer-side verification

A consumer who saved a receipt photo locally can verify it against the chain at any
time:
1. Hash their local copy with SHA2-256.
2. Look up the assignment on-chain, find the matching `DATA_BLOB` hash.
3. If hashes match, the photo is authentic and was part of the signed transaction.

This works even after the recorder has pruned the blob — the on-chain hash is
permanent.

---

## 5. Edge Cases and Design Decisions

### 5.1 What if the operator under-provisions?

If `CAPACITY_LIMIT` is set to 500 GB but the operator's disk is only 200 GB, the
promise is unenforceable at the infrastructure level. This is an operational risk,
not a protocol risk. Validators can detect it (blob requests fail before the
retention period expires). Reputation consequences apply.

### 5.2 Retroactive policy changes

If amendment blocks are allowed later: a policy change MUST NOT shorten retention
for already-uploaded blobs. Each blob's retention is locked at upload time based on
the then-active policy. New uploads follow the new policy. This prevents bait-and-switch
("7 years" at upload, quietly changed to "30 days" later).

### 5.3 Blob deduplication across chains

Current design: blobs are owned by one chain. If two chains upload identical content,
it's stored once on disk but tracked for both chains. Pruning must check all owners
before deleting the file. Current `blob_owners` is a 1:1 map — this needs to become
1:N if cross-chain dedup is desired. **Simpler alternative:** keep 1:1 ownership,
accept minor disk waste. Revisit if storage pressure demands it.

### 5.4 Encrypted blobs

Some use cases (medical records, financial documents) may want blobs encrypted
client-side before upload. The recorder stores opaque bytes; the MIME type would be
`application/octet-stream` or a custom `application/x-ao-encrypted`. The on-chain
hash still works for verification (hash of ciphertext). Retention rules apply to
the declared MIME type. This needs no protocol changes — just a PWA-side
encrypt-before-upload option. Out of scope for this phase but compatible with
the design.

### 5.5 Blob fees: pre-substitution byte count

The existing recording fee formula is:

```
fee = ceil(assignment_bytes × FEE_RATE_num × SHARES_OUT / FEE_RATE_den)
```

Today, `assignment_bytes` is the serialized size of the signed assignment DataItem
— after separable substitution (blobs replaced with 32-byte hashes). This makes
the fee independent of blob size, meaning a 100 MB photo receipt costs the same to
record as a 500-byte text-only transfer. That's unrealistic: the recorder bears
real-world costs (bandwidth, storage, power, maintenance) proportional to what it
actually stores.

**Decision:** Compute fees on the **pre-substitution** assignment size. The full
DATA_BLOB children (MIME + content) are included in the byte count before
`substituteSeparable()` replaces them with hashes. The fee is then computed against
this larger size. The chain records the post-substitution assignment (compact), but
the fee reflects the true cost of what the recorder accepted and must store.

**How this works in practice:**

1. Consumer builds assignment with DATA_BLOB children (full payloads).
2. `assignment_bytes` = `serialize(assignment_with_blobs).len()`.
3. Fee computed: `ceil(assignment_bytes × FEE_RATE / SHARES_OUT)`.
4. `substituteSeparable()` replaces blobs with hashes.
5. Signed, post-substitution assignment submitted to recorder alongside blob uploads.
6. On-chain: compact assignment (hashes only). Fee pool: compensated for full size.

**Example:** A transfer with a 2 MB receipt photo:
- Pre-substitution: ~2,000,500 bytes (photo dominates)
- Post-substitution: ~534 bytes (hash + metadata)
- At a fee rate of 1/10,000, the fee is ~200 shares vs. ~0.05 shares without blobs.

**Why this is fair:**
- The chain creator set the `FEE_RATE` knowing their cost structure. If they accept
  blobs (via `BLOB_POLICY`), the fee rate already reflects storage costs.
- Recorders who don't accept blobs (no `BLOB_POLICY`) never see large assignments.
- Recorders who accept blobs are compensated proportionally — a 100 MB video costs
  ~200,000× more in fees than a 500-byte transfer, roughly matching the difference
  in real-world storage/bandwidth cost.
- No new genesis parameters needed. The existing `FEE_RATE` mechanism scales
  naturally.

**Implementation notes:**
- `buildAssignment()` (both Rust and TypeScript) must compute fee against the
  pre-substitution byte count, then perform substitution, then sign.
- The recorder validates: re-serialize the post-substitution assignment, add back
  known blob sizes (from the uploaded blobs' metadata), verify the fee was computed
  against the correct total. This prevents a consumer from claiming a small
  pre-substitution size to underpay fees.
- The iterative fee convergence loop (currently 3 rounds) still works — the blob
  sizes are constant across iterations, only the fee/change amounts shift.

**Recorder-side validation of blob fees:**

When the recorder receives a block submission with blob references:
1. For each DATA_BLOB hash in the post-substitution assignment, look up the blob's
   size from `blob_meta` (it was uploaded moments before).
2. Compute `expected_pre_sub_bytes = post_sub_bytes - (N × 33) + sum(blob_sizes + overhead)`.
   (Each substitution replaced a variable-size blob with a 33-byte hash item:
   1 byte signed VBC type code for SHA256 + 32 bytes fixed hash data.)
3. Verify: `actual_fee >= ceil(expected_pre_sub_bytes × FEE_RATE / SHARES_OUT)`.
4. Reject if underpaid.

This keeps the recorder honest too — it can't inflate blob sizes to extract higher
fees, because the consumer computed the fee client-side from the actual blob content.

### 5.6 Multi-recorder blob replication

In a future multi-recorder topology, blob retention promises become harder to
enforce. If Recorder A promises 7 years but crashes after 2, does Recorder B
(which may have replicated the blob) inherit the obligation? This is a
multi-recorder governance question, not a blob-specific one. For now, single-recorder
chains own their promises entirely.

---

## 6. Implementation Tasks (Proposed Roadmap Items)

### Phase N8+ (Blob Retention) — estimated as a focused extension

**N8a: On-chain blob linking + pre-substitution fee calculation** (closes Gap A)
- Extend `buildAssignment()` in `ao-pwa/src/core/assignment.ts` to accept
  `DATA_BLOB` children (the MIME+content payloads).
- **Fee calculation on pre-substitution size:** Compute `assignment_bytes` from the
  full assignment including blob content, then apply `ceil(bytes × FEE_RATE_num ×
  SHARES_OUT / FEE_RATE_den)`. Only then run `substituteSeparable()` to replace
  blobs with hashes before signing. No new genesis parameters needed — the existing
  `FEE_RATE` scales naturally with blob size (see §5.5).
- Upload blobs first, then embed the full payloads as separable children.
  Hashes appear on-chain; full blobs live on the recorder.
- Update `ConsumerView.tsx` to wire attachments into assignment building (resolve
  the TODO at line 283).
- **Recorder-side fee validation:** On block submission, recorder reconstructs the
  expected pre-substitution byte count from post-substitution assignment + known
  blob sizes from `blob_meta`. Rejects assignments where the fee is too low for
  the actual blob content stored (see §5.5 for validation algorithm).
- Rust side: `ao-chain` assignment validation must accept `DATA_BLOB` children
  and validate fees against pre-substitution size. Update `ao-recorder` block
  submission handler to perform the blob-aware fee check.
- Add round-trip tests: build assignment with blob children, verify fee reflects
  full blob size, sign, verify hash matches uploaded blob, verify recorder accepts.

**N8b: BLOB_POLICY in genesis**
- Allocate type codes: `BLOB_POLICY` (40), `BLOB_RULE` (41), plus child type codes
  for `MIME_PATTERN`, `MAX_BYTES`, `RETENTION_SECONDS`, `CAPACITY_LIMIT`,
  `THROTTLE_THRESHOLD`, `THROTTLE_MAX_BYTES`. (Or reuse existing generic types
  like NOTE for the MIME pattern string, and sized integers for numeric fields —
  needs design decision on specificity vs. simplicity.)
- Extend genesis builder (`ao-chain/src/genesis.rs`, `ao-cli`, PWA genesis creator)
  to accept optional `BLOB_POLICY` container.
- Extend `ao-recorder` `BlobStore` to read the active chain's `BLOB_POLICY` and
  enforce it on upload (reject blobs that violate MIME/size rules or exceed capacity).
- Add `GET /chain/{id}/blob-policy` endpoint returning the policy as JSON.
- PWA: display the chain's blob policy in `ChainDetail.tsx` so users know their
  retention guarantees.

**N8c: Blob metadata table + automated pruning**
- Add `blob_meta` SQLite table: `(hash, chain_id, mime, size, uploaded_at)`.
- Populate on upload; backfill existing blobs from filesystem scan + file mtime.
- Background task (configurable interval, default daily): iterate `blob_meta`,
  evaluate retention rules, delete expired blobs, return **410 Gone** for
  pruned hashes.
- Logging: structured log entry per prune event (hash, chain, age, rule).
- Sysop command: `ao-recorder prune --dry-run` for manual preview.

**N8d: Audit endpoints**
- `HEAD /chain/{id}/blob/{hash}` — returns `Content-Length`, `Content-Type`,
  `X-AO-Uploaded-At`, `X-AO-Retention-Expires` headers. No body transfer.
- `GET /chain/{id}/blobs/manifest` — JSON array of all blob metadata for the chain.
  Paginated if large.
- PWA `BlobViewer`: handle 410 status gracefully with explanatory message.

**N8e: Validator blob audit (stretch)**
- `ao-validator` optional mode: walk chain blocks, extract DATA_BLOB hashes,
  HEAD-check each against recorder, report compliance.
- Alert on premature pruning (blob missing before retention expiry).

### Dependency order

```
N8a (on-chain linking)
  → N8b (policy in genesis) — needs type codes from N8a context
    → N8c (pruning) — needs policy to know what to prune
      → N8d (audit endpoints) — needs metadata table from N8c
        → N8e (validator audit) — needs audit endpoints from N8d
```

N8a is independently valuable and should ship first. The rest form a chain.

---

## 7. Type Code Allocation

Current separable range: 32–39 allocated, 40–63 available.

| Code | Name | Category | Purpose |
|------|------|----------|---------|
| 40 | `BLOB_POLICY` | Container | Retention policy (in genesis or amendment) |
| 41 | `BLOB_RULE` | Container | Single retention rule within policy |
| 42 | `MIME_PATTERN` | Variable | Glob pattern for MIME matching |
| 43 | `RETENTION_SECS` | Fixed-8 | Minimum retention in seconds (i64) |
| 44 | `CAPACITY_LIMIT` | BigInt | Total guaranteed storage in bytes |
| 45 | `THROTTLE_THRESHOLD` | BigInt | Remaining capacity at which throttling begins |

Note: `MAX_BYTES` can reuse an existing bigint type or get code 46. `PRIORITY` can
be a VBC value child. Exact allocation should be finalized during implementation
against the full type code registry in `byteCodeDefinitions.json`.

**Alternative:** Keep it simpler — encode the entire policy as a single `NOTE`
(type 32) child of genesis with a structured text format (like CooperativeMetadata's
`key:value` lines). Less formal, easier to implement, harder to validate
programmatically. Not recommended if we want automated enforcement, but viable as
an interim step.

---

## 8. Open Questions for Review

1. **Genesis-only or amendable?** Starting with genesis-only is simpler, but an
   operator who wants to upgrade from a 100 GB to 1 TB commitment can't do so
   without creating a new chain. Is that acceptable for the first implementation?

   Yes.  A new chain is an acceptable form of amendment.

2. **Type code block allocation:** Should BLOB_POLICY and friends live in the
   separable range (40+) or the non-separable range? The policy itself doesn't need
   to be separable (it's metadata about the chain, not transaction content). But
   putting it in the separable range keeps the 40–63 block as "chain metadata."

   BLOB_POLICY is a policy / commitment, I don't believe that should be separable.

3. **Default policy for chains without BLOB_POLICY:** If a genesis block has no
   policy, should the recorder apply a built-in default (e.g. "best-effort, no
   guarantee") or refuse blob uploads entirely? Recommendation: best-effort default
   with a clear "no retention guarantee" indicator in the PWA.

   Concur, best-effort default.

4. **Cross-chain blob dedup:** Worth the complexity? Current 1:1 ownership is simple.
   A busy recorder with 1000 chains might see some duplicate images across chains,
   but content-addressed storage already deduplicates on disk. The issue is only
   around pruning (can't delete until all owners' retention has expired).

   Not worth the complexity.

5. **Blob fee:** Should uploading a blob cost shares (a recording fee proportional
   to size)? This would create economic pressure against storage abuse. But it
   complicates the upload flow (the consumer needs enough shares to cover the blob
   fee). Probably a later consideration.

   Go ahead and consider at this time, ideally the blob cost should be representative
   of real-world costs.  So, bandwidth, storage, power, maintenance time, all these
   things cost more to store a 100MB transaction with blob than to store a 500 byte
   transaction.  In some respect, the size of the blob is in the hands of the recorder
   so the recorder should bear the expense of their recording proportional to the
   expense of recording cheaper transactions for more efficiently operating recorders.

---

## 9. Summary

The blob storage infrastructure is solid but disconnected from the chain's trust
model. Three changes close the gap:

1. **Link blobs on-chain** (DATA_BLOB separable children in assignments) — the
   protocol already supports this; it's a PWA wiring task.
2. **Compute fees on pre-substitution size** — the existing `FEE_RATE` mechanism
   scales naturally when `assignment_bytes` includes full blob content before
   separable substitution. No new genesis parameters. A 2 MB receipt photo costs
   proportionally more to record than a 500-byte text transfer, reflecting the
   recorder's real-world storage and bandwidth costs.
3. **Declare retention policy in genesis** (BLOB_POLICY container) — makes the
   recorder's storage commitment transparent, auditable, and enforceable.

With these in place, a consumer can be told: "Your receipt photo is stored for 7
years. Here's the on-chain hash. The recording fee covered the cost of that storage.
Any copy of the photo can be verified against this hash forever, even after the
recorder prunes it." That's the promise a real business needs.
