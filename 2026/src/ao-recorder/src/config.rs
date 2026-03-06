use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub db_path: String,
    pub genesis_path: String,
    pub blockmaker_seed: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            host: "127.0.0.1".to_string(),
            port: 3000,
            db_path: "chain.db".to_string(),
            genesis_path: "genesis.bin".to_string(),
            blockmaker_seed: String::new(),
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
