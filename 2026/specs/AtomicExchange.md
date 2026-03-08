# Atomic Multi-Chain Exchange — CAA Protocol (Phase 6)

Specification for Conditional Assignment Agreements (CAA): atomic cross-chain share transfers with escrow, recording proofs, and timeout recovery.

Related specs: [Architecture.md](Architecture.md) (0A), [WireFormat.md](WireFormat.md) (0B), [EconomicRules.md](EconomicRules.md) (0D).

Reference: [2018 MultichainExchange design](../../docs/html/MultichainExchange.html) — conceptual origin, not binary-compatible.

---

## 1. Overview

A CAA enables atomic exchange across independent chains. Either all per-chain assignments complete, or none do — shares are never permanently lost.

**Example:** Alice holds 135 CCC. She wants to give Bob 1 BCG. Charlie (exchange agent) offers 12 CCC for 1 BCG. The CAA atomically executes:
- CCC chain: Alice gives 135 CCC → Alice receives 123 CCC change, Charlie receives 12 CCC
- BCG chain: Charlie gives 15 BCG → Charlie receives 14 BCG change, Bob receives 1 BCG

### 1.1 Design Principles

1. **No permanently locked shares.** Every escrow has a deadline. Timeout always releases escrowed shares back to givers.
2. **Ouroboros recording.** Chains are ordered. Each recording produces a proof that unlocks the next chain. The last chain's recording makes the CAA binding; that proof is then recorded back to all earlier chains.
3. **Existing recorder model.** Each chain's recorder remains sovereign. CAA coordination is a client-side concern — recorders process CAA components as specialized assignments, not as a new consensus protocol.
4. **Graceful degradation.** A chain that doesn't support CAA simply rejects CAA submissions. Clients fall back to the exchange-agent model (Phase 4).

### 1.2 State Machine

```
proposed → signed → recording → binding → finalized
                         ↓
                      expired
```

| State | Meaning |
|-------|---------|
| **proposed** | CAA structure built, not yet fully signed |
| **signed** | All participants have signed all components |
| **recording** | First chain has recorded; ouroboros in progress |
| **binding** | Last chain recorded before deadline; CAA is irreversible |
| **finalized** | Binding proof recorded on all chains; receivers can spend |
| **expired** | Deadline passed without completing recordings; escrowed shares released |

---

## 2. Wire Format

### 2.1 New Type Codes

All in the second inseparable band (|code| 64–95), continuing from validator codes (64–68).

| Code | Name | Size | Description |
|---:|:---|:---|:---|
| 69 | `CAA` | container | Conditional Assignment Agreement — top-level container |
| 70 | `CAA_COMPONENT` | container | Per-chain component: chain ref + assignment + signatures |
| 71 | `CHAIN_REF` | 32 | SHA2-256 chain ID (fixed, references a chain) |
| 72 | `ESCROW_DEADLINE` | 8 | Escrow expiration timestamp (after which shares auto-release) |
| 73 | `CHAIN_ORDER` | vbc-value | 0-based position of this chain in the ouroboros sequence |
| 74 | `RECORDING_PROOF` | container | Proof that a CAA component was recorded on a chain |
| 75 | `CAA_HASH` | 32 | SHA2-256 hash of the CAA (all components, no proofs) |
| 76 | `BLOCK_REF` | container | Reference to a specific block: chain ID + height + hash |
| 77 | `BLOCK_HEIGHT` | vbc-value | Block height within a BLOCK_REF |
| 78 | `COORDINATOR_BOND` | variable | Bond amount declared by coordinator on non-last chains; forfeited on escrow timeout |

All codes 69–78 satisfy `|code| & 0x20 == 0` (inseparable) — correct for protocol-critical data.

### 2.2 CAA Structure

