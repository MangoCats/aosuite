# Sims Development Roadmap

The sims module is a consumer of the base 2026 project, never a dependency of it. Each sims phase begins only after the base phase it depends on has delivered working, tested software. Base development proceeds on its own schedule without awareness of sims.

## Dependency Principle

```
Base 2026 ROADMAP (independent, unchanged)
  Phase 0 ──► Phase 1 ──► Phase 2 ──► Phase 3 ──► Phase 4 ──► Phase 5 ──► Phase 6

Sims ROADMAP (follows, never blocks)
                           Sim-A ──────► Sim-B ──────► Sim-C ──────► Sim-D ──► Sim-E
                           needs P2       needs P3      needs P4      needs P5   needs P6
```

No sims work creates requirements on base phases. If a base phase is delayed, the corresponding sims phase simply waits.

## Phase Overview

| Sims Phase | Base Dependency | Status | Outstanding |
|------------|----------------|--------|-------------|
| Sim-A: CLI Agents & Text Observer | Phase 2 | ✓ | Scenario injection (add/remove agents mid-sim); formal 1-hr/4-hr acceptance runs |
| Sim-B: Viewer PWA | Phase 3 | ✓ | — |
| Sim-C: Map View & Exchange Agents | Phase 4 | ✓ | Market equilibrium test scenario (formal convergence verification) |
| Sim-D: Auditor View & Adversarial Agents | Phase 5 | ✓ | Vendor credentials (needs separable items); late-recording attacker (complex window semantics); integrity timeline visualization |
| Sim-E: Atomic Exchange & Full Scenario | Phase 6 | ✓ | Chaos testing with recorder restarts; CAA state machine visualization in Individual User View |
| Sim-F: Onboarding Layer | — | — | All deliverables |
| Sim-G: Recorder Competition & Chain Migration | N34–N35 | — | All deliverables |

---

## Sim-A: CLI Agents and Text Observer

**Depends on:** Base Phase 2 complete (ao-recorder running, ao-cli functional)

**What exists at this point:** ao-recorder HTTP API, ao-cli commands (keygen, genesis, balance, assign, accept, refute, history, export), SQLite UTXO store, SSE/WebSocket block notifications.

### Deliverables

**Agent framework** ✓
- Agent process launcher — spawns independent agent processes from a scenario file. ✓
- Agent trait/interface: `decide()` loop driven by role logic, `report_state()` for observer. ✓
- Simulated clock with real-time (1:1) and accelerated modes. In accelerated mode, agents compress wait times but transaction ordering remains realistic. ✓
- Personality profile loader (TOML). Profile specifies: role, name, location (lat/lon), economic parameters, activity schedule, chain affiliations. ✓

**CLI-based agents** ✓
- **Vendor agent:** Calls `ao genesis` to create a chain. Periodically checks balance. Accepts incoming assignments (monitors SSE for redemptions). Simulates availability schedule (open hours, days off). Parameterized by: chain name, coin label, pricing, float cap, schedule. ✓
- **Consumer agent:** Discovers vendor chains (configured list or coordinator query). Calls `ao assign` + `ao accept` to purchase from exchange agents, then redeems at vendors. Parameterized by: budget, purchase frequency, vendor preferences, location. ✓
- **Exchange agent:** Holds positions in multiple chains. Posts bid/ask by responding to incoming assignment proposals. Executes two-leg trades (receive on chain A, send on chain B). Parameterized by: chains traded, spread, position limits, risk tolerance. ✓
- **Recorder operator agent:** Minimal — starts ao-recorder instances, monitors health. May operate multiple chains. ✓

All agents interact with ao-recorder over HTTP, exactly as real CLI users would. No internal shortcuts.

**Sim coordinator** ✓
- Reads scenario file, spawns agents, collects periodic state reports. ✓
- Text-mode observer: prints a live-updating terminal dashboard (agent table, recent transactions, chain summaries). Suitable for headless/CI environments. ✓
- Scenario injection: pause/resume agents via API. ✓ Add/remove agents mid-simulation, trigger server restarts, simulate agent downtime.

