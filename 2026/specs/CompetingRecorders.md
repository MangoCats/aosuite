# TⒶ³ Competing Recorders — Phase 0 Specification

**Status:** REVIEW COMPLETE — all 21 independent review findings addressed; ready for implementation
**Depends on:** N33 (recorder relay federation)
**Spec pattern:** Standalone document (per ValidationAndTrust.md, AtomicExchange.md)

---

## 1. Design Philosophy

TⒶ³ for Assign Onward is a **recorder marketplace**, not multi-node consensus.

Competition happens through **migration** (switching recorders) and **market forces** (fee competition, uptime reputation), not simultaneous multi-writer BFT. A curry goat vendor doesn't need Byzantine fault tolerance — they need the ability to fire a bad recorder and hire a better one.

- One active recorder per chain at any time (preserves TⒶ¹ simplicity)
- Recorder switches are explicit, owner-authorized, on-chain events
- Market pressure (not consensus algorithms) keeps recorders honest
- Large-market feature (million+ population); design now, deploy when scale demands

## 2. Roles and Identity

### 2.1 Chain Owner

The holder of any **currently valid owner key**, with the genesis issuer key as the root of the authority chain (see §6). Ownership is a key custody question, not a protocol question. The owner:

- Authorizes which recorder serves their chain
- Sets the fee ceiling at genesis
- Signs `RECORDER_CHANGE` blocks to switch recorders

### 2.2 Recorder

An independent service operator running an ao-recorder instance. Identified by Ed25519 key. A recorder:

- Competes for chains by offering lower fees, better uptime, geographic proximity
- Publishes a `RECORDER_IDENTITY` (signed self-description)
- May serve many chains simultaneously
- Earns compensation per block (share reward, indirect benefit from deflationary burn, or off-chain payment — see §4.3)

## 3. Competition Model: Owner-Selected

**Model A (owner-selected)** is the only supported model. Personal opinions should outweigh complex algorithms.

- Chain owner chooses their recorder
- No voting, staking, or algorithmic selection
- Switching cost is intentionally low — this is the structural defense against bad behavior
- Reputation signals (uptime history, validator reports, peer reviews) inform owner choice but are not enforced by protocol

## 4. Fee Economics

### 4.1 Fee Ceiling

The **genesis fee rate** is a permanent ceiling for the chain's deflationary burn rate. Recorders may charge at or below this ceiling. The share reward rate (§4.3) is separately negotiated and not subject to this ceiling.

### 4.2 Per-Block Fee Transparency

Every recorded block includes the recorder's actual fee rate. This is verifiable by validators and visible to chain participants.

### 4.3 Recorder Compensation

Two independent on-chain mechanisms, which may be used together, separately, or not at all:

- **Deflationary burn** (existing `FEE_RATE` from genesis): shares retired per transaction. This is supply management — it reduces `SHARES_OUT` over time. The burn rate could be zero if desired. The recorder is not directly compensated by burns; it benefits indirectly through appreciation of its own holdings (if any).

- **Share reward**: a fraction of each transaction's shares is directed to the recorder's key as direct on-chain compensation. The reward is deducted from the giver(s) alongside the burn fee, extending the balance equation to: `sum(giver_amounts) = sum(receiver_amounts) + fee_shares_burned + reward_shares_to_recorder`. The reward rate is agreed by consensus of owner and recorder, recorded in the genesis block as the initial rate. Rate adjustments are signed by **both** owner and recorder and recorded on-chain via `REWARD_RATE_CHANGE` blocks, making the history auditable.

A chain may also compensate its recorder off-chain (fiat payment, service trade, etc.) — this is outside the protocol's concern.

#### Share Reward Defaults

Default reward rates will be tuned through simulation — the appropriate value depends heavily on what each share represents and how that value changes over time. The initial default should be a reasonable guess that covers recorder operating costs.

### 4.4 Predatory Pricing Defense

Reputation signal carries most weight. Low switching cost is the structural defense — if a recorder undercuts to gain market share then raises prices, the owner simply switches.

## 5. Recorder Switch (Same Owner)

The primary TⒶ³ use case. The owner keeps their chain but moves it to a different recorder.

