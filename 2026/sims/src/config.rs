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
}

fn default_speed() -> f64 { 1.0 }
fn default_duration() -> u64 { 300 }

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // lat/lon deserialized for Sim-C map view
pub struct AgentConfig {
    pub name: String,
    pub role: String,
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
}

fn default_adjust_interval() -> u64 { 30 }

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
    /// Attack type: "double_spend", "expired_utxo", "key_reuse".
    pub attack: String,
    /// Target vendor name to interact with.
    pub target_vendor: String,
    /// Seconds between attack attempts (sim time).
    #[serde(default = "default_attack_interval")]
    pub attack_interval_secs: u64,
}

fn default_attack_interval() -> u64 { 15 }

pub fn parse_bigint(s: &str) -> num_bigint::BigInt {
    use num_traits::One;
    if let Some(exp) = s.strip_prefix("2^") {
        let n: u32 = exp.parse().expect("invalid exponent");
        num_bigint::BigInt::one() << n
    } else {
        s.parse().expect("invalid big integer")
    }
}