```
CAA (69)
├── ESCROW_DEADLINE (72): timestamp — global escrow expiration
├── LIST_SIZE (27): number of chain components
├── CAA_COMPONENT (70) [chain 0 — first in ouroboros order]
│   ├── CHAIN_REF (71): 32-byte chain ID
│   ├── CHAIN_ORDER (73): 0
│   ├── COORDINATOR_BOND (78): bond amount (required for non-last chains)
│   ├── ASSIGNMENT (8): per-chain assignment (givers + receivers + fee bid)
│   └── AUTH_SIG (30) [per participant on this chain]
│       ├── ED25519_SIG (2)
│       ├── TIMESTAMP (5)
│       └── PAGE_INDEX (29)
├── CAA_COMPONENT (70) [chain 1]
│   ├── CHAIN_REF (71): 32-byte chain ID
│   ├── CHAIN_ORDER (73): 1
│   ├── ASSIGNMENT (8): per-chain assignment
│   └── AUTH_SIG (30) [per participant]
│       └── ...
└── AUTH_SIG (30) [overall CAA signatures — all participants across all chains]
    ├── ED25519_SIG (2): signature over the entire CAA (excl. overall sigs)
    ├── TIMESTAMP (5)
    └── ED25519_PUB (1): signer's public key
```

**Signing rule:** Each participant signs:
1. Their per-component `AUTH_SIG` — signs the component's `ASSIGNMENT` (same as Phase 2 assignment signing).
2. An overall `AUTH_SIG` — signs the entire `CAA` container (all components, excluding overall signatures). This binds the participant to the cross-chain atomic intent.

### 2.3 Recording Proof

When a recorder records a CAA component, it produces a recording proof:

```
RECORDING_PROOF (74)
├── CHAIN_REF (71): chain ID where this was recorded
├── BLOCK_REF (76)
│   ├── CHAIN_REF (71): chain ID (same)
│   ├── BLOCK_HEIGHT (77): block height
│   └── SHA256 (3): block hash
├── CAA_HASH (75): SHA2-256 of the signed CAA
└── AUTH_SIG (30): recorder's block-signing key signature over this proof
    ├── ED25519_SIG (2)
    ├── TIMESTAMP (5)
    └── ED25519_PUB (1)
```

### 2.4 Binding Submission

To finalize a CAA on a chain, POST a JSON binding submission to `/chain/{id}/caa/bind`:

```json
{
  "caa_hash": "<hex-encoded 32-byte CAA hash>",
  "proofs": [
    <RECORDING_PROOF DataItem JSON from chain 0>,
    <RECORDING_PROOF DataItem JSON from chain 1>,
    ...
  ]
}
```

The `proofs` array must contain one recording proof per chain in the CAA (total_chains entries).

---

## 3. Escrow Protocol

### 3.1 Ouroboros Recording Sequence (2 chains)

For a CAA involving chains A (order 0) and B (order 1):

```
  Client          Chain A (AOR)       Chain B (AOR)
    │                  │                    │
    │  1. Submit CAA   │                    │
    │  (escrow)───────>│                    │
    │                  │                    │
    │  2. Recording    │                    │
    │     proof A ◄────│                    │
    │                  │                    │
    │  3. Submit CAA + │                    │
    │     proof A ─────│───────────────────>│
    │                  │                    │
    │  4. Recording    │                    │
    │     proof B ◄────│────────────────────│
    │                  │   (CAA now BINDING) │
    │                  │                    │
    │  5. Submit       │                    │
    │     binding ────>│                    │
    │     (proofs A+B) │                    │
    │                  │  (CAA FINALIZED)   │
    │                  │                    │
```

**Step 1:** Client submits the fully-signed CAA to chain A's recorder. The recorder validates the component assignment, places giver shares in escrow, and records the CAA component in a block. Returns a recording proof.

**Step 2–3:** Client takes chain A's recording proof and submits the CAA plus proof to chain B. Chain B validates its component, verifies chain A's recording proof, escrows givers' shares, and records. Returns recording proof B.

**Step 4:** With both proofs, the CAA is now **binding** on chain B (the last chain). Receivers on chain B can spend immediately.