### 5.1 Process

1. Owner selects new recorder (off-chain negotiation)
2. N33 sync begins — new recorder starts replicating chain state (block replay + blob fetch). Sync may continue through steps 3–4.
3. Owner submits `RECORDER_CHANGE_PENDING` to the **outgoing recorder**, which records it. Enters queued state:
   - Active CAA escrows continue to completion or expiration (including timeout expiration — see §5.1 note)
   - New CAA escrows are blocked from starting
   - Regular (non-CAA) transactions continue normally
4. The outgoing recorder monitors escrow state. Once all active CAA escrows have resolved (completed or expired), the recorder auto-constructs and records the `RECORDER_CHANGE` block (analogous to escrow sweep in AtomicExchange.md §5)
5. **Hard cutover** — momentary downtime acceptable ("momentary downtime on Bob's Curry Goat is a non-issue")
6. New recorder begins serving the chain; new CAA escrows can now proceed

**§5.1 note:** An expired (timed-out) CAA escrow counts as resolved for the purpose of unblocking the recorder switch. The PENDING state waits only for escrows that are still actively in progress.

**Note:** The primary path (steps 3–4) requires cooperation from the outgoing recorder. If the outgoing recorder refuses, chain migration (§5.4) is the fallback.

### 5.2 Block Signing Authority

