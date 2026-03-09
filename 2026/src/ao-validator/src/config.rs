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
    /// Anchor configuration (N29). If absent, no automatic anchoring.
    #[serde(default)]
    pub anchor: Option<AnchorConfig>,
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

/// Anchor backend configuration (N29).
#[derive(Deserialize, Clone)]
pub struct AnchorConfig {
    /// Path to the primary anchor file (JSON lines).
    pub path: String,
    /// Anchor every N blocks verified. Default: 100.
    #[serde(default = "default_anchor_interval")]
    pub interval_blocks: u64,
    /// Additional file paths for replicated anchoring (disk-failure resilience).
    #[serde(default)]
    pub replica_paths: Vec<String>,
}

fn default_host() -> String { "127.0.0.1".to_string() }
fn default_port() -> u16 { 4000 }
fn default_poll_interval() -> u64 { 60 }
fn default_anchor_interval() -> u64 { 100 }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_config_defaults() {
        let toml = r#"
db_path = "validator.db"
validator_seed = "aa"

[[chains]]
recorder_url = "http://localhost:3000"
chain_id = "abc123"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        assert!(cfg.anchor.is_none());
    }

    #[test]
    fn anchor_config_with_replicas() {
        let toml = r#"
db_path = "validator.db"
validator_seed = "aa"

[anchor]
path = "/var/ao/anchors.jsonl"
interval_blocks = 50
replica_paths = ["/backup/anchors.jsonl", "/mnt/usb/anchors.jsonl"]

[[chains]]
recorder_url = "http://localhost:3000"
chain_id = "abc123"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let anchor = cfg.anchor.unwrap();
        assert_eq!(anchor.path, "/var/ao/anchors.jsonl");
        assert_eq!(anchor.interval_blocks, 50);
        assert_eq!(anchor.replica_paths.len(), 2);
    }

    #[test]
    fn anchor_interval_defaults_to_100() {
        let toml = r#"
db_path = "validator.db"
validator_seed = "aa"

[anchor]
path = "/var/ao/anchors.jsonl"

[[chains]]
recorder_url = "http://localhost:3000"
chain_id = "abc123"
"#;
        let cfg: Config = toml::from_str(toml).unwrap();
        let anchor = cfg.anchor.unwrap();
        assert_eq!(anchor.interval_blocks, 100);
        assert!(anchor.replica_paths.is_empty());
    }
}
