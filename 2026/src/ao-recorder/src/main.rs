use std::sync::Arc;

use tracing::info;

use ao_types::dataitem::DataItem;
use ao_crypto::sign::SigningKey;
use ao_chain::store::ChainStore;

use ao_recorder::{AppState, build_router, config};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(|s| s.as_str()).unwrap_or("recorder.toml");

    let cfg = config::load_config(config_path);

    let store = ChainStore::open(&cfg.db_path).expect("failed to open database");

    let meta = match store.load_chain_meta().unwrap() {
        Some(m) => {
            info!(chain_id = hex::encode(m.chain_id), symbol = %m.symbol, "Chain loaded");
            m
        }
        None => {
            let genesis_data = std::fs::read(&cfg.genesis_path)
                .expect("failed to read genesis file");
            let genesis_item = DataItem::from_bytes(&genesis_data)
                .expect("failed to decode genesis block");
            let m = ao_chain::genesis::load_genesis(&store, &genesis_item)
                .expect("failed to load genesis");
            info!(chain_id = hex::encode(m.chain_id), symbol = %m.symbol, "Genesis loaded");
            m
        }
    };

    let chain_id_hex = hex::encode(meta.chain_id);

    let seed_hex = cfg.blockmaker_seed.trim();
    let seed_bytes: Vec<u8> = hex::decode(seed_hex).expect("invalid blockmaker seed hex");
    let seed: [u8; 32] = seed_bytes.try_into().expect("blockmaker seed must be 32 bytes");
    let blockmaker_key = SigningKey::from_seed(&seed);

    let state = Arc::new(AppState::new(store, blockmaker_key));
    let app = build_router(state);

    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    info!(%bind_addr, chain_id = %chain_id_hex, "Starting AO Recorder");

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