**Step 5:** Client submits binding proof (both recording proofs) to chain A. Chain A verifies the proofs, transitions the CAA from escrowed to finalized, and receivers on chain A can now spend.

### 3.2 N-Chain Generalization

For N chains ordered 0..N-1:
1. Record on chain 0 (escrow). Get proof 0.
2. Record on chain 1 with proof 0 (escrow). Get proof 1.
3. ...
4. Record on chain N-1 with proofs 0..N-2 (escrow + binding). Get proof N-1.
5. Submit binding (all N proofs) to chains 0..N-2.

Chain N-1 receives all prior proofs during its initial recording and becomes binding immediately. All other chains need a follow-up binding submission.

### 3.3 Escrow Rules

1. **Escrow locks shares.** When a CAA component is recorded, giver UTXOs transition to `Escrowed` status. They cannot be spent in regular assignments while escrowed.
2. **Deadline enforcement.** If the escrow deadline passes without a binding proof being recorded, the recorder automatically releases escrowed shares. Released shares return to `Unspent` status.
3. **No partial binding.** A CAA is either fully binding (all chains recorded) or not binding at all. There is no partial state.
4. **Binding is irreversible.** Once a chain records a valid binding proof, the escrow transitions to spent/received. No refutation can undo this.
5. **Recording proofs are verifiable.** Each proof includes the recorder's signature over the block reference. The receiving recorder can verify this signature against a known/configured recorder public key for that chain.

### 3.4 Timeout and Recovery

**Auto-release:** Recorders sweep escrowed UTXOs whose deadline has passed. This runs as part of the normal block construction cycle (alongside expiry sweeps). Released shares return to `Unspent`.

**Client retry:** If a recording fails mid-ouroboros (network error, recorder down), the client retries with exponential backoff: 1s, 2s, 4s, 8s, ... up to 60s. The client continues retrying until either:
- The recording succeeds and the ouroboros completes.
- The escrow deadline passes and all shares auto-release.

**Idempotency:** Submitting the same CAA component twice to a recorder is idempotent — the second submission returns the existing recording proof without creating a new block.

**No permanently locked shares.** The escrow deadline guarantees that shares always become available again. The worst case is that shares are locked for the escrow period (default: 5 minutes, configurable per-CAA).

---

## 4. Recorder Changes

### 4.1 New UTXO Status

Add `Escrowed` to `UtxoStatus`:

```
Unspent → Escrowed (via CAA recording)
Escrowed → Spent (via binding proof — giver side)
Escrowed → Unspent (via timeout release)
Unspent → Spent (via regular assignment — unchanged)
```

### 4.2 New Database Tables

```sql
CREATE TABLE IF NOT EXISTS caa_escrows (
    caa_hash     BLOB NOT NULL,      -- SHA2-256 of the CAA
    chain_order  INTEGER NOT NULL,   -- this chain's position in ouroboros
    deadline     INTEGER NOT NULL,   -- escrow deadline (AO timestamp)
    status       TEXT NOT NULL DEFAULT 'escrowed',  -- escrowed | binding | finalized | expired
    block_height INTEGER NOT NULL,   -- block where CAA component was recorded
    proof_data   BLOB,               -- serialized RECORDING_PROOF for this chain
    total_chains INTEGER NOT NULL DEFAULT 0, -- number of chains in the CAA (for bind validation)
    bond_amount  BLOB NOT NULL DEFAULT x'00', -- coordinator bond (VBC-encoded BigInt); forfeited on timeout
    PRIMARY KEY (caa_hash)
);

CREATE TABLE IF NOT EXISTS caa_utxos (
    caa_hash BLOB NOT NULL,
    seq_id   INTEGER NOT NULL,
    role     TEXT NOT NULL,         -- 'giver' or 'receiver'
    PRIMARY KEY (caa_hash, seq_id),
    FOREIGN KEY (caa_hash) REFERENCES caa_escrows(caa_hash)
);

CREATE TABLE IF NOT EXISTS caa_giver_history (
    pubkey     BLOB NOT NULL,       -- 32-byte Ed25519 public key
    caa_hash   BLOB NOT NULL,       -- references caa_escrows
    escrowed_at INTEGER NOT NULL,   -- timestamp when escrowed
    PRIMARY KEY (pubkey, caa_hash)
);

CREATE TABLE IF NOT EXISTS known_recorder_keys (
    chain_id   BLOB NOT NULL,       -- 32-byte chain ID
    pubkey     BLOB NOT NULL,       -- 32-byte recorder public key
    added_at   TEXT NOT NULL,       -- ISO 8601 timestamp
    revoked_at TEXT,                -- NULL if active
    PRIMARY KEY (chain_id, pubkey)
);
```

