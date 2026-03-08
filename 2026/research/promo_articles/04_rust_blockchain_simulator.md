# Building a Blockchain Simulator in Rust

*Target audience: Rust developers, simulation enthusiasts, distributed systems engineers (r/rust, Hacker News, RustConf/EuroRust talk proposal). Technical deep-dive on architecture, agent framework, and viewer.*

---

We needed to test a blockchain protocol without deploying it. Not unit tests -- those existed. We needed to watch twelve independent agents trade across seven blockchains in real time, with competing exchange agents discovering market prices, adversarial agents trying to cheat, and a validator catching them. We needed a map, time controls, and the ability to click any agent and see their wallet.

This is how we built it in Rust.

## The Protocol in 30 Seconds

Assign Onward is a federated microblockchain protocol. Instead of one shared chain, every business runs its own. Bob's Curry Goat chain, Rita's Mango Futures chain, Dave's E-Bike Rentals chain. Exchange agents bridge between chains. A recorder hosts chains on commodity hardware. The protocol is ~16,000 lines of Rust across seven crates (`ao-types`, `ao-crypto`, `ao-chain`, `ao-recorder`, `ao-cli`, `ao-exchange`, `ao-validator`), with 187 tests.

The simulator needed to exercise all of this -- not by mocking the protocol, but by running the real recorder and having agents interact with it over HTTP, exactly as real users would.

## Architecture

```
                    ┌─────────────┐
                    │  Scenario   │
                    │  TOML file  │
                    └──────┬──────┘
                           │ parse
                    ┌──────▼──────┐
                    │ Coordinator │
                    │  (main.rs)  │
                    └──┬───┬───┬──┘
                       │   │   │  spawn
              ┌────────┘   │   └────────┐
              ▼            ▼            ▼
         ┌─────────┐ ┌─────────┐ ┌──────────┐
         │ Vendor   │ │Consumer │ │ Exchange │ ...
         │ Agent    │ │ Agent   │ │  Agent   │
         └────┬─────┘ └────┬────┘ └────┬─────┘
              │            │           │
              │   HTTP     │   HTTP    │   HTTP + MQTT
              ▼            ▼           ▼
         ┌──────────────────────────────────┐
         │      Embedded ao-recorder        │
         │  (real server, in-process)       │
         └──────────────────────────────────┘
              │
              │  state snapshots (mpsc channel)
              ▼
         ┌──────────────────────────────────┐
         │         Viewer API (Axum)        │
         │  REST + WebSocket push           │
         └──────────────────────────────────┘
              │
              │  HTTP / WS
              ▼
         ┌──────────────────────────────────┐
         │      Viewer PWA (React)          │
         │  Map + Agent Detail + Tables     │
         └──────────────────────────────────┘
```

Key design decision: **the simulator embeds the real recorder**. Agents don't talk to a mock. They make HTTP requests to a real Axum server running the real `ao-chain` validation logic with a real SQLite database. If the simulator works, the protocol works.

## Scenario Files

Everything starts with a TOML file:

```toml
[simulation]
name = "island-life"
recorder_port = 0       # auto-assign
speed = 10.0            # 10x real time
duration_secs = 300
mqtt_port = 1884        # enable MQTT for exchange agents

[[agent]]
name = "Bob"
role = "vendor"
lat = 18.2027
lon = -63.0890          # Sandy Ground, Anguilla

[agent.vendor]
symbol = "BCG"
description = "Bob's Curry Goat"
coins = "1000000000"
shares = "2^40"         # parsed as BigInt power expression
plate_price = 25
initial_float = 100

[[agent]]
name = "Charlie"
role = "exchange"
lat = 18.2190
lon = -63.0350

[agent.exchange]
referral_fee = 0.05
rebalance_threshold = 0.25
pairs = [
    { sell = "BCG", buy = "CCC", rate = 12.0 },
    { sell = "RMF", buy = "CCC", rate = 5.0 },
]

[[agent.exchange.inventory]]
vendor = "Bob"
plates = 80
```

The `ScenarioConfig` struct deserializes this via serde:

