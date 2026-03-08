use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    /// Trading pairs this exchange agent serves.
    pub pairs: Vec<TradingPair>,
    /// Chain connections (recorder URLs and chain IDs).
    pub chains: Vec<ChainConnection>,
    /// Optional MQTT config for real-time block notifications.
    #[serde(default)]
    pub mqtt: Option<MqttConfig>,
    /// Polling interval in seconds for deposit detection (default: 5).
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Trade request TTL in seconds (default: 300).
    #[serde(default = "default_trade_ttl")]
    pub trade_ttl_secs: u64,
    /// Assignment deadline in seconds from now (default: 86400).
    #[serde(default = "default_deadline")]
    pub deadline_secs: i64,
}

fn default_poll_interval() -> u64 { 5 }
fn default_trade_ttl() -> u64 { 300 }
fn default_deadline() -> i64 { 86400 }

#[derive(Deserialize, Clone)]
pub struct TradingPair {
    /// Symbol of chain we sell (consumer receives).
    pub sell: String,
    /// Symbol of chain we accept as payment.
    pub buy: String,
    /// Base exchange rate: buy units per sell unit.
    pub rate: f64,
    /// Bid/ask spread as a fraction (e.g. 0.02 = 2% total spread).
    #[serde(default = "default_spread")]
    pub spread: f64,
    /// Minimum trade size in sell-chain coins.
    #[serde(default)]
    pub min_trade: Option<u64>,
    /// Maximum trade size in sell-chain coins.
    #[serde(default)]
    pub max_trade: Option<u64>,
}

fn default_spread() -> f64 { 0.02 }

#[derive(Deserialize, Clone)]
pub struct ChainConnection {
    /// Chain symbol (e.g. "BCG").
    pub symbol: String,
    /// Recorder base URL (e.g. "http://localhost:3000").
    pub recorder_url: String,
    /// Chain ID hex. If omitted, discovered via GET /chains matching symbol.
    #[serde(default)]
    pub chain_id: Option<String>,
    /// Hex-encoded Ed25519 seed for this chain's signing key.
    pub key_seed: String,
    /// Maximum shares to hold on this chain (position limit).
    #[serde(default)]
    pub max_position: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct MqttConfig {
    pub host: String,
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    #[serde(default = "default_client_id")]
    pub client_id: String,
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,
}

fn default_mqtt_port() -> u16 { 1883 }
fn default_client_id() -> String { "ao-exchange".to_string() }
fn default_topic_prefix() -> String { "ao/chain".to_string() }

pub fn load_config(path: &str) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read config {}: {}", path, e))?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", path, e))?;
    if config.pairs.is_empty() {
        anyhow::bail!("at least one [[pairs]] entry required in {}", path);
    }
    if config.chains.is_empty() {
        anyhow::bail!("at least one [[chains]] entry required in {}", path);
    }
    Ok(config)
}
