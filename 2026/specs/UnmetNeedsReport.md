# Unmet Real-World Needs: Gap Analysis and Roadmap Proposals

**Date:** 2026-03-08
**Status:** Draft for review — edit with comments, then distill into ROADMAP tasks.
**Companion:** `BlobRetentionReport.md` covers blob retention and on-chain linking separately.

---

## 1. Consumer (AOE) — "I buy things with AO shares"

### 1.1 No transaction history

**Current state:** `ConsumerView.tsx` shows current UTXO balances but no record of
past transfers. The recorder stores all blocks, and the PWA fetches them
(`GET /chain/{id}/blocks?from=&to=`), but nothing renders a history view.

**What users need:** "Show me what I spent last week." A scrollable list of
outgoing/incoming transfers with date, amount, recipient chain/pubkey, and any
attached blob thumbnails. Filtering by date range. Export to CSV for personal
bookkeeping.

**Implementation sketch:**
- New `TransactionHistory.tsx` component.
- Walks blocks from recorder, filters for assignments involving the user's public
  keys (as giver or receiver).
- Displays: timestamp, amount (coins), direction (sent/received), counterparty
  pubkey (truncated), blob attachment indicator.
- IndexedDB cache of processed transactions for offline access and fast reload.
- CSV export button.

**Depends on:** N8a (on-chain blob linking) for attachment indicators. Can ship
without it — just omit attachment column initially.

**Add to Roadmap** now.

---

### 1.2 No wallet backup/restore UX

**Current state:** `wallet.ts` has `encryptSeed()` and `decryptSeed()` using
PBKDF2+AES-GCM. The wallet sync spec (`WalletSync.md` §3) defines
`buildFullExportPayload()` for complete wallet backup. But the PWA has no button to
trigger export or import.

**What users need:** "Back up my wallet to a file" and "Restore from backup." One
cleared browser or lost phone = permanent fund loss without this.

**Implementation sketch:**
- Settings panel: "Export Wallet Backup" → password prompt → downloads encrypted
  JSON file (the sync payload from `walletSync.ts` encrypted with
  `encryptSeed()`).
- Settings panel: "Import Wallet Backup" → file picker + password → decrypts,
  calls `importSyncPayload()`, merges keys into IndexedDB.
- Backup file format: `{"v":1, "type":"encrypted_backup", "data":"<hex>"}` where
  `data` is the encrypted sync payload. Compatible with the wallet sync spec's
  existing structures.
- Warning on first wallet creation: "No backup exists. Back up now?"
- Periodic reminder if no backup has been made in 30 days.

**Depends on:** Wallet sync infrastructure (`walletDb.ts`, `walletSync.ts`) which
is already implemented.

**Add to Roadmap** now for file backup and add another roadmap item for user backup to cloud
when cloud implementation is available.

---

### 1.3 No transfer confirmation screen

**Current state:** `ConsumerView.tsx` builds and submits the assignment in one
action. No preview of amounts, fees, or recipient before the irreversible submit.

**What users need:** "You're sending 5.2 BCG to `a3f8...c721`, fee 0.003 BCG,
change 2.1 BCG to new key. Confirm?" A modal or step-2 screen before final
submission.

**Implementation sketch:**
- Split the transfer flow into two steps: build (compute amounts, fees, change)
  and confirm (display summary, await user tap).
- `buildAssignment()` already returns the constructed DataItem — extract amounts
  from it for display before calling `submitBlock()`.
