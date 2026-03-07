use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct Config {
    /// Bind address for the validation API server.
    #[serde(default = "default_host")]
    pub host: String,
    /// Port for the validation API server.
    #[serde(default = "default_port")]
    pub port: u16,
    /// Path to the validator's SQLite database.
    pub db_path: String,
    /// Hex-encoded Ed25519 seed for signing attestations.
    pub validator_seed: String,
    /// How often to poll recorders, in seconds.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    /// Chains to monitor.
    pub chains: Vec<MonitoredChain>,
    /// Optional webhook URL for alteration alerts.
    #[serde(default)]
    pub webhook_url: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct MonitoredChain {
    /// Recorder base URL (e.g. "http://localhost:3000").
    pub recorder_url: String,
    /// Chain ID hex string.
    pub chain_id: String,
    /// Human-readable label for this chain.
    #[serde(default)]
    pub label: Option<String>,
}

fn default_host() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 4000 }
fn default_poll_interval() -> u64 { 60 }

pub fn load_config(path: &str) -> anyhow::Result<Config> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("failed to read config {}: {}", path, e))?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| anyhow::anyhow!("failed to parse {}: {}", path, e))?;
    if config.chains.is_empty() {
        anyhow::bail!("at least one [[chains]] entry required in {}", path);
    }
    if config.validator_seed.is_empty() {
        anyhow::bail!("validator_seed is required in {}", path);
    }
    Ok(config)
}