```rust
#[derive(Deserialize, Debug, Clone)]
pub struct ScenarioConfig {
    pub simulation: SimulationConfig,
    #[serde(rename = "agent")]
    pub agents: Vec<AgentConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub role: String,       // "vendor", "consumer", "exchange", "recorder", "validator", "attacker"
    pub lat: f64,
    pub lon: f64,
    pub vendor: Option<VendorConfig>,
    pub exchange: Option<ExchangeConfig>,
    pub consumer: Option<ConsumerConfig>,
    pub validator: Option<ValidatorConfig>,
    pub attacker: Option<AttackerConfig>,
}
```

Shares are specified as either decimal strings or power expressions (`"2^40"`), parsed into `num_bigint::BigInt`. This avoids forcing scenario authors to type 40-digit numbers while keeping arbitrary-precision arithmetic throughout the stack.

## Agent Framework

Each agent is a tokio task with a role-specific decision loop. The coordinator spawns them all, gives each a `RecorderClient` (HTTP client pointing at the embedded recorder), and wires up communication channels.

```rust
pub enum AgentMessage {
    RequestPubkey {
        chain_id: String,
        reply: oneshot::Sender<PubkeyResponse>,
    },
    SellToMe {
        chain_id: String,
        buyer_name: String,
        receivers: Vec<Receiver>,
        reply: oneshot::Sender<Result<TransferResult>>,
    },
    CrossChainBuy {
        buyer_name: String,
        sell_chain_id: String,
        pay_chain_id: String,
        pay_amount: BigInt,
        receiver_pubkey: [u8; 32],
        receiver_seed: [u8; 32],
        reply: oneshot::Sender<Result<CrossChainResult>>,
    },
    NotifyUtxo {
        pubkey: [u8; 32],
        seq_id: u64,
        amount: BigInt,
    },
}
```

Agents communicate through `mpsc` channels held in an `AgentDirectory`. When a consumer wants to buy from an exchange, it sends a `CrossChainBuy` message. The exchange agent receives it, checks inventory, executes the two-leg trade (sell-chain and pay-chain), and replies with the result.

The simulation simplifies one thing: agents share private key seeds in-process for signing. In production, multi-step signing would be used. But the *protocol interactions* -- the HTTP requests, the block construction, the UTXO validation -- are identical to production.

### Speed Control

Simulated time uses an `AtomicU64` storing `f64` bits:

```rust
pub type SharedSpeed = Arc<AtomicU64>;

pub fn read_speed(speed: &SharedSpeed) -> f64 {
    f64::from_bits(speed.load(Ordering::Relaxed))
}
```

At speed 10.0, an agent that would normally wait 30 seconds between purchases waits 3 seconds. Transaction ordering remains realistic -- agents still interact with the recorder asynchronously and can race against each other. The speed factor only compresses idle time.

Agents can also be paused individually via `AtomicBool` flags, exposed through the viewer API for interactive exploration.

## The Viewer

The viewer is a separate Axum server exposing a REST + WebSocket API:

```rust
pub fn build_viewer_router(state: ViewerAppState) -> Router {
    Router::new()
        .route("/api/agents", get(list_agents))
        .route("/api/agents/{name}", get(get_agent))
        .route("/api/chains", get(list_chains))
        .route("/api/transactions", get(list_transactions))
        .route("/api/agents/{name}/transactions", get(agent_transactions))
        .route("/api/speed", get(get_speed).post(set_speed))
        .route("/api/agents/{name}/pause", post(pause_agent))
        .route("/api/agents/{name}/resume", post(resume_agent))
        .route("/api/ws", get(ws_handler))
        .layer(cors)
        .with_state(state)
}
```

Agents push state snapshots to a `ViewerState` via an mpsc channel. The viewer state is an `Arc<RwLock<...>>` holding the current state of all agents, chains, and recent transactions. The WebSocket handler pushes updates to connected browser clients.

The React frontend (TypeScript, Vite) renders three views:

1. **Map View** (Leaflet): Agents positioned at real lat/lon coordinates. Vendors show open/closed status and inventory. Transaction arcs animate between participants. Heat map overlay for activity density. Audit overlay toggles green/red halos for validator status.

2. **Agent Detail**: Click any agent to see their wallet (balances across all chains), their simulated app screen (what their AOE/AOS/AOI would show), transaction history, and key inventory.

3. **Community Tables**: All agents, all chains, all transactions in sortable/filterable tables.

**Time controls** let the viewer scrub through simulation history. Agent state snapshots are timestamped and stored, so the viewer can replay the entire simulation from any point. Play, pause, speed slider (1x to 100x), and a scrub bar.

