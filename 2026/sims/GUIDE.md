# Assign Onward Simulation Guide

Run all the AO simulations and components on your local machine. Works on Windows (PowerShell) and Linux/macOS (bash).

## Prerequisites

- **Rust** (1.85+): https://rustup.rs
- **Node.js** (20+): https://nodejs.org (for PWA and viewer)
- **Git**: for cloning the repo

## Quick Start

```bash
# Build everything (Linux/macOS/Git Bash)
./scripts/build.sh

# Run a simulation
./scripts/run-sim.sh minimal

# Open the viewer in your browser
# → http://127.0.0.1:4200
```

```powershell
# Build everything (Windows PowerShell)
.\scripts\build.ps1

# Run a simulation
.\scripts\run-sim.ps1 minimal

# Open the viewer in your browser
# → http://127.0.0.1:4200
```

## Port Allocation

### Simulations (self-contained)

Each simulation starts its own embedded recorder, viewer API, and optional MQTT broker. No external processes needed.

| Component          | Port  | Notes                                |
|--------------------|-------|--------------------------------------|
| Embedded recorder  | auto  | Port 0 = OS-assigned (most sims)     |
| Recorder A (Sim-G) | 4100  | Primary recorder for recorder-switch |
| Recorder B (Sim-G) | 4101  | Secondary recorder for migration     |
| Viewer API         | 4200  | Browser UI for all sims              |
| MQTT broker        | 1884  | Island-Life sims only                |

### Full Stack (manual testing)

The `run-stack` script starts all standalone components for manual testing with the PWA.

| Component    | Port  | URL                          |
|--------------|-------|------------------------------|
| Recorder A   | 3000  | http://127.0.0.1:3000        |
| Recorder B   | 3010  | http://127.0.0.1:3010        |
| Validator    | 4000  | http://127.0.0.1:4000        |
| Exchange     | 3100  | http://127.0.0.1:3100        |
| Relay        | 3200  | ws://127.0.0.1:3200          |
| PWA (dev)    | 5173  | http://127.0.0.1:5173        |

No port conflicts between stacks: simulations use 4100+ range, stack uses 3000+ range.

## The Simulations

### Sim-A: Minimal

**File:** `scenarios/minimal.toml` | **Duration:** 2 minutes | **Speed:** 10x

The simplest possible scenario. One vendor (Bob sells curry goat), one exchange agent (Charlie), one consumer (Alice). Demonstrates the core buy-redeem cycle: Alice buys shares from Charlie, redeems them at Bob's for a plate of curry goat.

**What to watch:** A complete transaction cycle from purchase to redemption on a single chain.

**Agents:** Bob (vendor), Charlie (exchange), Alice (consumer), Gene (recorder)

---

### Sim-B: Three-Chain

**File:** `scenarios/three-chain.toml` | **Duration:** 3 minutes | **Speed:** 10x

Three vendors competing for customers through their own exchange agents. Shows how multiple independent chains coexist on one recorder.

**What to watch:** Multiple vendors operating simultaneously, each with their own economy. Consumers choosing between vendors.

**Agents:** 3 vendors (Bob, Maria, Kwame), 3 exchanges (Charlie, Dave, Eli), 2 consumers (Alice, Fatima)

---

### Sim-C: Exchange 3-Chain

**File:** `scenarios/exchange-3chain.toml` | **Duration:** 3 minutes | **Speed:** 10x

Cross-chain trading using legacy two-leg exchanges. A consumer pays on one chain and receives shares on another, mediated by exchange agents who hold inventory on both chains.

**What to watch:** Cross-chain value transfer — how exchange agents bridge separate vendor economies.

**Agents:** 3 vendors (Bob, Carol, Maria), 2 exchanges (Charlie, Eve), 2 consumers (Alice, Dan)

---

### Sim-D: Price War

**File:** `scenarios/price-war.toml` | **Duration:** 5 minutes | **Speed:** 10x

Two exchange agents compete on price. Both start with different rates and adjust dynamically based on competitor rates and their own inventory levels. Demonstrates price discovery in a free market.

**What to watch:** Exchange rates converging as Charlie and Eve undercut each other. Watch how inventory depletion triggers rate adjustments.

