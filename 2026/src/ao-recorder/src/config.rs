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
    /// Optional API keys for authentication. Empty = no auth required.
    #[serde(default)]
    pub api_keys: Vec<String>,
    /// Per-IP rate limit for read endpoints (requests/second). 0 = no limit.
    #[serde(default)]
    pub read_rate_limit: f64,
    /// Per-IP rate limit for write endpoints (requests/second). 0 = no limit.
    #[serde(default)]
    pub write_rate_limit: f64,
    /// Max concurrent SSE/WebSocket connections. 0 = no limit.
    #[serde(default)]
    pub max_connections: usize,
    /// Allow non-HTTPS validator URLs (for local dev only).
    #[serde(default)]
    pub allow_insecure_validators: bool,
    /// Maximum single blob size in bytes. Default: 5 MB (5242880).
    #[serde(default = "default_max_blob_bytes")]
    pub max_blob_bytes: usize,
    /// Per-chain blob storage quota in bytes. Default: 100 MB (104857600).
    #[serde(default = "default_blob_quota_per_chain")]
    pub blob_quota_per_chain: u64,
    /// Human-readable recorder name/description for RECORDER_IDENTITY.
    #[serde(default)]
    pub recorder_name: Option<String>,
    /// Public URL where this recorder is reachable (e.g. "https://recorder.example.com").
    #[serde(default)]
    pub recorder_url: Option<String>,
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
    /// Alert check interval in seconds. Default: 60.
    #[serde(default = "default_check_interval")]
    pub check_interval_seconds: u64,
    /// Optional webhook URL for alert notifications.
    #[serde(default)]
    pub webhook_url: Option<String>,
}

fn default_max_blob_bytes() -> usize { 5_242_880 }
fn default_blob_quota_per_chain() -> u64 { 100 * 1024 * 1024 }

fn default_disk_warn() -> f64 { 10.0 }
fn default_disk_error() -> f64 { 5.0 }
fn default_stale_seconds() -> u64 { 86400 }
fn default_memory_interval() -> u64 { 3600 }
fn default_check_interval() -> u64 { 60 }

#[derive(Deserialize, Clone)]
pub struct ValidatorEndpoint {
    /// Validator API base URL (e.g. "http://localhost:4000").
    pub url: String,
    /// Human-readable label.
    #[serde(default)]
    pub label: Option<String>,
}

/// Allow non-HTTPS validator URLs (for local development only).
#[derive(Deserialize, Default, Clone)]
pub struct SecurityConfig {
    #[serde(default)]
    pub allow_insecure_validators: bool,
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
            api_keys: Vec::new(),
            read_rate_limit: 0.0,
            write_rate_limit: 0.0,
            max_connections: 0,
            allow_insecure_validators: false,
            max_blob_bytes: default_max_blob_bytes(),
            blob_quota_per_chain: default_blob_quota_per_chain(),
            recorder_name: None,
            recorder_url: None,
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
    // Validate validator URLs
    for (i, v) in config.validators.iter().enumerate() {
        let url = v.url.trim_end_matches('/');
        if !url.starts_with("http://") && !url.starts_with("https://") {
            anyhow::bail!(
                "validators[{}].url must start with http:// or https://, got: {}",
                i, v.url
            );
        }
        if !config.allow_insecure_validators && url.starts_with("http://") {
            // Allow localhost/127.0.0.1/[::1] without HTTPS
            let host_part = url.strip_prefix("http://").unwrap_or("");
            let is_local = host_part.starts_with("localhost")
                || host_part.starts_with("127.")
                || host_part.starts_with("[::1]");
            if !is_local {
                anyhow::bail!(
                    "validators[{}].url must use HTTPS for non-local hosts (set allow_insecure_validators = true to override): {}",
                    i, v.url
                );
            }
        }
    }
    Ok(config)
}