### 4.3 New Endpoints

| Method | Endpoint | Request | Response |
|--------|----------|---------|----------|
| POST | `/chain/{id}/caa/submit` | Signed CAA JSON (+ optional recording proofs from prior chains) | Recording proof JSON |
| POST | `/chain/{id}/caa/bind` | Binding submission: CAA hash + all recording proofs | Block info JSON |
| GET | `/chain/{id}/caa/{caa_hash}` | — | CAA status: state, deadline, block height, proofs |
| GET | `/admin/recorder-keys` | — | List of known recorder keys (chain ID, pubkey, status) |
| POST | `/admin/recorder-keys` | `{"chain_id": "<hex>", "pubkey": "<hex>", "action": "add"|"revoke"}` | Updated key entry |

### 4.4 Validation Rules for CAA Submit

1. CAA must contain a valid `CAA_COMPONENT` for this chain (matched by `CHAIN_REF`).
2. **Chain count limit.** The CAA must contain at most **8 chains** (`MAX_CAA_CHAINS`). This bounds the amplification factor of N-chain attacks.
3. Component's `ASSIGNMENT` must pass all standard assignment validation (signatures, UTXO availability, key uniqueness, balance equation).
4. `ESCROW_DEADLINE` must be in the future and at most **10 minutes** from current time (anti-griefing: prevents capital lockup via excessively long deadlines).
5. **Per-giver rate limit.** Each giver public key may have at most **3** active escrows (`MAX_ACTIVE_ESCROWS_PER_GIVER`). This prevents capital-lockup cycling attacks where an adversary repeatedly escrows a victim's shares.
6. **Coordinator bond (non-last chains).** For chains where `CHAIN_ORDER` < total_chains - 1, the CAA component must include a `COORDINATOR_BOND (78)` field. The bond amount must be ≥ **10%** of the total giver amount on that component (`MIN_BOND_FRACTION = 1/10`). The bond creates an economic disincentive for the last-chain theft attack: if the coordinator completes the last chain but abandons earlier chains, the bond is forfeited on timeout. Bond validation runs before signature verification.
7. All overall `AUTH_SIG` signatures must verify against the CAA content. The number of overall signatures must equal the total number of participants across all components. No two overall signatures may be from the same public key. Every locally-verifiable participant (local givers by UTXO lookup + all receivers by pubkey) must have a corresponding overall signature.
8. If this chain's `CHAIN_ORDER` > 0, recording proofs for all prior chains must be present and valid. Each proof's `CHAIN_REF` must match the expected chain at that position in the ouroboros order.
9. Giver UTXOs must be `Unspent` (not already escrowed or spent). UTXOs released from a prior escrow have a **30-second cooldown** before they can be re-escrowed (anti-cycling).
10. `CHAIN_ORDER` values across all components must be contiguous integers 0..N-1 (where N = `LIST_SIZE`), with no duplicates.

### 4.5 Validation Rules for CAA Bind

1. `CAA_HASH` must match an existing escrowed CAA on this chain.
2. Recording proofs must be present for ALL chains in the CAA. The expected count is read from `total_chains` in the stored escrow record — this avoids the client needing to re-submit the CAA structure.
3. Each proof must contain a valid recorder signature. Proofs must cover exactly N **distinct** chains (no duplicate `CHAIN_REF` values).
4. The escrowed CAA must not be expired.