Chain of authority: the `RECORDER_CHANGE` block signed by a valid owner key names the new recorder key. The outgoing recorder co-signs ideally, but one signature (owner's) is sufficient in practice.

### 5.3 Client Discovery

- Recorder URL included in `RECORDER_CHANGE` block as a hint
- Old recorder redirects clients if cooperative; `RECORDER_CHANGE` on-chain confirms redirect is legitimate
- Some customers will need to re-scan QR code (acceptable real-world friction)

### 5.4 Uncooperative Recorder

If the recorder being replaced refuses to record the `RECORDER_CHANGE_PENDING` or `RECORDER_CHANGE` blocks, **chain migration (§7) is the fallback**. The owner creates a new chain with a valid `CHAIN_MIGRATION` block signed by their owner key(s), revoking the old chain and recorder.

The uncooperative recorder may continue serving the old chain at its existing address, but this is effectively a hijacked chain — shares "spent" on the uncooperatively-continuing chain are still valid on the migrated chain (same self-defeating hijacking logic as §7.3).

**Reputation symmetry:** A "rogue owner" could also migrate without notifying the recorder, besmirching the recorder's reputation. A diligent recorder, upon discovering the migration, will independently record the `CHAIN_MIGRATION` on the old chain — which in turn dents the owner's reputation for having moved without properly recording a forwarding address. Both parties have incentive to cooperate because both get hurt by not doing so.

### 5.5 Recorder URL Changes

URLs change (especially raw IP addresses, which some recorders will inevitably use). A URL change without a recorder change requires a `RECORDER_URL_CHANGE` block, signed by **both** the recorder and the chain owner. This:

- Prevents a compromised recorder from silently redirecting traffic
- Makes the move public and auditable on-chain
- If the old address is still reachable, it can redirect clients; the on-chain record assures customers the redirect is legitimate
- More commonly, the vendor prints new QR codes and the on-chain record assures customers they aren't being hijacked to a malicious recorder

## 6. Owner Key Rotation

Proactive key management — rotate keys before losing them, rather than dealing with migration trust tiers after the fact.

### 6.1 `OWNER_KEY_ROTATION` Block

Signed by the current owner key, declares:

- **New owner pubkey** — becomes immediately valid for owner operations
- **Old key expiration** (optional) — timestamp after which the old key is no longer valid. If omitted, the old key remains valid indefinitely alongside the new key.

Both old and new keys are valid for owner operations (`RECORDER_CHANGE`, `RECORDER_URL_CHANGE`, further rotations, etc.) until the old key expires or is revoked.

### 6.2 Multiple Active Keys

The owner may accumulate multiple valid keys through successive rotations without expiring previous ones. This is at the owner's discretion — more keys means more attack surface but also more resilience against individual key loss.

### 6.3 Rate Limiting

The **maximum rotation rate** is set in the genesis block (e.g., one new key per 24 hours). This prevents an attacker who compromises a single key from rapidly flooding the chain with rotations to lock out the legitimate owner. The recorder enforces this rate limit.

**Pre-live exemption:** Rate limits on rotation and revocation are not enforced until the first non-setup transaction (share assignment, CAA, etc.) is recorded. This allows the owner to establish multiple backup keys immediately after genesis before the chain goes live.

### 6.4 Key Revocation

An `OWNER_KEY_REVOCATION` block, signed by one or more currently valid owner keys, invalidates a specified owner key. You cannot revoke yourself into having zero valid keys.

#### Rate Limiting

The first revocation may be executed immediately (no rate-limit delay). Subsequent revocations are rate-limited: one revocation per `24 / N` hours, where `N` is the number of valid owner keys co-signing the revocation. A single key can eventually revoke all others, but it takes at least 24 hours to revoke two keys. Multiple keys acting together move faster.

#### Notifications

All parties (owner key holders, recorder, and any other listed notification contacts) must be notified immediately of both executed and pending revocations. Use **every available channel**: SSE/WebSocket push from the recorder, push notification servers (APNs, FCM) when available, MQTT topic alerts, email, SMS, and any other configured contact methods. If the receiving hardware supports audible alerts, trigger them. Key revocation is a high-severity event — over-notification is preferable to missed notification.

#### Override

A revocation signed by N valid keys may be overridden by a counter-message signed by N+1 valid keys. An override **reinstates** the revoked key and places the revoker(s) on hold. This works even if the revocation has already executed — the override reverses it. This allows the legitimate owner (holding a majority of keys) to undo an attacker's revocation.

#### Hold on Override (default, overridable)

When a pending revocation is overridden, the key(s) that signed the overridden revocation are placed **on immediate hold** — unable to sign any further actions — and become pending revocation themselves within 24 hours. This is a cooling-off period: the keys are effectively frozen, allowing the owner to investigate before the on-chain revocation finalizes. This behavior is enabled by default but may be disabled per-chain in the genesis block.

When the hold period expires, the recorder auto-constructs the revocation block as part of its block construction sweep (analogous to share expiration sweeps in EconomicRules.md §4). No owner signature is required — the hold-to-revocation transition is a deterministic consequence of the `OWNER_KEY_OVERRIDE` that any validator can verify. If no owner intervenes to explicitly pardon the held key before expiration, it is revoked. Auto-constructed revocations from hold expiration **count toward the revocation rate limit** — this prevents an attacker from triggering holds to exhaust the rate limit budget.

#### Protocol Note on Constants

All "magic numbers" (24-hour rate limit, hold periods, etc.) are default constants which may be varied per-chain. Where appropriate, they are included in the genesis block as operational parameters. Recorder software must be validated to properly handle individual chains with varying constant definitions.

### 6.5 Chain of Authority

Fully auditable on-chain: genesis key → rotation₁ → rotation₂ → ... with revocations and expirations visible in the block history. Validators can reconstruct the set of valid owner keys at any block height.

### 6.6 Ownership Transfer via Rotation

Key rotation subsumes simple ownership transfer: rotate to the buyer's key, then expire/revoke the seller's key. No chain migration needed unless a fresh chain identity is also desired.

## 7. Chain Migration (Ownership Transfer)

Distinct from recorder switch — the chain itself changes hands.

### 7.1 Process

1. **Freeze** the old chain (final block with `CHAIN_MIGRATION` pointer)
2. **Copy UTXOs** into a new chain under the new owner's genesis
3. Not a CAA exchange, not a bridge — UTXO carry-forward
4. New genesis is self-contained (see resolved Q27)

This is a new-chain-creation event, not a modification of the existing chain.

### 7.2 Signing Requirements

| Role | Signature | Requirement |
|------|-----------|-------------|
| Old chain owner | Signs `CHAIN_MIGRATION` block | RECOMMENDED |
| New chain owner | Signs new genesis | REQUIRED |
| Old chain recorder | Co-signs migration block | OPTIONAL |
| New chain recorder | Signs new genesis | REQUIRED |

If the old recorder has flaked out, life goes on — restore chain from backup and continue on a new recorder. If the old owner has lost keys, the new chain is still valid (see §7.3).

### 7.3 Migration Trust Tiers

When the old chain owner has lost keys, migration still works but with weaker continuity assurance:

| Tier | Old owner signs? | Assurance |
|------|-----------------|-----------|
| **Full** | Yes (old owner key) | Cryptographic proof of continuity |
| **Surrogate** | No, but proves majority share ownership (see below) | Economic proof — hijacking would be self-defeating |
| **Social** | Neither — new chain simply claims continuity | Customer/community verification only |

All three tiers produce a valid new chain. The difference is the strength of the continuity claim, which affects reputation/trust signals but not protocol validity.

**Surrogate tier mechanics:** The new genesis includes signed proofs from each UTXO key establishing majority ownership of the old chain. Each proof is a message signed by the UTXO's secret key, binding that UTXO to the new chain's genesis hash. Validators sum the proven share amounts; if they exceed 50% of the old chain's outstanding shares, the surrogate claim is valid. These proofs are embedded in the new genesis block alongside the carried-forward UTXOs.

**Hijacking is self-defeating:** If a hijacker creates a competing chain honoring old UTXOs, customers can spend shares on the hijacker's chain and still retain their secret keys (the wallet recognizes it as a different chain). When they encounter the real owner's chain, those shares are still valid there too. The hijacker gave away goods for free.

**Wallet implication (UI, not protocol):** When a chain migration occurs (new chain ID claiming old UTXOs), the wallet must treat old and new chains as distinct and retain old secret keys until the customer explicitly confirms the migration is legitimate.

## 8. New Type Codes and DataItem Structures

Type codes 128–159 are in the fourth inseparable band. Codes 128–143 allocated below; 144–159 reserved for future TⒶ³ extensions. (The third inseparable band, 64–95, is fully allocated to validator, CAA, and BLOB_POLICY types.)

### 8.1 Type Code Table

| Code | Name | Size | Description |
|---:|:---|:---|:---|
| 128 | `OWNER_KEY_ROTATION` | container | Owner key rotation (§6.1) |
| 129 | `OWNER_KEY_REVOCATION` | container | Owner key revocation (§6.4) |
| 130 | `RECORDER_CHANGE_PENDING` | container | Queued recorder switch; blocks new CAA (§5.1) |
| 131 | `RECORDER_CHANGE` | container | Recorder switch with new pubkey + URL (§5.1) |
| 132 | `RECORDER_URL_CHANGE` | container | Recorder URL update (§5.5) |
| 133 | `CHAIN_MIGRATION` | container | Final block, points to new chain (§7) |
| 134 | `RECORDER_IDENTITY` | container | Recorder self-description (§10.1) |
| 135 | `SURROGATE_PROOF` | container | UTXO ownership proof for surrogate migration (§7.3) |
| 136 | `RECORDER_URL` | variable | Recorder URL (UTF-8) |
| 137 | `RECORDING_FEE_ACTUAL` | variable | Actual fee charged in this block (rational, §4.3 of WireFormat) |
| 138 | `OWNER_KEY_OVERRIDE` | container | Revocation override + hold-on-override (§6.4) |
| 139 | `KEY_ROTATION_RATE` | 8 | Genesis parameter: minimum interval between rotations (timestamp delta) |
| 140 | `REVOCATION_RATE_BASE` | 8 | Genesis parameter: base revocation interval, default 24h (timestamp delta) |
| 141 | `REWARD_RATE` | variable | Share reward per transaction (rational, §4.3 of WireFormat) |
| 142 | `REWARD_RATE_CHANGE` | container | Adjusts reward rate; signed by both owner and recorder |
| 143 | `DESCRIPTION_INSEP` | variable | Inseparable human-readable text (UTF-8); used in OWNER_KEY_OVERRIDE |

### 8.2 DataItem Structures

```
OWNER_KEY_ROTATION (128, container)
├── ED25519_PUB (1): new owner public key
├── AUTH_SIG (30): signed by a currently valid owner key
└── TIMESTAMP (5): old key expiration (optional; absent = no expiration)

OWNER_KEY_REVOCATION (129, container)
├── ED25519_PUB (1): key being revoked
├── AUTH_SIG (30): signed by one or more currently valid owner keys
│   (multiple AUTH_SIG children for multi-key co-signing)
└── TIMESTAMP (5): effective time (immediate if ≤ now; future for pending)

RECORDER_CHANGE_PENDING (130, container)
├── ED25519_PUB (1): new recorder public key
├── RECORDER_URL (136): new recorder URL hint
└── AUTH_SIG (30): signed by a currently valid owner key

RECORDER_CHANGE (131, container)
├── ED25519_PUB (1): new recorder public key
├── RECORDER_URL (136): new recorder URL
├── AUTH_SIG (30): owner signature (REQUIRED)
└── AUTH_SIG (30): outgoing recorder signature (OPTIONAL)

RECORDER_URL_CHANGE (132, container)
├── RECORDER_URL (136): new URL
├── AUTH_SIG (30): recorder signature (REQUIRED)
└── AUTH_SIG (30): owner signature (REQUIRED)

CHAIN_MIGRATION (133, container)
├── CHAIN_REF (71): new chain ID (SHA2-256 of new genesis)
├── AUTH_SIG (30): owner signature (RECOMMENDED)
└── AUTH_SIG (30): recorder signature (OPTIONAL)

RECORDER_IDENTITY (134, container)
├── ED25519_PUB (1): recorder public key
├── RECORDER_URL (136): service URL
├── DESCRIPTION_INSEP (143): human-readable name/description (inseparable)
├── AUTH_SIG (30): self-signature
└── TIMESTAMP (5): publication timestamp

SURROGATE_PROOF (135, container)
├── SEQ_ID (7): UTXO sequence ID on old chain
├── AMOUNT (6): share amount of this UTXO
├── CHAIN_REF (71): new chain genesis hash (binds proof to specific new chain)
└── AUTH_SIG (30): signed by the UTXO's secret key

OWNER_KEY_OVERRIDE (138, container)
├── SHA256 (3): hash of the OWNER_KEY_REVOCATION being overridden
├── ED25519_PUB (1): key(s) placed on hold (one per key; from the overridden revocation's signers)
│   (multiple ED25519_PUB children if multiple keys signed the overridden revocation)
├── TIMESTAMP (5): hold expiration (default: 24h from recording; after which held keys are revoked)
├── AUTH_SIG (30): N+1 valid owner key signatures (one AUTH_SIG per co-signer)
│   (must exceed the number of signatures on the overridden revocation)
└── DESCRIPTION_INSEP (143): optional human-readable reason/notes (inseparable — must not be hash-substituted)

REWARD_RATE_CHANGE (142, container)
├── REWARD_RATE (141): new reward rate
├── AUTH_SIG (30): owner signature (REQUIRED)
└── AUTH_SIG (30): recorder signature (REQUIRED)
```

### 8.3 Genesis Parameters

New optional children of the `GENESIS` (15) container for TⒶ³ chains:

```
GENESIS (15, container)
├── ... (existing children: shares, coin label, fee rate, expiry, etc.)
├── REWARD_RATE (141): initial share reward per transaction (default: 0 = no reward)
├── KEY_ROTATION_RATE (139): min interval between rotations (default: 24h)
└── REVOCATION_RATE_BASE (140): base revocation interval (default: 24h)
```

If absent, defaults apply: no share reward, 24-hour rotation rate, 24-hour revocation base. The existing `FEE_RATE` (deflationary burn) continues to operate independently regardless of reward rate.

### 8.4 Per-Block Fee Transparency

Each `BLOCK_CONTENTS` (13) includes a `RECORDING_FEE_ACTUAL` (137) child — the resolved fee actually applied to this block, encoded as a rational fraction (same format as `RECORDING_BID`). This is the final value after any bid negotiation: the recorder's `RECORDING_BID` proposes a rate; `RECORDING_FEE_ACTUAL` records what was charged. Validators verify this does not exceed the chain's genesis fee ceiling.

## 9. Blob Retention on Migration

When a chain migrates (§7) or switches recorders (§5):

- **Recorder switch:** The new recorder inherits the chain's `BLOB_POLICY` (defined in genesis, per BlobRetentionReport.md). N33 sync includes blob transfer. The new recorder assumes retention obligations from the point of takeover; blobs already past their retention window on the old recorder may be absent.
- **Chain migration (new chain):** The new genesis may define its own `BLOB_POLICY`. Blobs from the old chain are not automatically carried forward — they remain on the old chain's recorder (or expire per its policy). If the new chain needs old blobs, they must be re-uploaded. The `CHAIN_MIGRATION` pointer allows clients to fetch historical blobs from the old recorder while it remains available.

## 10. Recorder Identity and Discovery

### 10.1 Publication

A recorder publishes its `RECORDER_IDENTITY` (§8.2) across every available channel. The recorder's key signs every publication, proving authenticity regardless of channel. Consistency across channels is the key trust signal.

**Required:**
- **On-chain on every chain the recorder serves.** This is the authoritative source — clients can always find their chain's recorder identity by reading the chain.

**Encouraged:**
- **Recorder registry chain.** Within a given market, each recorder should publish a chain listing known cooperating recorders. This forms part of the recorder reputation basis — a recorder vouching for peers creates a web of accountability.

**Optional (doesn't hurt):**
- **HTTP endpoint** (e.g., `GET /identity` on the recorder's URL)
- **DHT/gossip** for decentralized discovery
- **Static configuration** for bootstrap/pilot deployments

### 10.2 Architecture Layers

1. **Recorder Identity** — signed self-description (`RECORDER_IDENTITY`), Ed25519 key, published everywhere
2. **Recorder Registry** — discovery: static config → chain-level metadata → registry chains → DHT/gossip (progressive)
3. **Recorder Competition** — owner-selected (Model A only)
4. **Recorder Incentives** — fee competition, optional share rewards (`REWARD_RATE` genesis parameter)
5. **Fork Detection** — during botched transitions, validators may detect conflicting blocks at the same height from old and new recorders. Earliest valid block (by timestamp) wins; the fork is reported as a validator alert. This is a diagnostic/reputation signal, not a consensus mechanism — it can only occur during the transition window and resolves when one recorder stops.

### 10.3 API Note

All new block types (§8) are submitted via the existing `POST /chain/{id}/submit` endpoint. No new API endpoints are required — the recorder validates block type and signatures generically. Query endpoints (`GET /chain/{id}/blocks`, SSE events) already return all block types including TⒶ³ blocks.

## 11. Relationship to N33

N33 (recorder relay federation) is a **prerequisite** for TⒶ³. N33 should be designed as TⒶ³ building blocks:

- Chain state synchronization between recorders (needed for switch)
- Relay protocol for forwarding client requests during transition
- Block replay and blob fetch mechanisms

## 12. Comparable Systems Studied

| System | Relevance |
|--------|-----------|
| Nano | Block-lattice, ORV on conflict only |
| Holochain | Agent-centric, no global consensus |
| Stellar SCP | Subjective trust slices |
| Certificate Transparency | Auditability model |
| Hyperledger Fabric | Separated ordering/validation |
| Tendermint | BFT reference (contrast — we avoid this) |
| Ceramic Network | Stream-based, IPFS-backed |
| Avalanche | Probabilistic consensus (contrast) |

## 13. Resolved Questions

### ~~Q27: New-chain genesis validation during ownership transfer~~ RESOLVED

New genesis is self-contained — it lists the carried-forward UTXOs directly. The old chain's `CHAIN_MIGRATION` block is an informational pointer enabling optional audit. No cross-chain cryptographic validation required; the one-time-key property means unspent UTXOs are self-evidently valid (auditors can check the old chain to confirm none were spent).

### ~~Q28: `RECORDER_CHANGE` block signing requirements~~ RESOLVED

See §5.2 (recorder switch) and §7.2 (ownership transfer). Summary: new chain owner + new recorder = REQUIRED. Old chain owner = PREFERRED. Old recorder = OPTIONAL. Three migration trust tiers handle key loss scenarios (§7.3). Hijacking is structurally self-defeating due to one-time-key-per-chain design.

### ~~Q29: Recorder switch during active CAA escrows~~ RESOLVED

Recorder switch is **blocked** during active CAA escrows via a queued transition: `RECORDER_CHANGE_PENDING` drains active escrows (no new ones start), then `RECORDER_CHANGE` fires once all escrows have resolved. See §5.1.

### ~~Q30: `RECORDER_CHANGE` content — URL or just pubkey?~~ RESOLVED

**Pubkey + URL + fallback.** The `RECORDER_CHANGE` block includes both the new recorder's pubkey and URL. If the URL later changes (common with raw IP addresses), a `RECORDER_URL_CHANGE` block signed by both recorder and chain owner updates it on-chain. Old address can redirect; on-chain record assures legitimacy. See §5.3, §5.5.

### ~~Q31: Minimal acceptance test~~ RESOLVED

#### Test A: Recorder switch (happy path)

1. Create chain on Recorder A, issue shares to multiple recipients
2. Transact several blocks (normal operation)
3. Start a CAA escrow, then initiate `RECORDER_CHANGE_PENDING` to Recorder B
4. Verify new CAA escrows are blocked; existing CAA completes normally
5. `RECORDER_CHANGE` fires, Recorder B takes over
6. Transact on Recorder B — verify chain continuity, correct balances
7. Verify full chain history readable from Recorder B (block replay integrity)

#### Test B: Ownership transfer — Full tier (old owner signs)

1. Create chain on Recorder A, issue shares, transact
2. Old owner signs `CHAIN_MIGRATION` block freezing old chain
3. New owner creates new chain with carried-forward UTXOs
4. Verify UTXOs are valid on new chain, old chain is frozen
5. Verify audit trail: new genesis → `CHAIN_MIGRATION` → old chain history

#### Test C: Ownership transfer — Surrogate tier (key loss, majority share proof)

1. Create chain, issue shares, transact
2. Simulate old owner key loss (discard key)
3. New owner proves majority share ownership
4. New chain created with UTXO carry-forward, no old-owner signature
5. Verify chain is valid but trust tier reflects weaker assurance

#### Test D: Ownership transfer — Social tier (total key loss)

1. Create chain, issue shares, transact
2. Simulate total key loss (discard all owner keys)
3. New chain created claiming continuity with no cryptographic proof
4. Verify customer wallet retains old chain keys (treats chains as distinct)
5. Simulate hijacker creating competing chain with same UTXOs
6. Customer spends on hijacker chain, verify keys still valid on real owner's chain

#### Test E: Recorder URL change

1. Create chain on Recorder A at URL₁
2. Recorder moves to URL₂; both recorder and chain owner sign `RECORDER_URL_CHANGE`
3. Verify chain info reflects new URL
4. Verify old URL redirect (if reachable) is consistent with on-chain record

#### Test F: Owner key rotation and revocation

1. Create chain with genesis key K₁, set rotation rate limit (e.g., 1 per 24h)
2. Rotate to K₂ with no expiration on K₁ — verify both keys can sign owner operations
3. Rotate to K₃ — verify K₁, K₂, K₃ all valid
4. Revoke K₁ — verify K₁ immediately invalid, K₂ and K₃ still valid
5. Rotate to K₄ with expiration on K₂ — verify K₂ valid before expiration, invalid after
6. Attempt second rotation within rate limit window — verify rejected
7. Perform `RECORDER_CHANGE` signed by K₃ — verify accepted (non-genesis key works)

#### Test G: Ownership transfer via key rotation

1. Create chain, issue shares, transact
2. Owner rotates to buyer's key K_buyer
3. Owner revokes/expires all previous keys
4. Buyer signs `RECORDER_CHANGE` with K_buyer — verify accepted
5. Verify no chain migration needed — same chain ID, continuous history

#### Test H: Revocation override and hold-on-override

1. Create chain with keys K₁, K₂, K₃ (three rotations)
2. Attacker compromises K₁, uses it to revoke K₂ (first revocation — executes immediately)
3. Verify K₂ is revoked; all parties receive notifications via all channels
4. Legitimate owner signs `OWNER_KEY_OVERRIDE` with K₂ + K₃ (2 keys > 1 key) — override reinstates K₂
5. Verify K₂ is valid again — can sign owner operations
6. Verify K₁ placed on immediate hold — cannot sign any operations
7. Verify K₁ becomes pending revocation within 24h hold period
8. After hold expiration, verify recorder auto-constructs K₁ revocation; K₁ is fully revoked

#### Test I: Revocation rate limiting

1. Create chain with keys K₁, K₂, K₃, K₄ (revocation base = 24h)
2. K₁ revokes K₂ — verify immediate (first revocation)
3. K₁ attempts to revoke K₃ immediately — verify rejected (rate limited: 24/1 = 24h wait)
4. After 24h, K₁ revokes K₃ — verify accepted
5. K₁ + K₄ co-sign to revoke another key — verify rate = 24/2 = 12h (faster with more signers)

#### Test J: Cannot revoke to zero keys

1. Create chain with single key K₁
2. Rotate to K₂
3. K₂ revokes K₁ — verify accepted (K₂ remains)
4. K₂ attempts to revoke itself — verify rejected (would leave zero valid keys)

#### Test K: Override escalation

1. Create chain with keys K₁, K₂, K₃, K₄, K₅
2. K₁ revokes K₂
3. K₂ + K₃ override (2 > 1) — K₁ placed on hold
4. Attacker also holds K₃; K₃ + K₄ attempt to revoke K₅ (2 keys)
5. K₂ + K₄ + K₅ override (3 > 2) — K₃ and K₄ placed on hold
6. Verify final state: K₂ and K₅ valid; K₁, K₃, K₄ held/revoked

#### Test L: Uncooperative recorder fallback

1. Create chain on Recorder A, issue shares, transact
2. Owner submits `RECORDER_CHANGE_PENDING` — Recorder A refuses to record it
3. Owner creates new chain via `CHAIN_MIGRATION` on Recorder B
4. Verify UTXOs carried forward, old chain effectively frozen
5. Recorder A continues serving old chain — verify shares spent there are still valid on new chain
6. Recorder A discovers migration, records `CHAIN_MIGRATION` on old chain — verify both chains consistent

#### Test M: Reward rate change

1. Create chain with `REWARD_RATE` = R₁, issue shares, transact several blocks
2. Verify recorder receives R₁ shares per block to its key
3. Owner and recorder co-sign `REWARD_RATE_CHANGE` to R₂
4. Transact further blocks — verify recorder now receives R₂ per block
5. Attempt `REWARD_RATE_CHANGE` signed by owner only — verify rejected (requires both)

#### Test N: Blob retention across recorder switch

1. Create chain on Recorder A with `BLOB_POLICY`, upload blobs, transact
2. Initiate recorder switch to Recorder B
3. Verify N33 sync transfers blobs to Recorder B
4. After cutover, verify blobs accessible from Recorder B
5. Verify blobs past retention window on old recorder may be absent on new recorder

#### Test O: CAA escrow drain before recorder switch

1. Create chain on Recorder A
2. Start two CAA escrows with different deadlines
3. Initiate `RECORDER_CHANGE_PENDING` to Recorder B
4. Attempt to start new CAA — verify rejected
5. First escrow completes — verify `RECORDER_CHANGE` does NOT fire (second still active)
6. Second escrow expires — verify `RECORDER_CHANGE` fires automatically
7. Verify new CAA can proceed on Recorder B

#### Test P: Key expiration timing

1. Create chain with key K₁, rotate to K₂ with K₁ expiration at T+48h
2. At T+24h: sign operation with K₁ — verify accepted (not yet expired)
3. At T+48h+1: sign operation with K₁ — verify rejected (expired)
4. Verify K₂ unaffected by K₁ expiration

---

## Appendix: Scope

**In scope:** recorder switching, fee competition, ownership transfer, owner key rotation, type codes. State synchronization is defined by N33 (§11) and referenced here only as a dependency.

**Out of scope:** TⒶ⁴ (underwriter/checker bounty system), global chain directory, regulatory compliance, simultaneous multi-writer consensus.