**Agents:** 2 vendors (Bob, Carol), 2 competing exchanges (Charlie, Eve), 2 consumers (Alice, Dan)

---

### Sim-E: Atomic Exchange

**File:** `scenarios/atomic-exchange.toml` | **Duration:** 3 minutes | **Speed:** 10x

CAA (Conditional Assignment Agreement) atomic cross-chain swaps. Both sides of a trade complete atomically — if one side fails, neither side commits. This is the cryptographic escrow mechanism that makes trustless cross-chain trading possible.

**What to watch:** Atomic swaps completing (both chains update together) vs legacy trades. The validator confirms chain integrity throughout.

**Agents:** 2 vendors (Bob, Carol), 1 atomic exchange (Charlie), 2 atomic consumers (Alice, Dan), 1 validator (Victor)

---

### Sim-F: Island Life

**File:** `scenarios/island-life.toml` | **Duration:** 5 minutes | **Speed:** 10x

A realistic beach economy on Anguilla. Four vendors, three competing exchange agents, five consumers — all positioned on real geographic coordinates. This is the flagship simulation that shows what a real AO deployment looks like.

**What to watch:**
- Ziggy undercuts Charlie's exchange rates on BCG
- Transaction arcs on the map show commerce flowing across Sandy Ground
- Click any agent for wallet details, inventory, and trading rates
- Ted operates from Tortola as a regional exchange with different dynamics

**Agents:** 4 vendors (Bob, Rita, Dave, Oscar) + 3 payment issuers (CCC, ZIC, TCC), 3 exchanges (Charlie, Ziggy, Ted), 5 consumers (Alice, Eddie, Karen, Luke, Mona), MQTT broker for block notifications

---

### Sim-F2: Island Life Full

**File:** `scenarios/island-life-full.toml` | **Duration:** 5 minutes | **Speed:** 10x

The complete Island Life scenario with security testing. Mixes CAA atomic swaps with legacy two-leg trades, adds a validator monitoring chain integrity, and includes an attacker (Mallory) attempting double-spends.

**What to watch:**
- Purple solid arcs = atomic CAA swaps, dashed arcs = legacy trades
- Validator green/red halos show chain integrity status
- Mallory's double-spend attacks get rejected
- Legitimate trading continues unaffected

**Agents:** Everything from Island Life + validator (Victor) + attacker (Mallory)

---

### Sim-G: Audit Adversarial

**File:** `scenarios/audit-adversarial.toml` | **Duration:** 3 minutes | **Speed:** 10x

Security stress test. Five different attack types run simultaneously against the protocol while legitimate commerce continues. The validator detects all attacks and fires alerts.

**Attack types tested:**
1. **Double-spend** (Mallory) — tries to spend the same shares twice
2. **Key reuse** (Trudy) — reuses a one-time public key
3. **Expired UTXO** (Oscar) — tries to spend expired shares
4. **Chain tampering** (Eve) — directly modifies the database
5. All attacks rejected; validator detects Eve's DB tampering via rolled-hash mismatch

**What to watch:** All attack attempts are rejected. Alice's legitimate transactions complete normally. The validator's alert panel shows detections.

**Agents:** 2 vendors, 1 exchange, 1 consumer (Alice), 1 validator (Victor), 4 attackers

---

### Sim-H: Infrastructure Resilience

**File:** `scenarios/infra-resilience.toml` | **Duration:** 2 minutes | **Speed:** 1x

White-hat security testing of N10 server hardening features. Five probe agents verify that the recorder correctly enforces rate limits, payload size guards, API key authentication, connection limits, and error sanitization.

**Expected results:**
- FloodProbe: sees 429 (rate limited) after threshold
- PayloadProbe: gets 413 (payload too large), recorder doesn't crash
- AuthProbe: gets 401 for missing/invalid API keys
- ConnProbe: gets 503 when exceeding max connections
- ErrorProbe: gets generic errors, no internal details leaked

**What to watch:** Each probe's status panel showing expected HTTP response codes.

**Agents:** 1 vendor (Bob), 5 infra testers (FloodProbe, PayloadProbe, AuthProbe, ConnProbe, ErrorProbe)

---

### Sim-G2: Recorder Switch

**File:** `scenarios/recorder-switch.toml` | **Duration:** 3 minutes | **Speed:** 10x