## Agent Types

**Vendors** create genesis chains, set pricing, and accept incoming assignments. They're the simplest agents -- mostly they wait for customers.

**Consumers** discover vendor chains, pick an exchange agent, initiate cross-chain purchases, and redeem at vendors. Their behavior is parameterized: budget, purchase frequency, vendor preferences, which exchange agent to use.

**Exchange agents** are the most complex. They:
- Issue their own payment chains (CCC, ZIC, TCC)
- Hold inventory in vendor chains (bought during setup)
- Accept cross-chain buy requests from consumers
- Execute two-leg trades (receive payment on their chain, send inventory on the vendor chain)
- Monitor MQTT for block notifications
- Rebalance when inventory gets lopsided
- Compete on price with other exchange agents (dynamic rate adjustment)

The `price-war.toml` scenario pins two exchange agents against each other on the same trading pair and lets them undercut each other until the spread converges. Watching this happen on the map -- two dots near each other, rates ticking down in their detail panels -- is more convincing than any theoretical argument about market efficiency.

**Validators** run `ao-validator::verify_block_batch()` against the recorder, polling periodically, tracking integrity status per chain, and reporting anomalies to the viewer.

**Adversarial agents** attempt to break the protocol:
- **Double-spend attacker**: Submits conflicting assignments using the same UTXO. The recorder must reject the second.
- **Key-reuse attacker**: Tries to receive shares on an already-used key. Must be rejected.
- **Expired-UTXO attacker**: Builds an assignment referencing a UTXO past its expiration timestamp. Must be rejected.

All attackers log every attempt and outcome. The viewer shows them with red styling and their success/failure rates. In a correctly functioning system, the success rate is always zero.

## What We Learned

**Embed the real server.** The single best decision was running the actual `ao-recorder` in-process rather than building a mock. Every bug found by the simulator was a real protocol bug. The simulator became our most effective integration test.

**TOML scenarios are the right abstraction.** Adding a new test case means writing a new TOML file, not new Rust code. The `island-life.toml` scenario has 19 agents across 7 chains with real Anguilla coordinates, exchange competition, and referral fees. It reads like a story:

```toml
[[agent]]
name = "Alice"
role = "consumer"
lat = 18.2030
lon = -63.0885   # Sandy Ground beach (tourist)

[agent.consumer]
buy_from = "Charlie"
want_symbol = "BCG"
pay_symbol = "CCC"
fund_from = "CCC-Issuer"
interval_secs = 30
```

**AtomicU64 for shared f64 is fine.** We store the speed factor as `f64::to_bits()` in an `AtomicU64` with `Relaxed` ordering. No mutex, no contention. The viewer can adjust speed while agents are running, and each agent picks up the new value on its next sleep cycle.

**The viewer is the demo.** We built the viewer for debugging. It turned out to be the most powerful way to explain the system to anyone. Running `island-life.toml` and narrating what happens on screen communicates the architecture better than any documentation.

## Running It

```bash
cd 2026/sims
cargo run -- scenarios/island-life.toml --viewer-port 4200

# In another terminal:
cd 2026/sims/viewer
npm run dev
# Open http://localhost:5173
```

Six scenarios ship with the code:

| Scenario | Agents | Chains | What It Tests |
|----------|--------|--------|---------------|
| `minimal.toml` | 4 | 1 | Basic buy-redeem loop |
| `three-chain.toml` | 8 | 3 | Multi-chain trading, exchange competition |
| `island-life.toml` | 19 | 7 | Full IslandLife narrative on Anguilla geography |
| `price-war.toml` | 7 | 3 | Two exchanges competing, price discovery |
| `exchange-3chain.toml` | 7 | 3 | Cross-chain trades across 3 chains |
| `audit-adversarial.toml` | 9 | 2 | Validator + 3 attacker types |

The whole thing -- seven protocol crates, simulator, viewer -- is MIT-licensed.

GitHub: [assignonward/aosuite](https://github.com/assignonward/aosuite)

---

*The simulation suite is in `2026/sims/`. The protocol crates are in `2026/src/`. The viewer PWA is in `2026/sims/viewer/`. Total Rust: ~16,000 lines of protocol + ~3,000 lines of simulator. Total TypeScript: ~3,100 lines of PWA + ~2,000 lines of viewer.*
