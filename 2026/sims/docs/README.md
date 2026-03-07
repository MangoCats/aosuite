# Sims — Simulated Community

The **sims** module creates "simulated users" — independent software agents that play-act as users of aosuite components. Multiple instances of these agents form a simulated community, exercising the software across all community roles. The simulation is both a testing tool and a demonstration environment: it shows what a living Assign Onward economy looks and feels like, from individual user experience down to cross-chain market dynamics.

## Goals

1. **Realistic behavior.** Each agent acts independently based on its role, personality, location, and economic situation — not from a central script. Emergent behavior surfaces edge cases that scripted tests miss.
2. **Observability.** A real human viewer can inspect the simulation at any level: watch an individual user's app experience, review an auditor's cross-chain view, or see the whole community from above.
3. **Full role coverage.** The simulation includes every community role defined in aosuite and produces the full range of on-chain activity: genesis, share issuance, consumer purchases, exchange agent arbitrage, self-assignment refreshes, expiration sweeps, validation, and (eventually) multi-chain atomic exchange.

## Simulated Roles

Each agent runs as an independent process against real aosuite components (ao-recorder instances, MQTT brokers). Agents use the same APIs that real users would.

| Role | App | Agent Behavior |
|------|-----|----------------|
| **Vendor** | AOS | Issues a chain (e.g. Bob's Curry Goat). Sets prices. Accepts incoming plate/credit redemptions. Manages share float. Updates availability based on simulated schedule and inventory. |
| **Consumer** | AOE | Discovers vendors via chain info and GPS. Buys credits from exchange agents. Redeems credits at vendor locations. Browses, compares prices, occasionally churns. |
| **Exchange Agent** | AOI | Holds inventory in multiple chains. Sets bid/ask spreads. Executes automated trades. Manages risk exposure and position limits. Competes with other exchange agents on price. |
| **Recorder Operator** | AOR | Runs one or more ao-recorder instances. Handles genesis setup. Monitors server health. (Mostly infrastructure — the agent's "personality" is in uptime and fee rate choices.) |
| **Validator / Auditor** | AOV | Monitors chains for integrity. Publishes attestations. Has cross-chain visibility. Flags anomalies. |
| **Attacker** (optional) | — | Attempts double-spend, key reuse, expired-share exploits, social engineering. Validates that the system rejects bad behavior. |

### Agent Personalities

Agents are parameterized with personality profiles drawn from the [IslandLife](../../docs/html/IslandLife.html) narrative and logical extensions:

- **Bob** (vendor): Reliable, conservative pricing, high food-safety credentials, seasonal availability.
- **Oscar** (vendor): Unreliable, low quality, no credentials — tests how the system surfaces reputation.
- **Alice** (consumer): Savvy tourist, compares prices, trusts Charlie, uses AOE GPS features.
- **Charlie** (exchange agent): Professional, fair margins, wide chain coverage, referral agreements.
- **Ziggy** (exchange agent): Aggressive, thin margins, willing to deal with risky vendors.
- **Dave** (vendor + investor): Bike rentals, careful float management, also invests in other chains.
- **Eddie** (investor): Early adopter, seed investor, knows the community.
- **Victor** (auditor): Methodical, monitors multiple chains, publishes integrity reports.
- **Rita** (vendor): Seasonal mango futures — tests time-limited, option-like instruments.
- **Ted** (exchange agent): Regional intermediary, cross-island reach, referral network.

New agents can be added by writing a personality profile (JSON/TOML) without changing simulation code. The profile specifies: role, location, movement pattern, economic parameters (budget, risk tolerance, pricing strategy), activity schedule, and chain affiliations.

## Viewer Perspectives

The simulation provides three complementary ways for a human to observe what's happening.

### 1. Individual User View

Step into any simulated user's shoes. See what they see in their app:

- **Wallet state:** Current key balances across all chains the user participates in. Share counts and coin-display equivalents.
- **Transaction history:** Chronological log of every assignment the user has been party to — sent, received, fees paid. Each entry shows counterparty (if known), chain, amount, timestamp, and block height.
- **App screen:** A rendered approximation of what the AOE/AOS/AOI screen would show this user right now — nearby vendors (for consumers), incoming payments (for vendors), portfolio and open orders (for exchange agents).
- **Notifications:** Real-time SSE/WebSocket events as they arrive — new blocks, confirmations, price changes.
- **Key inventory:** All Ed25519 keys this agent has generated, which are spent, which hold live UTXOs, which are approaching expiration.

For auditor agents (Victor), this view additionally shows:
- Chains being monitored with integrity status (last verified height, any detected anomalies).
- Cross-chain hash attestation history.
- Alert log.

### 2. Community Table View

A tabular overview of all simulated users and their current state.

| Column | Description |
|--------|-------------|
| Agent | Name and role icon |
| Location | Current simulated GPS position |
| Chains | Chains this agent participates in, with balance summaries |
| Recent Activity | Last 3 transactions (condensed) |
| Status | Active / idle / offline / error |
| Net Worth | Estimated total value across all chain holdings (in a reference currency like EC$ or USD) |

Sortable and filterable by role, chain, location, activity level, or net worth. Click any row to jump to that agent's Individual User View.

Additional table views:
- **Chain table:** One row per blockchain — height, total shares outstanding, active UTXOs, fee rate, last block time, recorder identity, validator status.
- **Transaction log:** Global chronological feed of all assignments across all chains, filterable by chain, participant, amount, or time range.

### 3. Map View

A geographic visualization of the simulated community, rendered on a pannable, zoomable map.

**Zoomed out (community overview):**
- Heat map of transaction density by area.
- Chain coverage zones (which vendors serve which areas).
- Exchange agent reach overlays.
- Activity pulse — animated dots showing recent transactions.

**Zoomed in (street level):**
- Individual agent icons at their current GPS positions.
- Vendor availability indicators (open/closed, inventory level).
- Active transaction arcs — animated lines connecting participants in in-progress assignments.
- Consumer discovery radius — what a consumer at this location would see in their AOE app.

**Interactions:**
- Click an agent icon → jump to their Individual User View.
- Click a transaction arc → see the assignment details (chain, amount, participants, block).
- Click a vendor location → see their chain info, current share float, recent sales volume.
- Time slider → scrub through simulation history, watch the community evolve.

## Architecture Sketch

```
┌─────────────────────────────────────────────────────┐
│                   Viewer (React PWA)                │
│  ┌──────────┐  ┌──────────────┐  ┌───────────────┐ │
│  │ User View│  │ Table View   │  │  Map View     │ │
│  └────┬─────┘  └──────┬───────┘  └───────┬───────┘ │
│       └───────────┬────┴─────────────────┘          │
│              Viewer API (WebSocket + REST)           │
└──────────────────┬──────────────────────────────────┘
                   │
┌──────────────────┴──────────────────────────────────┐
│              Sim Coordinator                         │
│  - Spawns and manages agent processes               │
│  - Collects agent state snapshots for viewer         │
│  - Controls simulation clock (real-time or fast)     │
│  - Injects scenarios (new agents, failures, attacks) │
└──────┬───────────┬───────────┬──────────────────────┘
       │           │           │
  ┌────┴──┐   ┌────┴──┐   ┌───┴───┐
  │Agent 1│   │Agent 2│   │Agent N│   (independent processes)
  │(Bob)  │   │(Alice)│   │(Ziggy)│
  └───┬───┘   └───┬───┘   └───┬───┘
      │           │           │
      └─────────┬─┴───────────┘
                │
     Real aosuite infrastructure
  ┌─────────────┴──────────────┐
  │  ao-recorder instances     │
  │  MQTT broker               │
  │  SQLite chain databases    │
  └────────────────────────────┘
```

**Key design decisions:**

- **Agents talk to real infrastructure.** Agents use the same HTTP API and MQTT topics that real AOE/AOS/AOI clients would. The simulation doesn't mock the server — it runs actual ao-recorder instances.
- **Sim Coordinator is observer, not controller.** It spawns agents, collects their state for the viewer, and can inject events (new agent joins, server goes down), but it does not dictate agent behavior. Each agent runs its own decision loop.
- **Simulation clock.** Supports both real-time (1:1) and accelerated modes. In accelerated mode, agents compress their activity schedules. Useful for quickly generating months of simulated chain history.
- **State snapshots.** Each agent periodically reports its state (location, balances, recent transactions, app screen state) to the coordinator via a lightweight internal channel. The viewer reads from the coordinator, never directly from agents.

## Folder Structure

```
sims/
├── docs/              ← you are here
│   └── README.md
├── coordinator/       ← sim coordinator service (Rust or TS)
├── agents/            ← agent framework and role implementations
│   ├── profiles/      ← personality profiles (TOML/JSON)
│   ├── vendor.rs      ← vendor agent logic
│   ├── consumer.rs    ← consumer agent logic
│   ├── exchange.rs    ← exchange agent logic
│   ├── auditor.rs     ← validator/auditor agent logic
│   └── attacker.rs    ← adversarial agent logic
├── viewer/            ← React PWA for all three view perspectives
│   ├── user-view/     ← individual user perspective
│   ├── table-view/    ← community tables
│   └── map-view/      ← geographic visualization
└── scenarios/         ← predefined simulation setups
    ├── island-life.toml   ← full IslandLife cast
    ├── minimal.toml       ← 1 vendor + 1 consumer + 1 recorder
    └── stress.toml        ← many agents, high transaction rate
```

## Scenarios

A scenario file defines the initial community: which agents exist, their profiles, which chains to create, initial share distributions, and geographic layout.

**island-life** — The full IslandLife cast: Bob, Rita, Eddie, Alice, Charlie, Ziggy, Dave, Ted, Oscar, Victor, Gene, Faythe, Sharon, Karen. Multiple chains (BCG, RFM, CCC, DEB, ZIC, TCC). Caribbean island geography. The canonical demonstration scenario.

**minimal** — One vendor (Bob), one consumer (Alice), one exchange agent (Charlie), one recorder. Single chain. Useful for development and debugging.

**stress** — 50+ agents, 10+ chains, aggressive trading. Tests system limits and surfaces performance issues. Maps to the ROADMAP acceptance criterion of 100K assignments from genesis.

## Relationship to ROADMAP

The sims module is not part of the current Phase 2 deliverables. It can begin development alongside Phase 3 (when the React PWA exists to reference for the viewer) and grows incrementally:

| Phase | Sims Capability |
|-------|----------------|
| Phase 2 (now) | CLI-only agents using ao-cli commands against ao-recorder. Text-only observation. |
| Phase 3 | Viewer PWA with user view and table view. Consumer and vendor agents. |
| Phase 4 | Exchange agent agents. Map view. Multi-chain scenarios. |
| Phase 5 | Auditor agents with cross-chain visibility. Attacker scenarios. |
| Phase 6 | CAA escrow simulation. Full island-life scenario. |
