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
    /// Deposit detection mode: "sse" (default) or "polling".
    #[serde(default = "default_deposit_detection")]
    pub deposit_detection: String,
    /// Polling interval in seconds for deposit detection (default: 5).
    /// Used as poll interval in polling mode and as fallback interval when SSE drops.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Trade request TTL in seconds (default: 300).
    #[serde(default = "default_trade_ttl")]
    pub trade_ttl_secs: u64,
    /// Assignment deadline in seconds from now (default: 86400).
    #[serde(default = "default_deadline")]
    pub deadline_secs: i64,
}

fn default_deposit_detection() -> String { "sse".to_string() }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deposit_detection_defaults_to_sse() {
        let toml = r#"
[[pairs]]
sell = "BCG"
buy = "MPF"
rate = 1.5

[[chains]]
symbol = "BCG"
recorder_url = "http://localhost:3000"
key_seed = "aa"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.deposit_detection, "sse");
    }

    #[test]
    fn deposit_detection_can_be_set_to_polling() {
        let toml = r#"
deposit_detection = "polling"

[[pairs]]
sell = "BCG"
buy = "MPF"
rate = 1.5

[[chains]]
symbol = "BCG"
recorder_url = "http://localhost:3000"
key_seed = "aa"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.deposit_detection, "polling");
    }

    #[test]
    fn invalid_deposit_detection_rejected() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_invalid_detect.toml");
        std::fs::write(&path, r#"
deposit_detection = "SSE"

[[pairs]]
sell = "A"
buy = "B"
rate = 1.0

[[chains]]
symbol = "A"
recorder_url = "http://localhost:3000"
key_seed = "bb"
"#).unwrap();
        let result = super::load_config(path.to_str().unwrap());
        let err = result.err().expect("should fail on invalid deposit_detection");
        let msg = err.to_string();
        assert!(msg.contains("invalid deposit_detection"), "got: {}", msg);
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn poll_interval_default() {
        let toml = r#"
[[pairs]]
sell = "A"
buy = "B"
rate = 1.0

[[chains]]
symbol = "A"
recorder_url = "http://localhost:3000"
key_seed = "bb"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert_eq!(cfg.poll_interval_secs, 5);
    }
}

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
    if config.deposit_detection != "sse" && config.deposit_detection != "polling" {
        anyhow::bail!(
            "invalid deposit_detection '{}' in {} (must be 'sse' or 'polling')",
            config.deposit_detection, path,
        );
    }
    Ok(config)
}
