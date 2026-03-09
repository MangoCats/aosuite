use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct ScenarioConfig {
    pub simulation: SimulationConfig,
    #[serde(rename = "agent")]
    pub agents: Vec<AgentConfig>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct SimulationConfig {
    pub name: String,
    /// Display title for viewer onboarding overlay.
    #[serde(default)]
    pub title: Option<String>,
    /// 1-3 sentence overview for first-time visitors.
    #[serde(default)]
    pub description: Option<String>,
    /// Bulleted guidance: what to watch for in this scenario.
    #[serde(default)]
    pub what_to_watch: Vec<String>,
    /// Recorder port. 0 = auto-assign.
    #[serde(default)]
    pub recorder_port: u16,
    /// Speed multiplier: 1.0 = real-time, 10.0 = 10x faster.
    #[serde(default = "default_speed")]
    pub speed: f64,
    /// Total simulation duration in seconds (real time).
    #[serde(default = "default_duration")]
    pub duration_secs: u64,
    /// MQTT broker port (0 = disabled).
    #[serde(default)]
    pub mqtt_port: u16,
    /// Optional recorder security config (N10 features).
    /// When present, enables API key auth, rate limiting, connection limits.
    #[serde(default)]
    pub recorder_security: Option<RecorderSecurityConfig>,
    /// Secondary recorder port for dual-recorder scenarios (Sim-G).
    /// When non-zero, a second embedded recorder is started on this port.
    #[serde(default)]
    pub secondary_recorder_port: u16,
}

fn default_speed() -> f64 { 1.0 }
fn default_duration() -> u64 { 300 }

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // lat/lon deserialized for Sim-C map view
pub struct AgentConfig {
    pub name: String,
    pub role: String,
    /// One-sentence description for viewer tooltips and welcome panel.
    #[serde(default)]
    pub blurb: Option<String>,
    #[serde(default)]
    pub lat: f64,
    #[serde(default)]
    pub lon: f64,
    /// Vendor-specific config.
    #[serde(default)]
    pub vendor: Option<VendorConfig>,
    /// Exchange-specific config.
    #[serde(default)]
    pub exchange: Option<ExchangeConfig>,
    /// Consumer-specific config.
    #[serde(default)]
    pub consumer: Option<ConsumerConfig>,
    /// Validator-specific config.
    #[serde(default)]
    pub validator: Option<ValidatorConfig>,
    /// Attacker-specific config.
    #[serde(default)]
    pub attacker: Option<AttackerConfig>,
    /// White-hat infrastructure tester config.
    #[serde(default)]
    pub infra_tester: Option<InfraTesterConfig>,
    /// Recorder operator config (Sim-G: TⒶ³ operations).
    #[serde(default)]
    pub recorder_operator: Option<RecorderOperatorConfig>,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // initial_float deserialized for future vendor float management
pub struct VendorConfig {
    pub symbol: String,
    #[serde(default = "default_description")]
    pub description: String,
    /// Display coin count (e.g., "1000000000").
    #[serde(default = "default_coins")]
    pub coins: String,
    /// Initial shares (e.g., "2^40" or decimal).
    #[serde(default = "default_shares")]
    pub shares: String,
    /// Fee rate numerator (default "1").
    #[serde(default = "default_fee_num")]
    pub fee_num: String,
    /// Fee rate denominator (default auto-calculated from shares).
    #[serde(default)]
    pub fee_den: Option<String>,
    /// Coins per plate/unit.
    #[serde(default = "default_plate_price")]
    pub plate_price: u64,
    /// Number of plates to sell to exchange agents initially.
    #[serde(default = "default_initial_float")]
    pub initial_float: u64,
    /// Coverage radius in meters for map overlay (default 500).
    #[serde(default = "default_coverage_radius")]
    pub coverage_radius_m: f64,
}

fn default_coverage_radius() -> f64 { 500.0 }

fn default_description() -> String { String::new() }
fn default_coins() -> String { "1000000000".to_string() }
fn default_shares() -> String { "2^40".to_string() }
fn default_fee_num() -> String { "1".to_string() }
fn default_plate_price() -> u64 { 25 }
fn default_initial_float() -> u64 { 50 }

/// Compute a sensible fee rate denominator for a given share supply.
/// Target: fee for a ~400-byte page ≈ 0.4% of a plate (25 coins).
/// fee = ceil(400 * num * shares / den) ≈ shares * 25 * 0.004 / coins
/// => den ≈ 400 * num * coins / (25 * 0.004) = 400 * coins / 0.1 = 4000 * coins
pub fn auto_fee_den(coins: &str) -> num_bigint::BigInt {
    let coins_val = parse_bigint(coins);
    // den = 4000 * coins gives fee ≈ 0.1% of total shares per 400-byte page
    // which is about 0.4% of a plate price
    &coins_val * num_bigint::BigInt::from(4000u64)
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // atomic deserialized for scenario documentation; behavior driven by message type
pub struct ExchangeConfig {
    /// Single-chain mode: vendor name to buy initial inventory from.
    #[serde(default)]
    pub buy_from: Option<String>,
    /// Single-chain mode: number of plates to buy initially.
    #[serde(default = "default_initial_buy")]
    pub initial_buy: u64,
    /// Multi-chain mode: trading pairs.
    #[serde(default)]
    pub pairs: Vec<TradingPairConfig>,
    /// Multi-chain mode: initial inventory to buy per chain symbol.
    #[serde(default)]
    pub inventory: Vec<InventoryConfig>,
    /// Referral fee: fraction kept when referring a customer to another exchange (0.0–1.0).
    #[serde(default)]
    pub referral_fee: f64,
    /// Rebalance threshold: restock when inventory drops below this fraction of initial (0.0–1.0).
    #[serde(default = "default_rebalance_threshold")]
    pub rebalance_threshold: f64,
    /// Enable dynamic price discovery (adjust rates based on competitor rates).
    #[serde(default)]
    pub price_discovery: bool,
    /// How often (in seconds) to adjust rates when price_discovery is enabled.
    #[serde(default = "default_adjust_interval")]
    pub adjust_interval_secs: u64,
    /// Enable CAA atomic swaps (default false for backward compat).
    #[serde(default)]
    pub atomic: bool,
    /// CAA escrow deadline in seconds (default 120).
    #[serde(default = "default_escrow_secs")]
    pub escrow_secs: u64,
}

fn default_adjust_interval() -> u64 { 30 }
fn default_escrow_secs() -> u64 { 120 }

#[derive(Deserialize, Debug, Clone)]
pub struct TradingPairConfig {
    /// Symbol of chain we sell (consumer receives).
    pub sell: String,
    /// Symbol of chain we accept as payment (consumer pays).
    pub buy: String,
    /// Exchange rate: how many `buy` units per `sell` unit.
    /// E.g., rate=12 means 1 BCG costs 12 CCC.
    #[serde(default = "default_rate")]
    pub rate: f64,
}

fn default_rate() -> f64 { 1.0 }

#[derive(Deserialize, Debug, Clone)]
pub struct InventoryConfig {
    /// Vendor name to buy from.
    pub vendor: String,
    /// Number of plates/units to buy.
    #[serde(default = "default_initial_buy")]
    pub plates: u64,
}

fn default_initial_buy() -> u64 { 50 }
fn default_rebalance_threshold() -> f64 { 0.25 }

#[derive(Deserialize, Debug, Clone)]
pub struct ConsumerConfig {
    /// Exchange agent name to buy from.
    pub buy_from: String,
    /// Vendor name to redeem at (single-chain mode).
    #[serde(default)]
    pub redeem_at: Option<String>,
    /// For cross-chain: symbol of chain consumer wants.
    #[serde(default)]
    pub want_symbol: Option<String>,
    /// For cross-chain: symbol of chain consumer pays with.
    #[serde(default)]
    pub pay_symbol: Option<String>,
    /// Vendor to buy initial payment currency from (cross-chain mode).
    #[serde(default)]
    pub fund_from: Option<String>,
    /// Seconds between purchases (sim time).
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
    /// Use CAA atomic swap instead of two-leg cross-chain (default false).
    #[serde(default)]
    pub atomic: bool,
}

fn default_interval() -> u64 { 30 }

#[derive(Deserialize, Debug, Clone)]
pub struct ValidatorConfig {
    /// Poll interval in seconds (sim time).
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Max blocks to fetch per poll batch.
    #[serde(default = "default_batch_size")]
    pub batch_size: u64,
}

fn default_poll_interval() -> u64 { 10 }
fn default_batch_size() -> u64 { 100 }

#[derive(Deserialize, Debug, Clone)]
pub struct AttackerConfig {
    /// Attack type: "double_spend", "expired_utxo", "key_reuse", "chain_tamper".
    pub attack: String,
    /// Target vendor name to interact with.
    pub target_vendor: String,
    /// Seconds between attack attempts (sim time).
    #[serde(default = "default_attack_interval")]
    pub attack_interval_secs: u64,
}

fn default_attack_interval() -> u64 { 15 }

/// White-hat infrastructure tester configuration.
/// These agents test server-level resilience (N10 security hardening),
/// NOT protocol correctness. They verify that the recorder properly
/// enforces rate limits, auth, payload guards, and connection limits.
#[derive(Deserialize, Debug, Clone)]
pub struct InfraTesterConfig {
    /// Test type: "flood", "oversized_payload", "auth_bypass",
    /// "connection_exhaustion", "error_probe".
    pub test: String,
    /// Target vendor name (used to find a valid chain_id for requests).
    pub target_vendor: String,
    /// Seconds between test rounds (sim time).
    #[serde(default = "default_infra_interval")]
    pub probe_interval_secs: u64,
    /// Number of concurrent requests for flood/connection tests.
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
    /// API key to use for authenticated requests (empty = test without auth).
    #[serde(default)]
    pub api_key: Option<String>,
}

fn default_infra_interval() -> u64 { 10 }
fn default_concurrency() -> usize { 50 }

/// Optional recorder security config, embedded in SimulationConfig.
/// When present, the sim starts the recorder with these N10 features enabled.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct RecorderSecurityConfig {
    /// API keys for authentication. Empty = no auth.
    #[serde(default)]
    pub api_keys: Vec<String>,
    /// Per-IP rate limit for read endpoints (req/sec). 0 = no limit.
    #[serde(default)]
    pub read_rate_limit: f64,
    /// Per-IP rate limit for write endpoints (req/sec). 0 = no limit.
    #[serde(default)]
    pub write_rate_limit: f64,
    /// Max concurrent SSE/WebSocket connections. 0 = no limit.
    #[serde(default)]
    pub max_connections: usize,
}

/// Recorder operator configuration for TⒶ³ chain infrastructure operations.
/// The operator manages owner key rotation, recorder switching, and chain migration
/// on a timed schedule within the simulation.
#[derive(Deserialize, Debug, Clone)]
pub struct RecorderOperatorConfig {
    /// Target vendor name whose chain this operator manages.
    pub target_chain: String,
    /// Seconds (sim time) after start to rotate the owner key.
    /// 0 or absent = skip rotation.
    #[serde(default)]
    pub rotate_after_secs: u64,
    /// Seconds (sim time) after start to initiate recorder switch.
    /// 0 or absent = skip switch.
    #[serde(default)]
    pub switch_after_secs: u64,
    /// Seconds (sim time) after start to perform chain migration.
    /// 0 or absent = skip migration.
    #[serde(default)]
    pub migrate_after_secs: u64,
    /// Symbol for the new chain created during migration (default: target + "2").
    #[serde(default)]
    pub migration_symbol: Option<String>,
}

pub fn parse_bigint(s: &str) -> num_bigint::BigInt {
    use num_traits::One;
    if let Some(exp) = s.strip_prefix("2^") {
        let n: u32 = exp.parse().expect("invalid exponent");
        num_bigint::BigInt::one() << n
    } else {
        s.parse().expect("invalid big integer")
    }
}
