use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct MqttConfig {
    /// MQTT broker URL, e.g. "localhost" or "broker.example.com".
    pub host: String,
    /// MQTT broker port (default 1883).
    #[serde(default = "default_mqtt_port")]
    pub port: u16,
    /// MQTT client ID (must be unique per recorder instance).
    #[serde(default = "default_client_id")]
    pub client_id: String,
    /// Topic prefix for block notifications. Blocks publish to `{prefix}/{chain_id}/blocks`.
    #[serde(default = "default_topic_prefix")]
    pub topic_prefix: String,
}

fn default_mqtt_port() -> u16 { 1883 }
fn default_client_id() -> String { "ao-recorder".to_string() }
fn default_topic_prefix() -> String { "ao/chain".to_string() }

#[derive(Deserialize)]
pub struct ChainConfig {
    pub db_path: String,
    pub genesis_path: String,
    #[serde(default)]
    pub blockmaker_seed: Option<String>,
}

#[derive(Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub blockmaker_seed: String,
    #[serde(default)]
    pub data_dir: Option<String>,
    /// Single-chain backward-compatible fields.
    #[serde(default)]
    pub db_path: Option<String>,
    #[serde(default)]
    pub genesis_path: Option<String>,
    /// Multiple chain configs.
    #[serde(default)]
    pub chains: Vec<ChainConfig>,
    /// Optional MQTT configuration for block notification publishing.
    #[serde(default)]
    pub mqtt: Option<MqttConfig>,
    /// Optional validator endpoints for trust indicators in chain info.
    #[serde(default)]
    pub validators: Vec<ValidatorEndpoint>,
    /// Known recorder public keys for CAA recording proof verification.
    /// Maps chain_id hex → recorder pubkey hex.
    #[serde(default)]
    pub known_recorders: std::collections::HashMap<String, String>,
    /// Optional operational alert configuration.
    #[serde(default)]
    pub alerts: Option<AlertsConfig>,
    /// Enable the /dashboard HTML page.
    #[serde(default)]
    pub dashboard: bool,
}

#[derive(Deserialize, Clone)]
pub struct AlertsConfig {
    /// Disk free space warning threshold (percent). Default: 10.0.
    #[serde(default = "default_disk_warn")]
    pub disk_warn_percent: f64,
    /// Disk free space error threshold (percent). Default: 5.0.
    #[serde(default = "default_disk_error")]
    pub disk_error_percent: f64,
    /// Stale chain alert threshold in seconds. Default: 86400 (24h).
    #[serde(default = "default_stale_seconds")]
    pub stale_chain_seconds: u64,
    /// Memory baseline logging interval in seconds. Default: 3600 (1h).
    #[serde(default = "default_memory_interval")]
    pub memory_log_interval_seconds: u64,
    /// Optional webhook URL for alert notifications.
    #[serde(default)]
    pub webhook_url: Option<String>,
}

fn default_disk_warn() -> f64 { 10.0 }
fn default_disk_error() -> f64 { 5.0 }
fn default_stale_seconds() -> u64 { 86400 }
fn default_memory_interval() -> u64 { 3600 }

#[derive(Deserialize, Clone)]
pub struct ValidatorEndpoint {
    /// Validator API base URL (e.g. "http://localhost:4000").
    pub url: String,
    /// Human-readable label.
    #[serde(default)]
    pub label: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            blockmaker_seed: String::new(),
            data_dir: None,
            db_path: Some("chain.db".to_string()),
            genesis_path: Some("genesis.bin".to_string()),
            chains: Vec::new(),
            mqtt: None,
            validators: Vec::new(),
            known_recorders: std::collections::HashMap::new(),
            alerts: None,
            dashboard: false,
        }
    }
}

pub fn load_config(path: &str) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read config file {}: {}", path, e))?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", path, e))?;
    if config.blockmaker_seed.is_empty() {
        anyhow::bail!("blockmaker_seed is required in {}", path);
    }
    Ok(config)
}