### 4.6 Block Construction During CAA Submit

When a CAA component is accepted, the recorder constructs a real block (identical structure to a regular assignment block) containing the CAA component's authorization as a page. This ensures:

- The recording proof references a real block hash (not a placeholder).
- The chain's block height, hash chain, and shares_out advance correctly.
- The CAA submission is visible in the chain's block history.

The fee is deducted from `SHARES_OUT` as with regular assignments. Giver UTXOs are marked `Escrowed` (not `Spent`), and receiver UTXOs are created with `Escrowed` status (not `Unspent`) — they become spendable only after binding.

---

## 5. Escrow Sweep

Added to the block construction cycle, after the existing expiry sweep. The sweep is non-fatal — if it fails, block production continues (the next block will retry).

```rust
// In block construction, after expiry sweep:
if let Ok((count, fee_restore)) = run_escrow_sweep(store, current_timestamp) {
    shares_out += fee_restore;
}
```

The escrow sweep finds all `caa_escrows` with `status = 'escrowed'` and `deadline < current_timestamp`. For each expired escrow:

1. Set escrow status to `expired`.
2. For each **giver** UTXO in `caa_utxos`: transition from `Escrowed` back to `Unspent` (shares preserved).
3. For each **receiver** UTXO in `caa_utxos`: **delete** the UTXO entirely (these are phantom receivers that were never finalized) and remove the public key from `used_keys` (allowing the key to be reused in a future assignment).
4. **Restore the recording fee minus bond.** The restored amount = fee - bond_amount. The fee = sum(giver amounts) - sum(receiver amounts). The bond forfeiture effectively burns shares: `shares_out` is increased by only `(fee - bond_amount)` rather than the full `fee`, so `bond_amount` shares are permanently retired from circulation. This creates an economic penalty for escrow timeout — the coordinator loses real value. On successful binding, no bond is forfeited — the full fee is retained by the recorder as normal. (The bond is not a separate UTXO; it is a counter-field in the `caa_escrows` table that controls how much of the fee is restored on timeout.)

Step 3 is essential for accounting: without it, phantom receiver UTXOs would permanently consume sequence IDs and lock public keys. Step 4 maintains the `SHARES_OUT` invariant — since the CAA assignment was never finalized, the fee deduction is partially reversed (reduced by the forfeited bond).

This is distinct from the expiry sweep (which retires shares permanently). Escrow release preserves all shares.

---

## 6. Recording Proof Verification

A recorder receiving a recording proof from another chain must verify:

1. **Proof structure:** Contains `CHAIN_REF`, `BLOCK_REF`, `CAA_HASH`, and `AUTH_SIG`.
2. **CAA hash match:** The `CAA_HASH` in the proof matches the submitted CAA's hash.
3. **Recorder signature:** The `AUTH_SIG` verifies against a known public key for that chain's recorder.

**Recorder key discovery:** Recorder public keys are loaded from TOML config at startup under `[known_recorders]`, and can be dynamically added or revoked at runtime via the `/admin/recorder-keys` endpoint. Keys are stored in the `known_recorder_keys` database table and held in an `RwLock<HashMap>` for concurrent read access. Revoked keys are soft-deleted (`revoked_at` timestamp set) and immediately excluded from proof verification.

```toml
[known_recorders]
# chain_id_hex = "recorder_pubkey_hex"
"abc123..." = "def456..."
```

---

## 7. Client-Side Coordination

The CAA coordinator runs client-side (in ao-exchange agent, ao-cli, or ao-pwa). It:

1. Builds the CAA structure with components for each chain.
2. Collects signatures from all participants.
3. Submits to chain 0, gets proof 0.
4. Submits to chain 1 with proof 0, gets proof 1.
5. ... continues through all chains.
6. Submits binding proof back to chains 0..N-2.