**Scenarios** ✓
- `minimal.toml` — 1 vendor (Bob), 1 consumer (Alice), 1 exchange agent (Charlie), 1 recorder (Gene). Single chain (BCG). Basic buy-redeem loop. ✓
- `three-chain.toml` — Bob (BCG), Rita (RFM), Dave (DEB), Charlie + Ziggy as competing exchange agents, 3 consumers, 2 recorders. Tests multi-chain trading and exchange competition. ✓

### Acceptance Criteria

- `minimal` scenario runs unattended for 1 hour, producing a chain with 50+ blocks, no errors.
- `three-chain` scenario runs for 4 hours, all three chains grow, exchange agents maintain positions, consumer agents complete cross-chain purchases.
- Text observer displays live state. Agent logs are reviewable per-agent.

---

## Sim-B: Viewer PWA — User View and Table View

**Depends on:** Base Phase 3 complete (React PWA with AOE + AOS views exist as reference), Sim-A complete

**What exists at this point:** Browser key management, AOE consumer UI, AOS vendor UI, SSE real-time updates, GPS vendor map in AOE.

### Deliverables

**Viewer API** (Rust, WebSocket + REST) ✓
- Serves agent state snapshots from the coordinator to the viewer PWA. ✓
- Endpoints: list agents, get agent detail, list chains, global transaction feed, agent transaction history. ✓
- WebSocket push for real-time updates (new transactions, agent state changes). ✓

**Individual User View** (React) ✓
- Select any agent from a list → see their perspective. ✓
- **Wallet panel:** Balances across all chains, share counts and coin-display equivalents, approaching-expiration warnings. ✓
- **App screen panel:** Rendered approximation of what this agent's AOE/AOS/AOI would show. For consumers: coin balances and last purchase. For vendors: chain info, balance, redemptions. For exchange agents: inventory bars, trading rates. ✓
- **Transaction history panel:** Scrollable, filterable log of every assignment this agent participated in. Each row: timestamp, chain, direction (sent/received), counterparty name, share amount, coin equivalent, fee paid, block height. Click a transaction to see full assignment details. ✓
- **Key inventory panel:** Per-chain key summary: total/unspent/spent keys, unspent amount in coins, oldest unspent key age with stale-key warning. ✓

**Community Table View** (React) ✓
- **Agent table:** All simulated users. Columns: name, role, location, status (active/idle/offline), chains participated in, net worth estimate. Sortable, filterable. Click row → Individual User View. ✓
- **Chain table:** All blockchains. Columns: name, chain ID (truncated), recorder, height, total shares, active UTXOs, fee rate, last block time, validator status. Click row → chain detail panel with block list. ✓
- **Transaction log:** Global feed, all chains. Columns: timestamp, chain, from → to, amount, fee, block. Filterable by chain, agent, time range, amount range. ✓ (amount/time-range filtering deferred)

### Acceptance Criteria

- Viewer connects to a running Sim-A simulation and displays live data.
- User View accurately reflects an agent's wallet state — balances match `ao balance` CLI output for the same keys.
- Table View updates within 2 seconds of a new block being recorded.
- All three table views (agents, chains, transactions) support sorting and filtering.

---

## Sim-C: Map View and Exchange Agents

**Depends on:** Base Phase 4 complete (AOI view, exchange infrastructure, MQTT, automated trading), Sim-B complete

**What exists at this point:** AOI automated trading UI, exchange listing as separable items, MQTT block notifications, referral fee structures, two-party exchange flow.

### Deliverables

