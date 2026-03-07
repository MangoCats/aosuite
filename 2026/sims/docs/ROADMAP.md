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
| Sim-D: Auditor View & Adversarial Agents | Phase 5 | — | All deliverables |
| Sim-E: Atomic Exchange & Full Scenario | Phase 6 | — | All deliverables |

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

**Auditor/Validator agent**
- Runs ao-validator against one or more recorder instances.
- Periodically fetches blocks, verifies hash chains, publishes attestations.
- Reports integrity status, last verified height, and any anomalies to the coordinator.

**Auditor view in viewer PWA**
- When viewing Victor (or any auditor agent) in Individual User View, additional panels appear:
  - **Monitored chains:** Table of chains under audit. Columns: chain name, recorder, last verified block, last attestation time, integrity status (green/yellow/red), anchor reference.
  - **Integrity timeline:** Visual timeline showing verification checkpoints and any detected anomalies.
  - **Cross-chain overview:** Auditor's aggregate view of the ecosystem — total chains monitored, total shares across all chains, anomaly count, vendor credential status.
  - **Alert log:** Chronological alerts (chain alteration detected, validator offline, credential expired).

**Auditor perspective in Map View**
- Toggle "auditor overlay" on the map: chains with clean validator status show green halos around their recorder/vendor icons; unvalidated chains show gray; chains with detected anomalies show red pulsing indicators.
- Click a validated chain's icon → see the integrity timeline and last anchor proof.

**Adversarial agents**
- **Double-spend attacker:** Attempts to submit conflicting assignments using the same UTXO. Verifies that the recorder rejects the second attempt and that the auditor would detect it if the recorder were compromised.
- **Expired-share exploiter:** Attempts to use expired UTXOs. Verifies rejection.
- **Key-reuse attacker:** Attempts to receive shares on an already-used key. Verifies rejection.
- **Late-recording edge case agent:** Submits valid-but-late assignments, tests that refutation blocks them correctly, tests that unrefuted late recordings succeed.

Attackers log all attempts and outcomes. The viewer shows attacker agents with a distinct icon and their success/failure rates in the agent table.

**Vendor credentials in simulation**
- Vendor agents (Bob, Oscar) carry credential references (food safety certificates, guest relation courses) as separable items in their chain data.
- Consumer agents factor credential presence into vendor preference. Oscar's lack of credentials is visible in his vendor profile.
- Auditor view highlights credential status per vendor.

### Acceptance Criteria

- Auditor agent successfully validates all chains in the `island-life` scenario.
- If a recorder's database is manually tampered with (block modified), the auditor detects it within one poll interval and the alert appears in the viewer.
- All attacker agent attempts are correctly rejected by the system. Zero false acceptances.
- Auditor overlay on the map correctly reflects chain integrity status.

---

## Sim-E: Atomic Exchange and Full Scenario

**Depends on:** Base Phase 6 complete (CAA escrow protocol), Sim-D complete

**What exists at this point:** CAA conditional assignment agreements, escrow state in UTXO, coordinator recording across chains, timeout recovery.

### Deliverables

**CAA-capable agents**
- Consumer agents can initiate atomic cross-chain exchanges (e.g., "give 1 BCG to Bob, costs 12 CCC via Charlie") using the CAA escrow flow.
- Exchange agents participate in CAA as intermediaries — escrowing on both chains, completing the ouroboros recording sequence.
- Agents handle CAA failure modes: timeout, server failure during escrow, partial recording. Verify correct escrow release on failure.

**CAA visualization**
- Individual User View shows CAA state machine progress: proposed → signed → recording → binding → finalized (or → expired).
- Map View shows multi-chain transaction arcs with a distinct style (dashed lines connecting participants across chains, with escrow state annotations).
- Transaction log distinguishes CAA escrow assignments from simple assignments.

**Chaos testing scenario**
- `chaos.toml` — Extension of `island-life` with injected failures: random recorder restarts, agent crashes, network delays, and concurrent CAA + simple assignments. Verifies: no share loss, no double-spend, correct escrow release, chains remain consistent.

**Full demonstration scenario**
- `island-life-full.toml` — Complete IslandLife narrative played out over simulated weeks: Bob's genesis and Eddie's initial investment, Charlie's first purchase, Alice's cruise visit, Rita's mango season, Dave's bike rentals, Oscar's failed reputation, Ziggy's aggressive trading, Victor auditing everything, market equilibrium emergence. Designed as the canonical demonstration of the Assign Onward ecosystem.

### Acceptance Criteria

- Three-party two-chain CAA completes successfully in the simulation.
- `chaos` scenario runs for 8 hours with random failures. Post-run audit: all chain hashes valid, no share loss, no double-spend, all timed-out escrows correctly released.
- `island-life-full` scenario produces a viewable history that tells the IslandLife story through the simulation data — a new viewer can understand how the ecosystem works by watching the replay.

---

## Summary

| Sims Phase | Base Dependency | Key Deliverables | Status |
|------------|----------------|------------------|--------|
| Sim-A | Phase 2 | Agent framework, CLI agents (vendor/consumer/exchange), text observer, coordinator, minimal + three-chain scenarios | ✓ |
| Sim-B | Phase 3 | Viewer PWA, Individual User View, Community Table View, viewer API | ✓ |
| Sim-C | Phase 4 | Map View, MQTT-based exchange agents, referral fees, island-life scenario, time scrubber | ✓ |
| Sim-D | Phase 5 | Auditor agents, auditor view + map overlay, adversarial agents, vendor credentials | — |
| Sim-E | Phase 6 | CAA-capable agents, escrow visualization, chaos testing, full IslandLife scenario | — |

Each sims phase adds capability only after the base software it depends on is delivered and tested. Sims development never creates deadlines, blockers, or requirements for base 2026 work.
