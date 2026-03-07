use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::info;

use ao_types::dataitem::DataItem;
use ao_crypto::sign::SigningKey;
use ao_chain::store::ChainStore;

use ao_recorder::{AppState, ChainState, build_router, config, mqtt};

fn load_blockmaker_key(seed_hex: &str) -> Result<SigningKey> {
    let seed_bytes: Vec<u8> = hex::decode(seed_hex.trim())
        .context("invalid blockmaker seed hex")?;
    let seed: [u8; 32] = seed_bytes.try_into()
        .map_err(|v: Vec<u8>| anyhow::anyhow!("blockmaker seed must be 32 bytes, got {}", v.len()))?;
    SigningKey::try_from_seed(&seed)
        .map_err(|e| anyhow::anyhow!("invalid Ed25519 seed: {}", e))
}

fn load_chain(db_path: &str, genesis_path: &str, blockmaker_key: &SigningKey) -> Result<(String, ChainStore)> {
    let store = ChainStore::open(db_path)
        .context("failed to open database")?;

    let meta = match store.load_chain_meta()
        .context("failed to query chain metadata")?
    {
        Some(m) => {
            info!(chain_id = hex::encode(m.chain_id), symbol = %m.symbol, "Chain loaded");
            m
        }
        None => {
            let genesis_data = std::fs::read(genesis_path)
                .context("failed to read genesis file")?;
            let genesis_item = DataItem::from_bytes(&genesis_data)
                .map_err(|e| anyhow::anyhow!("failed to decode genesis block: {:?}", e))?;
            let m = ao_chain::genesis::load_genesis(&store, &genesis_item)
                .context("failed to load genesis")?;
            info!(chain_id = hex::encode(m.chain_id), symbol = %m.symbol, "Genesis loaded");
            m
        }
    };

    let _ = blockmaker_key; // used by caller
    Ok((hex::encode(meta.chain_id), store))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(|s| s.as_str()).unwrap_or("recorder.toml");

    let cfg = config::load_config(config_path)?;

    let default_key = load_blockmaker_key(&cfg.blockmaker_seed)?;

    let data_dir = cfg.data_dir.as_ref().map(PathBuf::from);
    if let Some(dir) = &data_dir {
        std::fs::create_dir_all(dir).context("failed to create data directory")?;
    }
    let state = Arc::new(AppState::new_multi(data_dir, SigningKey::from_seed(default_key.seed())));

    // Load single-chain config (backward compatible)
    if let (Some(db_path), Some(genesis_path)) = (&cfg.db_path, &cfg.genesis_path) {
        let (chain_id, store) = load_chain(db_path, genesis_path, &default_key)?;
        let chain_state = Arc::new(ChainState::new(store, SigningKey::from_seed(default_key.seed())));
        info!(chain_id = %chain_id, "Registered chain");
        state.add_chain(chain_id, chain_state);
    }

    // Load additional chains from [[chains]] config
    for chain_cfg in &cfg.chains {
        let bm_key = if let Some(seed_hex) = &chain_cfg.blockmaker_seed {
            load_blockmaker_key(seed_hex)?
        } else {
            SigningKey::from_seed(default_key.seed())
        };
        let (chain_id, store) = load_chain(&chain_cfg.db_path, &chain_cfg.genesis_path, &bm_key)?;
        let chain_state = Arc::new(ChainState::new(store, bm_key));
        info!(chain_id = %chain_id, "Registered chain");
        state.add_chain(chain_id, chain_state);
    }

    // Initialize MQTT if configured
    if let Some(mqtt_cfg) = &cfg.mqtt {
        if let Some(publisher) = mqtt::MqttPublisher::connect(mqtt_cfg) {
            state.set_mqtt(publisher);
            info!("MQTT block publishing enabled");
        } else {
            tracing::warn!("MQTT configured but connection failed — continuing without MQTT");
        }
    }

    let chain_count = state.chains.read()
        .map_err(|e| anyhow::anyhow!("chains lock poisoned: {}", e))?
        .len();
    let app = build_router(state);

    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    info!(%bind_addr, chain_count, "Starting AO Recorder");

    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .context("failed to bind TCP listener")?;
    axum::serve(listener, app).await
        .context("server error")?;

    Ok(())
}