- Show: total sent, fee, change returned, recipient pubkey (or "new key on this
  chain"), number of attachments.
- "Edit" button to go back, "Confirm & Send" to submit.

**Depends on:** Nothing. Pure UI change.

**Add to Roadmap** now.

---

### 1.4 No offline balance cache

**Current state:** Balance is recomputed from chain data on every view via
`GET /chain/{id}/utxo/{seq_id}` calls. Offline = no balance display.

**What users need:** "I can see my balance even without internet." The shares are
theirs regardless of connectivity. Seeing a zero balance offline erodes trust.

**Implementation sketch:**
- `walletDb.ts` already stores `amount` per key entry in IndexedDB.
- `chainBalance()` already sums unspent keys per chain.
- On app load: display cached balance immediately from IndexedDB (with
  "last verified: 2 hours ago" indicator).
- When online: validate against recorder, update IndexedDB, refresh display.
- Stale indicator: if last validation > 1 hour, show yellow "unverified" badge.

**Depends on:** Wallet sync migration (already done — `migrateFromLocalStorage()`
populates IndexedDB).

**Add to Roadmap** now.

---

### 1.5 No refutation UI

**Current state:** `ao refute` CLI command exists for disputing fraudulent or late
assignments. No PWA equivalent.

**What users need:** If a consumer receives a suspicious assignment (e.g. someone
recorded an old agreement after the consumer already spent those shares elsewhere),
they need a way to formally refute it from their phone.

**Implementation sketch:**
- "Dispute" button on transaction history entries (or UTXO detail view).
- Builds a refutation DataItem (type code for REFUTE exists in the protocol),
  signs it, submits to recorder.
- Confirmation dialog explaining consequences: "This will permanently mark
  assignment #{seq} as disputed. The recorder will reject any future transfers
  based on this assignment."

**Priority assessment:** Low frequency event. Most users will never need this.
But when needed, it's critical (fraud defense). Could ship as a "power user"
feature behind a settings toggle initially.

**Depends on:** Transaction history (§1.1) for discoverability. Can also be
triggered from UTXO detail view independently.

**Add to Roadmap** now, as a "power user" feature.

---

### 1.6 No fiat on/off-ramp

**Current state:** Getting AO shares requires finding an exchange agent (Phase 4
market maker) or receiving shares directly from a vendor/peer. No integration with
M-Pesa, bank transfers, USDC, or any fiat payment system.

**What users need:** "I want to buy 100 BCG with my credit card" or "Cash out my
shares to M-Pesa."

**Assessment:** This is a business/regulatory challenge more than a technical one.
AO is designed so exchange agents are independent operators who set their own
rates and accept whatever payment methods they choose. The protocol deliberately
avoids embedding fiat rails.

**What the roadmap could include:**
- Exchange agent documentation/template for operators who want to accept mobile
  money, card payments, or stablecoins.
- PWA deep-link format for exchange agents: scan a QR code that opens the
  exchange agent's payment page with pre-filled chain/amount parameters.
- API webhook for exchange agents to confirm fiat receipt and trigger automatic
  share transfer.

**Depends on:** Exchange agent infrastructure (Phase 4, done). The gap is
operational tooling, not protocol.

**Add to Roadmap** later.  On/off ramp targets need to be identified first.

---

## 2. Vendor (AOS) — "I run a business and accept AO shares"

### 2.1 Vendor profile lost on recorder restart

**Current state:** `POST /chain/{id}/profile` stores vendor profiles in the
recorder's in-memory state. A process restart or power outage loses all profiles.
Vendor disappears from map and discovery until they re-enter their info.

**What users need:** Profile persistence. Set it once, it survives reboots.

**Implementation sketch:**
- Add `vendor_profiles` SQLite table: `(chain_id TEXT PK, name TEXT, description
  TEXT, lat REAL, lng REAL, updated_at INT)`.
- `POST /chain/{id}/profile` writes to SQLite instead of (or in addition to)
  in-memory map.
- `GET /chain/{id}/profile` reads from SQLite.
- On startup: profiles are available immediately from database.
- Future: record profiles on-chain as `VENDOR_PROFILE` (type 36) separable items
  for tamper-proof audit trail. The SQLite table serves as a fast-access cache.

**Depends on:** Nothing. Straightforward persistence change in `ao-recorder`.

**Add to Roadmap** now, including on-chain records.

---

### 2.2 No printable QR signage

**Current state:** `QrCode.tsx` renders QR codes on-screen using the `qrcode`
library. No print layout, no signage template.

**What users need:** A beach vendor needs a laminated card: QR code + chain symbol
+ business name + "Scan to pay" text. Print-optimized, high-contrast, correct
physical size for scanning distance.

**Implementation sketch:**
- "Print QR" button in `VendorView` or `ChainDetail`.
- Opens a print-optimized page/modal: high-DPI QR code (SVG preferred for
  scalability), chain symbol in large text, business name, optional logo area.
- CSS `@media print` styles: white background, no navigation chrome, appropriate
  margins for standard paper sizes.
- Size guidance: QR code minimum 3 cm × 3 cm for reliable scanning at arm's
  length; 6 cm × 6 cm for countertop signage.

**Depends on:** Nothing. Pure frontend feature.

**Add to Roadmap** now, include output as a .pdf file and .png images as options.

---

### 2.3 No sales reporting

**Current state:** `VendorView.tsx` has a real-time SSE monitor showing incoming
blocks with assignment count and seq range. No aggregation, no historical view.

**What users need:** "How much did I make today/this week/this month?" Daily totals,
transaction count, average transaction size. CSV export for tax/accounting purposes.

**Implementation sketch:**
- New `SalesReport.tsx` component (or tab within VendorView).
- Fetches blocks from recorder, filters for assignments to vendor's chain.
- Aggregates: daily/weekly/monthly totals in coins, transaction count, average
  size, largest transaction.
- Date range picker.
- CSV export: date, time, amount (coins), amount (shares), sender pubkey
  (truncated), block height, seq ID.
- IndexedDB cache for offline access to historical reports.

**Depends on:** Recorder block pagination API (already exists:
`GET /chain/{id}/blocks?from=&to=`).

**Add to Roadmap** now.

---

### 2.4 No payment notifications

**Current state:** SSE events arrive while the PWA is open and focused. Close the
browser tab = no notification. No push notifications, no background sync.

**What users need:** A vendor hears a chime or sees a notification banner when
payment arrives, even if the app is in the background.

**Implementation sketch:**
- **Web Push API** (requires VAPID keys and a push server):
  - Recorder sends push notification on new block with assignments to subscribed
    chains.
  - PWA service worker receives push, shows system notification: "Received 5.2 BCG
    from customer."
  - Requires: push notification server (can be a lightweight sidecar to ao-recorder),
    VAPID key generation, user opt-in flow.
- **Simpler alternative — audio chime:**
  - While PWA is open (even background tab), SSE connection stays alive.
  - Play an audio chime (`new Audio('chime.mp3').play()`) on payment event.
  - Works without push infrastructure. Limited: only while tab is open.

**Priority assessment:** The audio chime is a quick win (hours of work). Full Web
Push is a multi-day effort requiring server-side infrastructure. Ship the chime
first, add push later.

**Depends on:** SSE subscription (already working).

**Add to Roadmap** now.  UX notes: these notifications must be configurable for style, volume, time windows for muting, a mute now for specified duration quick silence option, and manual toggle off/on.

---

### 2.5 No multi-chain dashboard for vendors

**Current state:** VendorView operates on one chain at a time. A vendor with
multiple product lines (e.g. BCG curry goat + RMF mango futures) must manually
switch.

**What users need:** Unified view: total revenue across all chains, per-chain
breakdown, single-screen overview.

**Implementation sketch:**
- Extend VendorView with a chain selector/summary bar showing all vendor-owned
  chains.
- Top-level summary: combined revenue (in a chosen display currency or per-chain).
- Per-chain cards: symbol, today's revenue, last transaction time, SSE status.
- Tap a card to expand into current single-chain detail view.

**Depends on:** The vendor knowing which chains they own. Currently determined by
which chain's genesis key matches their wallet. With wallet sync (`walletDb.ts`),
this is discoverable from IndexedDB.

**Add to Roadmap** now.

---

## 3. Exchange Agent / Market Maker — "I convert between currencies"

### 3.1 No CAA escrow UI in the PWA

**Current state:** The full CAA atomic escrow protocol is implemented in Rust
(`ao-chain` escrow states, `ao-exchange` trade manager). But the PWA has zero
interface for proposing, accepting, monitoring, or canceling escrows. All CAA
operations require the CLI or the exchange daemon.

**What users need:** A peer-to-peer user who wants to swap BCG for CCC without
using an exchange intermediary needs a PWA interface to: propose an escrow, see
pending escrows, accept/cancel, and monitor finalization.

**Assessment:** The Phase 4 exchange-agent model (automated intermediary) handles
most end-user scenarios. Direct P2P CAA is a power-user feature. However, it's
important for decentralization — relying entirely on intermediaries creates
centralization pressure.

**Implementation sketch:**
- New `EscrowView.tsx` component (or section within ConsumerView).
- "Propose Swap" form: source chain + amount, destination chain + amount, deadline.
- Pending escrow list with status indicators (STARTED, TRANSFERRED, FINALIZED,
  CANCELED, TIMED_OUT).
- Accept/Cancel buttons for counterparties.
- Real-time status via SSE on both chains involved.
- Timeout countdown display.

**Depends on:** `ao-exchange` HTTP API (`POST /trade/request`, `GET /trade/{id}`)
which already exists.

**Add to Roadmap** now.

---

### 3.2 No spread/P&L dashboard

**Current state:** `InvestorView.tsx` shows holdings across recorders but no trade
history, spread tracking, or profit/loss calculations.

**What users need:** An exchange operator needs: current inventory per chain,
trade volume (24h/7d/30d), realized P&L, current bid/ask spreads, and alerts
when inventory is low.

**Implementation sketch:**
- Extend `InvestorView` or create a new `ExchangeDashboard.tsx`.
- Fetch trade history from `ao-exchange` API (needs a new endpoint:
  `GET /trades?from=&to=`).
- Display: trade list, volume charts, inventory levels per chain with low-stock
  warnings, spread configuration.
- P&L calculation: sum of (sell price - buy price) across completed trade pairs.

**Depends on:** New `ao-exchange` endpoint for trade history. The exchange daemon
currently tracks `PendingTrade` states but may not persist completed trade history.
Needs a `trade_history` SQLite table.

**Add to Roadmap** now.

---

### 3.3 Polling-only deposit detection

**Current state:** `ao-exchange` polls recorders at fixed intervals to detect
incoming deposits. No SSE/WebSocket subscription.

**What users need:** Faster trade settlement. Polling at 5-second intervals means
up to 5 seconds of unnecessary latency per trade leg.

**Implementation sketch:**
- Subscribe to SSE on each watched chain (`GET /chain/{id}/events`).
- On new block event: immediately check for deposits matching pending trades.
- Fall back to polling if SSE connection drops (resilience).
- Configuration: `deposit_detection = "sse"` (default) or `"polling"` (legacy).

**Depends on:** Recorder SSE endpoint (already exists and is used by the PWA).

**Add to Roadmap** now.

---

## 4. Validator (AOV) — "I verify chains for trust"

### 4.1 No Prometheus metrics

**Current state:** `ao-recorder` and `ao-validator` log structured events but
expose no Prometheus-compatible metrics endpoint. Operators can't integrate with
standard monitoring stacks (Grafana, Prometheus, Alertmanager).

**What users need:** `/metrics` endpoint exposing: blocks processed, active chains,
blob storage usage, validation pass/fail counts, request latency histograms,
SSE connection count.

**Implementation sketch:**
- Add `prometheus` crate dependency to `ao-recorder` and `ao-validator`.
- Instrument key operations: block submission (counter + histogram), blob upload
  (counter + size histogram), validation run (counter + result label), SSE
  connections (gauge).
- `GET /metrics` endpoint returning Prometheus text format.
- Example Grafana dashboard JSON in `2026/ops/` directory.

**Depends on:** Nothing. Additive change.

**Add to Roadmap** now.

---

### 4.2 Anchoring limited to local files

**Current state:** `ao-validator` writes anchor proofs as append-only JSON lines
to a local file. If the disk fails, anchor history is lost.

**What users need:** External anchoring to a tamper-evident, publicly verifiable
medium. The spec (`ValidationAndTrust.md`) mentions Bitcoin OP_RETURN and
transparency logs as future backends.

**Assessment:** Bitcoin anchoring adds a dependency on Bitcoin infrastructure and
per-anchor transaction fees (~$1-5 per anchor at current rates). Transparency logs
(Certificate Transparency-style) are cheaper but less universally trusted.

**Implementation sketch (phased):**
- **Phase 1:** Replicate anchor file to a second location (S3, SFTP, or a second
  machine). Simple, addresses the disk-failure risk.
- **Phase 2:** Periodic batch anchor to a public transparency log or Nostr relay
  (timestamped, signed, verifiable). Lower cost than per-block Bitcoin TX.
- **Phase 3:** Bitcoin OP_RETURN for high-value chains that justify the cost.
  Batch multiple chain anchors into a single Merkle root per Bitcoin TX.

**Depends on:** Anchor trait is already pluggable (`ao-validator` uses a trait
for the anchor backend). Adding a new backend is a matter of implementing the
trait.

**Add to Roadmap** now.

---

### 4.3 No credential issuance UI

**Current state:** Vendor credentials are defined in `ValidationAndTrust.md`
(W3C VC-inspired, hash-match model). `CREDENTIAL_REF` (type 38) and
`CREDENTIAL_URL` (type 39) type codes are allocated. But there's no UI for a
validator operator to issue, revoke, or manage credentials.

**What users need:** A validator (e.g. a local business association) wants to
certify that Bob's BCG chain is a legitimate business. They need a workflow:
review vendor profile → issue credential → credential appears in chain metadata →
consumer sees trust badge.

**Implementation sketch:**
- Validator admin panel (could be a separate view in the PWA or a CLI workflow).
- "Issue Credential" form: select chain, enter credential claims (business name,
  registration number, inspection date), sign with validator's key.
- Credential recorded as `CREDENTIAL_REF` separable item on the vendor's chain.
- PWA `TrustIndicator.tsx` already shows validator endorsements — extend it to
  display credential details on tap.
- Revocation: validator submits a revocation entry (new assignment with
  `CREDENTIAL_REF` marked revoked). `TrustIndicator` checks revocation status.

**Depends on:** Validator key management (validator has its own Ed25519 keypair,
already configured in `ao-validator`).

**Add to Roadmap** now.

---

## 5. System Operator — "I run the recorder infrastructure"

### 5.1 Single-recorder fragility

**Current state:** Each chain has exactly one recorder. If it goes down, the chain
is unavailable. No replication, no failover, no read replicas.

**What users need:** High availability. A curry goat vendor can't lose their
payment system because of a server reboot.

**Assessment:** Multi-recorder topology is architecturally significant — it raises
questions about consistency (which recorder is authoritative?), conflict resolution
(what if two recorders accept conflicting assignments?), and identity (how does a
client verify it's talking to a legitimate recorder?). The spec notes this as F3
(future), with `known_recorders` config as the current trust model.

**Implementation sketch (progressive):**
- **Hot standby:** A second recorder subscribes to the primary's SSE feed and
  mirrors all blocks and blobs. On primary failure, DNS or load balancer switches
  to standby. No simultaneous writes — avoids consistency issues.
- **Read replicas:** Multiple recorders serve reads (block/UTXO/blob queries).
  Writes go to a single primary. Reduces primary load.
- **Active-active (far future):** Both recorders accept writes. Requires consensus
  protocol (e.g. the primary signs a "recorder sequence number" that replicas
  validate). Significantly more complex.

**Recommendation:** Hot standby is sufficient for the target deployment scale
(small businesses, not global exchanges). It solves the availability problem
without introducing consensus complexity.

**Depends on:** Recorder SSE feed (exists). Signed recorder identity (F3, not yet
implemented — needed to prevent rogue replicas).

**Add to Roadmap** now - put future elements further down the roadmap in appropriate priority.

---

### 5.2 Installers untested on real hardware

**Current state:** `.deb` and `.msi` installer packages exist (`2026/sims/`) but
have not been tested on actual Debian or Windows machines. The 72-hour Raspberry
Pi stress test has not been run.

**What users need:** Confidence that the software installs and runs correctly on
their target hardware.

**Implementation sketch:**
- **Acceptance test checklist:** Install on fresh Debian 12, verify systemd service
  starts, create chain, submit assignment, verify persistence across reboot.
- **Pi 5 stress test:** 72 hours continuous operation with simulated traffic
  (use Sim-A scenario on loop). Monitor: memory usage, SQLite WAL size, blob
  storage growth, response latency percentiles.
- **Windows MSI test:** Install, verify service registration, test with PWA from
  local browser.
- **Document results** in a test report with hardware specs, observed metrics,
  and any issues found.

**Depends on:** Physical hardware access. Cannot be automated in CI alone.

**Add to Roadmap** not at this time.  Will be tested manually as resources become available.

---

## 6. Cooperative / Agricultural Users — "We track produce from farm to market"

### 6.1 Cooperative metadata spec exists, no UI

**Current state:** `CooperativeMetadata.md` defines structured `key:value` notation
for delivery records, sales, cost allocation, and advance payments. Uses existing
separable types (`NOTE`, `DESCRIPTION`, `DATA_BLOB`). But the PWA has no interface
for entering or viewing cooperative-specific data.

**What users need:** A cooperative manager recording: "Farmer Jane delivered 180 kg
grade-A tomatoes, lot 2026-W10-012." A consumer verifying: "These tomatoes came
from Lot 2026-W10-012, delivered 2026-03-05, graded A."

**Assessment:** This is a specialized vertical UI built on top of the general
protocol. It could be a separate PWA view (like VendorView/ConsumerView) or a
plugin/extension system. The underlying protocol already supports everything
needed — the gap is purely UI.

**Implementation sketch:**
- New `CooperativeView.tsx` (or extension of VendorView for cooperative mode).
- Forms for: delivery entry (crop, weight, grade, lot), sale recording (crop,
  weight, price), cost allocation, advance payments.
- These are recorded as `NOTE` separable items with structured `key:value` content
  per `CooperativeMetadata.md`.
- Provenance viewer: given a lot number, trace the chain of custody from farmer
  delivery to consumer sale.
- Photo attachment for weighbridge receipts and inspection photos (uses existing
  blob infrastructure + the on-chain linking from `BlobRetentionReport.md` N8a).

**Depends on:** On-chain blob linking (N8a) for photo receipts. Core NOTE-based
metadata recording works without it.

**Add to Roadmap** now.

---

## 7. Cross-Cutting Concerns

### 7.1 No push notifications (all user roles)

Covered in §2.4 above. Affects vendors (payment alerts), consumers (transfer
confirmations), and validators (alert escalation). The audio chime is a quick
win; full Web Push is a larger investment.

### 7.2 Mobile-specific UI polish

**Current state:** PWA renders responsively (React + CSS) but has not been tested
or optimized for mobile viewports, touch targets, or system font scaling.

**What users need:** Comfortable use on a 5.5" phone screen. Tap targets ≥ 44px,
readable text at default font size, no horizontal scrolling, fast load on 3G.

**Implementation sketch:**
- Mobile-first CSS audit: check all views at 375px width.
- Touch target sizing: buttons, links, form controls ≥ 44×44px.
- Performance: Lighthouse audit targeting ≥ 90 on mobile performance.
- iOS Safari-specific: safe area insets, prevent zoom on input focus.

**Depends on:** Nothing. CSS and testing work.

**Add to Roadmap** now.

---

### 7.3 LoRa mesh transport

**Current state:** Wire format is designed for low-bandwidth transports (VBC
encoding, minimal wire format). Meshtastic LoRa mesh is mentioned as a design
target. No implementation exists.

**Assessment:** LoRa support would enable AO in areas with no internet or cellular
coverage (rural farms, disaster zones, remote islands). The protocol is compact
enough — a typical assignment is under 500 bytes, well within LoRa's ~200 byte
per-message payload with fragmentation.

**Implementation sketch:**
- Meshtastic serial/BLE bridge: a lightweight daemon that reads AO messages from
  a Meshtastic radio and forwards them to a local recorder (or vice versa).
- Message types: assignment submission (outbound), block confirmation (inbound),
  balance query (request/response).
- Store-and-forward: the bridge queues messages when the radio is busy and
  delivers them in order.

**Depends on:** Meshtastic protocol library. This is a standalone component that
talks to `ao-recorder` via HTTP — no changes to the recorder itself.

**Priority:** Niche but high-impact for the target market (Caribbean islands,
rural cooperatives). Could be a community contribution or a dedicated sprint.

**Add to Roadmap** not at this time.  Note it as an item to develop as resources
become avaialble.

---

## 8. Proposed Roadmap Items — Priority Matrix

### Tier 1: Ship-blockers for pilot deployment

These must be done before handing the system to real vendors and consumers.

| ID | Item | Section | Effort |
|----|------|---------|--------|
| U1 | Wallet backup/restore UX | §1.2 | Small |
| U2 | Transfer confirmation screen | §1.3 | Small |
| U3 | Vendor profile persistence (SQLite) | §2.1 | Small |
| U4 | Offline balance cache | §1.4 | Small |
| U5 | Transaction history view | §1.1 | Medium |

### Tier 2: High-value for adoption

| ID | Item | Section | Effort |
|----|------|---------|--------|
| U6 | Audio payment chime (vendor) | §2.4 | Small |
| U7 | Printable QR signage | §2.2 | Small |
| U8 | Sales reporting + CSV export | §2.3 | Medium |
| U9 | SSE-driven deposit detection (exchange) | §3.3 | Small |
| U10 | Prometheus metrics | §4.1 | Medium |
| U11 | Mobile UI audit | §7.2 | Medium |

### Tier 3: Important for maturity

| ID | Item | Section | Effort |
|----|------|---------|--------|
| U12 | Multi-chain vendor dashboard | §2.5 | Medium |
| U13 | Web Push notifications | §2.4 | Medium |
| U14 | Refutation UI | §1.5 | Medium |
| U15 | Exchange P&L dashboard | §3.2 | Medium |
| U16 | Credential issuance UI | §4.3 | Large |
| U17 | Anchor replication (off-disk) | §4.2 | Medium |

### Tier 4: Strategic / future

| ID | Item | Section | Effort |
|----|------|---------|--------|
| U18 | Hot-standby recorder | §5.1 | Large |
| U19 | CAA escrow UI | §3.1 | Large |
| U20 | Cooperative metadata UI | §6.1 | Large |
| U21 | Fiat on/off-ramp tooling | §1.6 | Large |
| U22 | LoRa mesh bridge | §7.3 | Large |
| U23 | Hardware acceptance tests | §5.2 | Medium (hardware-bound) |

### Dependency graph (simplified)

```
U1 (backup) ──────────────────────────────────┐
U2 (confirm) ─────────────────────────────────┤
U3 (profile persist) ─────────────────────────┤
U4 (offline balance) ─────────────────────────┼── Pilot-ready
U5 (tx history) ──────────────────────────────┘
                                               │
U6 (chime) ────────────────────────────────────┤
U7 (QR print) ────────────────────────────────┤
U8 (sales report) ← U5 (tx history)          ├── Adoption
U11 (mobile audit) ───────────────────────────┘
                                               │
U13 (push) ← U6 (chime, proves concept)       │
U12 (multi-chain) ← U8 (sales report)         ├── Maturity
U18 (hot standby) ← U10 (metrics)             │
U20 (cooperative) ← N8a (blob linking)        ┘
```

---

## 9. Open Questions for Review

1. **Pilot scope:** Which user roles are in scope for the first real-world pilot?
   If vendor + consumer only, Tier 1 + U6/U7 may be sufficient. If exchange agents
   are included, U9 and U15 move to Tier 1.

   Pilot would be vendor + consumer only, but exchange agents are already live in
   the sims and should be fully developed to make the sims as realistic and
   informative as possible.

2. **Cooperative UI priority:** Is the cooperative use case (§6.1) targeted for
   the first pilot, or is it a later vertical? This significantly affects scope.

   Not first pilot, but definitely a sim target, so again: develop as if it were
   in the field and develop sims to test/exercise it as if it were in the field.

3. **Push notification infrastructure:** Are we willing to run a push notification
   server (VAPID + endpoint), or should we rely on the audio chime and SSE-while-
   open model? Push adds operational complexity but is the expected UX for mobile.

   Far down the roadmap, a push notification server could become part of the
   system, but not in early pilots, not in the sims.  SSE-while-open now, server
   later. 

4. **Hot standby vs. "just restart quickly":** For the target scale (single vendor,
   single recorder on a Pi), is hot standby overkill? A systemd auto-restart with
   SQLite WAL recovery might be "good enough" availability for a first deployment.

   Just restart quickly is a now target.  Hot standby is a later target.

5. **LoRa priority:** Is there a concrete deployment scenario (e.g. a specific
   island or rural cooperative) that justifies LoRa work now, or is it aspirational?

   LoRa is aspirational, but that should never be an excuse to bloat the protocol
   making it slower than necessary over LoRa or any other communication links.

6. **Credential trust model:** Who issues credentials in the first deployment?
   A local business association? The software developer? A government agency?
   This affects the credential UI design and the validator's authority model.

   The pilot deployments will be face-to-face trust based.  People issue their
   own credentials.  Beyond that, it will vary from one scenario to the next and
   all possible sources of trust-basis are on the table.  Do not implement anything
   specific beyond face-to-face and friends-of-friends type trust models now.

7. **CSV export format:** Should transaction/sales CSV follow any specific
   accounting standard, or is a simple date/amount/counterparty format sufficient?

   Simple for now.  Keep it modular so that standards can be implemented / applied
   as they are identified as real needs.  Identify three likely standards for
   potential implementation and be sure that the system infrastructure supports
   adding all three as options, but do not take any of them to actual implementation
   yet.
