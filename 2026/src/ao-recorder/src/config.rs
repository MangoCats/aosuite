use serde::Deserialize;

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
        }
    }
}

pub fn load_config(path: &str) -> Config {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            toml::from_str(&content).unwrap_or_else(|e| {
                eprintln!("Warning: failed to parse {}: {}. Using defaults.", path, e);
                Config::default()
            })
        }
        Err(_) => {
            eprintln!("Warning: config file {} not found. Using defaults.", path);
            Config::default()
        }
    }
}