**Map View** (React, Leaflet) ✓
- Pannable, zoomable map rendering agent positions on simulated geography. ✓
- **Zoomed out:** Transaction heat map by area. ✓ Chain coverage zones (colored overlays showing where each vendor's credits are redeemable). ✓ Exchange agent reach lines. Activity pulse animation (dots at transaction locations, fading over time).
- **Zoomed in:** Individual agent icons with role-colored markers. ✓ Vendor open/closed indicator and inventory level. Consumer discovery radius circle. Active transaction arcs (dashed lines between participants with fade-over-time opacity). ✓
- **Interactions:** Click agent icon → Individual User View. ✓ Click transaction arc → assignment detail popup. ✓ Click vendor → chain info + recent sales panel. Hover agent → tooltip with name, role, key stats. ✓
- **Time controls:** Play/pause ✓, speed slider (1x to 100x) ✓, scrub bar over simulation history ✓. Map state replays from agent state snapshots. ✓

**Enhanced exchange agent simulation** ✓
- Agents now use MQTT subscriptions for block notifications (matching base Phase 4 infrastructure). ✓
- Referral fee logic: exchange agents negotiate referral agreements with each other per scenario config. Prices shown to consumers reflect referral chain costs (matching IslandLife economics: Charlie's plates at $12, Ziggy's at $11.40, cross-referral markups). ✓
- Position rebalancing: exchange agents automatically adjust holdings when inventory gets lopsided. ✓
- Competing exchange agent price discovery: multiple agents trading the same chain adjust rates dynamically based on competitor pricing and inventory levels. ✓ `price-war.toml` scenario demonstrates two exchanges competing on BCG↔CCC pair.

**Expanded scenarios** ✓
- `island-life.toml` — Full IslandLife cast on Caribbean island geography: Bob (BCG), Rita (RFM), Dave (DEB), Oscar (OGP), Charlie (CCC), Ziggy (ZIC), Ted (TCC), Eddie, Alice + 5 additional consumers, Gene + Faythe as recorders. Locations match the narrative (Sandy Ground, George Hill, Blowing Point, the Valley). ✓
- `price-war.toml` — 2 competing exchanges (Charlie, Eve) with overlapping BCG↔CCC pair, both with `price_discovery = true`. Tests dynamic rate adjustment under competition. ✓
- `exchange-3chain.toml` — 3 vendors (BCG, CCC, MFF), 2 exchange agents, 2 consumers doing cross-chain trades across 3 chains. ✓
- Market equilibrium test: 5 exchange agents, 3 chains, verify spread convergence within 200 transactions.

### Acceptance Criteria

- Map view renders all agents at correct positions. Pan and zoom are smooth with 20+ agents.
- Clicking any agent on the map navigates to their Individual User View and back.
- Time scrubber replays 4 hours of simulation history without lag.
- `island-life` scenario produces realistic market dynamics: BCG price settles near the IslandLife narrative range, Ziggy undercuts Charlie, Oscar's chain is thinly traded.
- Exchange agents use MQTT, not polling, for block notifications.

---

## Sim-D: Auditor View and Adversarial Agents

**Depends on:** Base Phase 5 complete (AOV validator, anchor proofs, vendor credentials), Sim-C complete

**What exists at this point:** ao-validator monitoring, chain integrity API, rolled-up hash anchoring, vendor credential references as separable items.

### Deliverables

**Auditor/Validator agent** ✓
- Runs `ao-validator::verify_block_batch()` against the embedded recorder. ✓
- Periodically fetches blocks in configurable batches, verifies hash chains via rolled hash accumulation. ✓
- Auto-discovers chains from recorder, re-discovers periodically. ✓
- Reports integrity status, last verified height, and any anomalies to the coordinator via `ViewerEvent::State`. ✓

**Auditor view in viewer PWA** ✓
- When viewing Victor (or any auditor agent) in Individual User View, additional panels appear: ✓
  - **Monitored chains:** Table of chains under audit with progress bars. Columns: chain symbol, validated height vs chain height, integrity status (green/red). ✓
  - **Integrity timeline:** Visual timeline showing verification checkpoints and any detected anomalies.
  - **Summary stats:** Total chains monitored, blocks verified, alert count. ✓
  - **Alert log:** Chronological alerts (chain alteration detected, verification failure, network errors). ✓

**Auditor perspective in Map View** ✓
- Toggle "Audit" overlay on the map: vendors with clean validator status show green halos; chains with detected anomalies show red dashed halos; unmonitored chains show gray. ✓
- Click a validated chain's icon → see the integrity timeline and last anchor proof.

**Adversarial agents** ✓
- **Double-spend attacker:** Attempts to submit conflicting assignments using the same UTXO. Verifies that the recorder rejects the second attempt. ✓
- **Key-reuse attacker:** Attempts to receive shares on an already-used key. Verifies rejection. ✓
- **Expired-UTXO attacker:** Builds assignment referencing a UTXO with expired timestamp. Verifies rejection. ✓ (Health-check style — confirms recorder enforces expiry rules.)
- **Late-recording edge case agent:** Submits valid-but-late assignments, tests that refutation blocks them correctly, tests that unrefuted late recordings succeed. (Deferred — complex window semantics.)

Attackers log all attempts and outcomes. ✓ The viewer shows attacker agents with distinct red styling and their success/failure rates in the agent table and Individual User View. ✓

**Vendor credentials in simulation** (Deferred — needs separable item infrastructure not yet in sims.)
- Vendor agents (Bob, Oscar) carry credential references (food safety certificates, guest relation courses) as separable items in their chain data.
- Consumer agents factor credential presence into vendor preference. Oscar's lack of credentials is visible in his vendor profile.
- Auditor view highlights credential status per vendor.

**Scenarios** ✓
- `audit-adversarial.toml` — 10 agents: Bob (BCG vendor), Carol (CCC vendor), Gene (recorder), Charlie (exchange), Alice (consumer), Victor (validator, poll_interval_secs=5), Mallory (double_spend attacker targeting Bob), Trudy (key_reuse attacker targeting Bob), Oscar (expired_utxo attacker targeting Carol), Eve (chain_tamper attacker targeting Bob). 180s duration, 10x speed. ✓

### Acceptance Criteria

- Auditor agent successfully validates all chains in the `audit-adversarial` scenario. ✓
- If a recorder's database is manually tampered with (block modified), the auditor detects it within one poll interval and the alert appears in the viewer. ✓ (Eve `chain_tamper` agent flips a byte in stored block data; Victor detects hash mismatch and raises "alteration" alert.)
- All attacker agent attempts are correctly rejected by the system. Zero false acceptances. ✓
- Auditor overlay on the map correctly reflects chain integrity status. ✓

---

## Sim-E: Atomic Exchange and Full Scenario

**Depends on:** Base Phase 6 complete (CAA escrow protocol), Sim-D complete

**What exists at this point:** CAA conditional assignment agreements, escrow state in UTXO, coordinator recording across chains, timeout recovery.

### Deliverables

**CAA-capable agents** ✓
- Consumer agents can initiate atomic cross-chain exchanges (e.g., "give 1 BCG to Bob, costs 12 CCC via Charlie") using the CAA escrow flow. ✓
- Exchange agents participate in CAA as intermediaries — escrowing on both chains, completing the ouroboros recording sequence via `ao_exchange::caa::execute_caa()`. ✓
- Consumer `atomic = true` config triggers `AtomicBuy` message flow instead of legacy two-leg `CrossChainBuy`. ✓
- Exchange agents track CAA metrics (total/successful/failed) via `CaaExchangeStatus`. ✓
- Agents handle CAA failure modes: timeout, server failure during escrow, partial recording. Verify correct escrow release on failure. (Timeout handled via short `escrow_secs`; recorder restart chaos deferred.)

**CAA visualization** ✓
- Individual User View shows CAA state machine progress: proposed → signed → recording → binding → finalized (or → expired). (Deferred — CAA status panel shows aggregate counts, not per-CAA state machine.)
- Map View shows CAA transaction arcs with distinct style (solid purple lines, thicker weight vs dashed normal arcs). ✓
- Transaction log distinguishes CAA from simple assignments via "CAA atomic" description. ✓
- Exchange agent view shows CAA section with total/successful/failed counts and status badge. ✓

**Chaos testing scenario**
- `chaos.toml` — Deferred. Recorder restarts require stopping/restarting embedded Axum server, complex infrastructure for marginal test value. Agent-level chaos (random pauses) already supported via pause flags.

**Full demonstration scenario** ✓
- `atomic-exchange.toml` — 2 vendors (BCG, CCC), 1 exchange agent (Charlie) with `atomic = true`, 2 consumers (Alice, Dan) both using CAA atomic swaps in opposite directions, 1 validator. 180s at 10x speed. ✓
- `island-life-full.toml` — Full IslandLife cast with CAA: Bob, Rita, Dave, Oscar as vendors; Charlie + Ziggy as atomic exchanges, Ted as legacy; Alice + Eddie atomic, Karen + Mona legacy, Luke atomic via Ziggy; Victor validator, Mallory attacker. 300s at 10x speed. ✓

**Pre-genesis architecture** ✓
- Genesis items pre-generated in `main.rs` before recorder startup, enabling `known_recorders` CAA configuration. ✓
- Vendor agents receive pre-built `(genesis_json, issuer_seed)` instead of generating genesis internally. ✓

### Acceptance Criteria

- Three-party two-chain CAA completes successfully in the simulation. ✓ (`atomic-exchange.toml`)
- `chaos` scenario runs for 8 hours with random failures. (Deferred — see above.)
- `island-life-full` scenario produces a viewable history that tells the IslandLife story through the simulation data — a new viewer can understand how the ecosystem works by watching the replay. ✓

---

## Sim-F: Onboarding Layer — Visitor-Friendly Presentation

**Depends on:** Sim-E complete (all agent types, scenarios, and viewer features exist)

**Why this phase exists:** The promo articles promise readers "watch twelve agents trade across seven chains on a map of Anguilla" and invite them to click a link to see a running simulation. The current viewer is an excellent operator dashboard but provides zero context for a first-time visitor. A reader arriving from an article encounters raw tables, unexplained acronyms (UTXO, AOS, AOI, CAA), and no narrative connecting the dots on the map to the story in the article. This phase adds a presentation layer — not a rewrite, but onboarding context that makes the existing viewer self-explanatory for casual visitors and usable as an elevator-pitch mini-demo.

**Design constraint:** All onboarding content is scenario-driven. Each TOML file carries its own title, description, and agent blurbs. The viewer reads this metadata from the API and renders it. No hardcoded scenario-specific text in the viewer code.

### Deliverables

**Scenario metadata in TOML + API**
- New optional fields in `[simulation]`: `title` (display name), `description` (1-3 sentence overview for visitors), `what_to_watch` (bulleted guidance).
- New optional field on each `[[agent]]`: `blurb` (one-sentence human-readable description for tooltips and welcome panel).
- New `GET /api/scenario` endpoint returning `{ name, title, description, what_to_watch, agents: [{ name, role, blurb }] }`.
- Existing scenarios updated with narrative metadata: `island-life.toml`, `island-life-full.toml`, `audit-adversarial.toml`, `atomic-exchange.toml`.

**Welcome overlay**
- On first page load, a dismissible overlay appears over the map:
  - Scenario title and description from API.
  - "What to watch" bullet list.
  - Agent roster: name, role badge, blurb for each agent.
  - Color legend: vendor (green), exchange (orange), consumer (blue), validator (purple), attacker (red).
  - "Got it" button dismisses. Overlay does not reappear until page reload.
- A small "?" button in the header re-opens the overlay at any time.

**Map-first default**
- Default tab changes from "Agents" to "Map". Visitors see geography first, not a data table.
- Map shows persistent agent name labels (not just hover tooltips). Labels positioned above markers, small font, no overlap logic needed at Anguilla zoom level.

**Map legend**
- Small collapsible legend panel in bottom-left corner of map: role colors, arc styles (dashed = normal, solid purple = CAA atomic), overlay button descriptions.

**Narrative transaction toasts**
- When a transaction occurs, a brief toast notification appears above the map (stacked, max 3 visible, auto-dismiss after 5 seconds).
- Toast text is human-readable: uses agent blurbs and scenario context to generate messages like "Alice bought BCG from Charlie (12 CCC)" rather than raw "assignment recorded on chain abc123..."
- Toasts are suppressible via a "Mute" toggle for power users who find them distracting.

### Acceptance Criteria

- A person who has never seen the project can open the viewer, read the welcome overlay, and understand what they're watching within 30 seconds.
- The welcome overlay correctly renders scenario-specific content from any TOML file with metadata.
- Map tab is the default landing view with visible agent labels.
- Transaction toasts provide enough narrative context that a visitor can follow the economic activity without clicking into agent detail views.
- All existing scenarios still work without metadata fields (graceful fallback to current behavior — no overlay if no description, no labels if no blurbs).
- The viewer remains fully functional as an operator tool — all existing tables, panels, and features are unchanged.

---

## Sim-G: Recorder Competition & Chain Migration

**Depends on:** N34–N35 complete (TⒶ³ recorder competition + PWA integration), Sim-E complete

**What exists at this point:** Owner key rotation/revocation/override (ao-chain), recorder switch flow (RECORDER_CHANGE_PENDING → CAA drain → RECORDER_CHANGE), chain migration with three tiers (Full/Surrogate/Social), reward rate changes, recorder URL changes, all acceptance tests A–P passing. PWA components for RecorderSwitch, OwnerKeyManager, ChainMigrationBanner, RecorderIdentity.

### Deliverables

**Dual-recorder infrastructure**
- Sim coordinator starts two embedded ao-recorder instances (Recorder A and Recorder B) when `secondary_recorder_port` is configured.
- Pre-genesis chains are created on Recorder A. After a recorder switch, agents redirect traffic to Recorder B.
- Both recorders share the same `known_recorders` map for CAA proof verification.

**Recorder operator agent** (`recorder_operator` role)
- New agent type that manages TⒶ³ chain infrastructure operations on a timed schedule.
- **Owner key rotation**: At `rotate_after_secs`, builds and submits `OWNER_KEY_ROTATION` (type 128) for the target chain. New key generated, signed by current owner key.
- **Recorder switch**: At `switch_after_secs`, builds and submits `RECORDER_CHANGE_PENDING` (type 130) pointing to Recorder B's pubkey and URL. Monitors chain info until the recorder auto-constructs `RECORDER_CHANGE` (type 131) after CAA drain. Reports phase transitions (pending → draining → complete) to the observer.
- **Chain migration**: At `migrate_after_secs`, builds a new genesis on Recorder B, then submits `CHAIN_MIGRATION` (type 133) with `CHAIN_REF` to the new chain. Old chain is frozen. Reports frozen status.
- All operations report state to the viewer via `ViewerEvent::State` with a new `recorder_op_status` field showing completed operations and current phase.

**Transfer builders**
- `build_owner_key_rotation()`: Constructs signed `OWNER_KEY_ROTATION` DataItem.
- `build_recorder_change_pending()`: Constructs signed `RECORDER_CHANGE_PENDING` with new recorder pubkey + URL.
- `build_chain_migration()`: Constructs signed `CHAIN_MIGRATION` with `CHAIN_REF` child.

**Scenario**
- `recorder-switch.toml` — 1 vendor (Bob, BCG), 1 consumer (Alice), 1 exchange (Charlie), 1 validator (Victor), 1 recorder operator (Helen). Two recorders (Gene on port 4100, secondary on 4101). Timeline: normal trading for 30s → owner key rotation at 30s → recorder switch initiated at 60s → CAA drain + auto-change → trading resumes on new recorder → chain migration at 120s → chain frozen. 180s at 10x speed.

### Acceptance Criteria

- Owner key rotation completes and subsequent assignments are validated against the new key set.
- Recorder switch completes: PENDING recorded, CAA drain observed (or skipped if none active), CHANGE auto-constructed, chain accessible on new recorder.
- Chain migration freezes the old chain — further assignment submissions are rejected.
- Normal trading (consumer purchases, exchange trades) continues uninterrupted through the recorder switch.
- Validator agent detects no integrity issues throughout the scenario.
- All agent state transitions are visible in the viewer observer output.

---

## Summary

| Sims Phase | Base Dependency | Key Deliverables | Status |
|------------|----------------|------------------|--------|
| Sim-A | Phase 2 | Agent framework, CLI agents (vendor/consumer/exchange), text observer, coordinator, minimal + three-chain scenarios | ✓ |
| Sim-B | Phase 3 | Viewer PWA, Individual User View, Community Table View, viewer API | ✓ |
| Sim-C | Phase 4 | Map View, MQTT-based exchange agents, referral fees, island-life scenario, time scrubber | ✓ |
| Sim-D | Phase 5 | Validator agent, validator view + audit map overlay, adversarial agents (double-spend, key-reuse, expired-UTXO), audit-adversarial scenario | ✓ |
| Sim-E | Phase 6 | CAA-capable agents, CAA visualization, atomic-exchange + island-life-full scenarios, pre-genesis architecture | ✓ |
| Sim-F | — | Onboarding layer: welcome overlay, map labels, legend, narrative toasts, scenario metadata API | — |
| Sim-G | N34–N35 | Recorder operator agent, dual-recorder infrastructure, owner key rotation + recorder switch + chain migration in simulation, recorder-switch scenario | — |

Each sims phase adds capability only after the base software it depends on is delivered and tested. Sims development never creates deadlines, blockers, or requirements for base 2026 work.