Demonstrates the full lifecycle of moving a chain between recorders (TA3 features). Uses two embedded recorders. A recorder operator (Helen) performs owner key rotation, initiates a recorder switch, and migrates the chain to a new recorder — all while trading continues.

**Timeline:**
- **0–30s:** Normal trading on Recorder A (port 4100)
- **~30s:** Helen rotates Bob's chain owner key
- **~60s:** Recorder switch initiated (RECORDER_CHANGE_PENDING)
- **~120s:** Chain migration — old chain frozen, new chain created on Recorder B (port 4101)

**What to watch:** Owner key rotation, recorder switch phase progression, chain migration completing, trading continuing uninterrupted through all operations.

**Agents:** 2 recorders (Gene, Faythe), 1 vendor (Bob), 1 exchange (Charlie), 1 consumer (Alice), 1 validator (Victor), 1 recorder operator (Helen)

## Running Simulations

### Single simulation

```bash
# Linux/macOS
./scripts/run-sim.sh island-life

# Windows
.\scripts\run-sim.ps1 island-life
```

Open http://127.0.0.1:4200 in your browser to see the viewer with map, agent panels, and transaction arcs.

### All simulations sequentially

```bash
./scripts/run-sim.sh all        # Linux
.\scripts\run-sim.ps1 all       # Windows
```

Runs every scenario one after another with a 3-second pause between each.

### Custom viewer port

```bash
./scripts/run-sim.sh minimal --viewer-port 8080
.\scripts\run-sim.ps1 minimal -ViewerPort 8080
```

## Running the Full Stack

For manual testing with the PWA browser app:

```bash
# Linux/macOS
./scripts/run-stack.sh

# Windows
.\scripts\run-stack.ps1
```

This starts two recorders, a validator, a relay, and the PWA dev server. Open http://127.0.0.1:5173 for the PWA.

### Options

```bash
./scripts/run-stack.sh --data-dir /my/data    # custom data directory
./scripts/run-stack.sh --no-pwa               # skip PWA dev server
```

```powershell
.\scripts\run-stack.ps1 -DataDir C:\my\data
.\scripts\run-stack.ps1 -NoPwa
```

### Using the stack

1. Open the PWA at http://127.0.0.1:5173
2. Set recorder URL to `http://127.0.0.1:3000` in settings
3. Create a chain (vendor view) or browse chains (consumer view)
4. Use `ao-cli` for command-line operations:

```bash
# Generate a keypair
ao-cli keygen

# Check recorder health
curl http://127.0.0.1:3000/health

# View recorder dashboard
open http://127.0.0.1:3000/dashboard
```

### Reproducible seeds

Set environment variables for deterministic recorder identities:

```bash
export AO_SEED_A="<64-char-hex>"
export AO_SEED_B="<64-char-hex>"
./scripts/run-stack.sh
```

## Viewer Controls

While a simulation is running, the viewer at http://127.0.0.1:4200 provides:

- **Scenario panel** — title, description, "what to watch" tips
- **Agent list** — click any agent for wallet, inventory, rates
- **Map** (geographic sims) — agent positions, transaction arcs
- **Speed control** — pause, 1x, 2x, 10x via POST /api/speed
- **Agent pause/resume** — pause individual agents via the viewer

### Viewer API

```
GET  /api/scenario               Scenario metadata
GET  /api/agents                 All agent states
GET  /api/agents/{name}          Single agent detail
GET  /api/chains                 Chain summaries
GET  /api/transactions           Recent transactions
POST /api/speed  {"speed": 5.0}  Change simulation speed
POST /api/agents/{name}/pause    Pause an agent
POST /api/agents/{name}/resume   Resume a paused agent
GET  /api/ws                     WebSocket for real-time updates
```

## Troubleshooting

**Port already in use:** Kill the process using the port, or use a custom viewer port:
```bash
./scripts/run-sim.sh minimal --viewer-port 4201
```

**Build fails:** Ensure Rust 1.85+ (`rustup update`). The `edition = "2024"` crates require a recent nightly or stable.

**Simulation exits immediately:** Check the terminal output for error messages. Common cause: missing scenario TOML file.

**Viewer shows no data:** The viewer API starts before agents. Wait a few seconds for agents to initialize and begin trading.