**Failure handling:** If any submission fails:
- Retry with exponential backoff.
- If deadline approaches (< 30 seconds remaining), stop retrying and let escrows expire.
- Log the failure for post-mortem analysis.

The coordinator is implemented in `ao-exchange` as a reusable async function, callable from both the exchange agent daemon and CLI tools.

---

## 8. Scope Boundaries

**In scope:**
- CAA wire format and type codes
- Escrow UTXO state and timeout release
- Recording proof generation and verification
- Ouroboros recording sequence (client-driven)
- CAA recorder endpoints (submit, bind, status)
- CAA coordinator in ao-exchange
- CLI commands for manual CAA operations
- Unit and integration tests

**Out of scope:**
- PWA CAA UI (deferred — Phase 4's exchange agent model is sufficient for end users)
- CAA involving 3+ chains (tested but not optimized; hard-capped at 8)
- Competing recorders per chain (TⒶ³)

---

## 9. Security Mitigations

### 9.1 Last-Chain Theft Attack

**Attack:** The coordinator completes recording on the last chain (making it binding and irreversible), then abandons the earlier chains. Escrows on earlier chains expire, returning shares to their givers — but the last chain's givers lose their shares permanently. The coordinator effectively steals from last-chain givers.

**Mitigation: Coordinator bond.** Non-last chains require a `COORDINATOR_BOND (78)` field worth ≥ 10% of the giver total on that component. On escrow timeout, the bond is forfeited (burned from `shares_out`). This makes the attack unprofitable: the coordinator must post bonds on all N-1 non-last chains, and abandoning them forfeits all bonds. The bond fraction is tunable via `MIN_BOND_FRACTION_NUM / MIN_BOND_FRACTION_DEN` constants.

### 9.2 Capital Lockup Cycling

**Attack:** An adversary repeatedly creates CAAs that escrow a victim's shares, locking them for the escrow period (up to 10 minutes). With 30-second cooldown between escrows, this achieves ~95% lockup rate, effectively denying the victim use of their shares.

**Mitigation: Per-giver rate limiting.** Each giver public key may have at most `MAX_ACTIVE_ESCROWS_PER_GIVER` (3) active escrows simultaneously. Tracked in `caa_giver_history` table, joined against active escrows. This bounds the maximum lockup to 3 × escrow_period, and ensures the victim always retains access to shares not in active escrows.

### 9.3 N-Chain Amplification

**Attack:** A malicious CAA includes a large number of chains, amplifying the complexity and resource cost of validation, and increasing the window for partial-completion attacks.

**Mitigation: Chain count limit.** `MAX_CAA_CHAINS` (8) caps the number of chains per CAA. Validation rejects CAAs exceeding this limit before any expensive processing.

### 9.4 TOCTOU Atomicity

**Confirmed safe.** The `Mutex<ChainStore>` acquired inside `spawn_blocking` plus SQLite's `BEGIN IMMEDIATE` transaction ensures that UTXO validation and escrow recording are atomic. No time-of-check-to-time-of-use gap exists.

### 9.5 Recorder Key Compromise

**Mitigation: Dynamic key rotation.** The `/admin/recorder-keys` endpoint allows adding new recorder keys and revoking compromised ones at runtime without restarting the recorder. Revoked keys are immediately excluded from recording proof verification. The `known_recorder_keys` database table provides an audit trail.

---

## 10. Acceptance Criteria

1. Three-party two-chain CAA (Alice → Bob via Charlie, CCC + BCG) completes in < 30 seconds.
2. Server failure during ouroboros causes correct escrow release after deadline.
3. No share loss or double-spend under concurrent CAA + regular assignments.
4. Escrowed shares cannot be spent in regular assignments.
5. Binding proof on one chain does not require the other chain to be reachable.
6. Expired escrow correctly returns shares to `Unspent`.
7. Idempotent CAA submission (re-submit returns existing proof).
