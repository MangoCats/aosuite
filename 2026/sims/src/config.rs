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
}

fn default_speed() -> f64 { 1.0 }
fn default_duration() -> u64 { 300 }

#[derive(Deserialize, Debug, Clone)]
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
}

#[derive(Deserialize, Debug, Clone)]
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
}

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
pub fn auto_fee_den(_shares: &str, coins: &str) -> num_bigint::BigInt {
    let coins_val = parse_bigint(coins);
    // den = 4000 * coins gives fee ≈ 0.1% of total shares per 400-byte page
    // which is about 0.4% of a plate price
    &coins_val * num_bigint::BigInt::from(4000u64)
}

#[derive(Deserialize, Debug, Clone)]
pub struct ExchangeConfig {
    /// Vendor name to buy initial inventory from.
    pub buy_from: String,
    /// Number of plates/units to buy initially.
    #[serde(default = "default_initial_buy")]
    pub initial_buy: u64,
}

fn default_initial_buy() -> u64 { 50 }

#[derive(Deserialize, Debug, Clone)]
pub struct ConsumerConfig {
    /// Exchange agent name to buy from.
    pub buy_from: String,
    /// Vendor name to redeem at.
    pub redeem_at: String,
    /// Seconds between purchases (sim time).
    #[serde(default = "default_interval")]
    pub interval_secs: u64,
}

fn default_interval() -> u64 { 30 }

pub fn parse_bigint(s: &str) -> num_bigint::BigInt {
    use num_traits::One;
    if let Some(exp) = s.strip_prefix("2^") {
        let n: u32 = exp.parse().expect("invalid exponent");
        num_bigint::BigInt::one() << n
    } else {
        s.parse().expect("invalid big integer")
    }
}
