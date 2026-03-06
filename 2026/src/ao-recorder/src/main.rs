use std::sync::{Arc, Mutex};

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use serde::Serialize;
use tracing::info;

use ao_types::dataitem::DataItem;
use ao_types::json as ao_json;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign::SigningKey;
use ao_chain::store::ChainStore;
use ao_chain::{genesis, validate, block};

mod config;

/// Shared application state.
struct AppState {
    store: Mutex<ChainStore>,
    blockmaker_key: SigningKey,
}

#[derive(Serialize)]
struct ChainInfo {
    chain_id: String,
    symbol: String,
    block_height: u64,
    shares_out: String,
    coin_count: String,
    fee_rate_num: String,
    fee_rate_den: String,
    expiry_period: i64,
    expiry_mode: u64,
    next_seq_id: u64,
}

#[derive(Serialize)]
struct UtxoInfo {
    seq_id: u64,
    pubkey: String,
    amount: String,
    block_height: u64,
    block_timestamp: i64,
    status: String,
}

#[derive(Serialize)]
struct BlockInfo {
    height: u64,
    hash: String,
    timestamp: i64,
    shares_out: String,
    first_seq: u64,
    seq_count: u64,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn error_response(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.to_string() }))
}

/// GET /chain/{id}/info
async fn chain_info(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
) -> Result<Json<ChainInfo>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.store.lock().unwrap();
    let meta = store.load_chain_meta().unwrap()
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "chain not loaded"))?;

    let expected_id = hex::encode(meta.chain_id);
    if chain_id_hex != expected_id {
        return Err(error_response(StatusCode::NOT_FOUND, "chain not found"));
    }

    Ok(Json(ChainInfo {
        chain_id: expected_id,
        symbol: meta.symbol.clone(),
        block_height: meta.block_height,
        shares_out: meta.shares_out.to_string(),
        coin_count: meta.coin_count.to_string(),
        fee_rate_num: meta.fee_rate_num.to_string(),
        fee_rate_den: meta.fee_rate_den.to_string(),
        expiry_period: meta.expiry_period,
        expiry_mode: meta.expiry_mode,
        next_seq_id: meta.next_seq_id,
    }))
}

/// GET /chain/{id}/utxo/{seq_id}
async fn get_utxo(
    State(state): State<Arc<AppState>>,
    Path((_chain_id, seq_id)): Path<(String, u64)>,
) -> Result<Json<UtxoInfo>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.store.lock().unwrap();
    let utxo = store.get_utxo(seq_id)
        .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "UTXO not found"))?;

    Ok(Json(UtxoInfo {
        seq_id: utxo.seq_id,
        pubkey: hex::encode(utxo.pubkey),
        amount: utxo.amount.to_string(),
        block_height: utxo.block_height,
        block_timestamp: utxo.block_timestamp,
        status: format!("{:?}", utxo.status),
    }))
}

/// GET /chain/{id}/blocks?from={height}&to={height}
async fn get_blocks(
    State(state): State<Arc<AppState>>,
    Path(_chain_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.store.lock().unwrap();
    let meta = store.load_chain_meta().unwrap()
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "chain not loaded"))?;

    let from: u64 = params.get("from").and_then(|s| s.parse().ok()).unwrap_or(0);
    let to: u64 = params.get("to").and_then(|s| s.parse().ok()).unwrap_or(meta.block_height);

    let mut blocks = Vec::new();
    for h in from..=to {
        if let Some(data) = store.get_block(h)
            .map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?
        {
            match DataItem::from_bytes(&data) {
                Ok(item) => blocks.push(ao_json::to_json(&item)),
                Err(e) => return Err(error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("decode error at height {}: {}", h, e),
                )),
            }
        }
    }

    Ok(Json(blocks))
}

/// POST /chain/{id}/submit
/// Accepts a JSON-encoded AUTHORIZATION DataItem.
async fn submit_assignment(
    State(state): State<Arc<AppState>>,
    Path(_chain_id): Path<String>,
    body: String,
) -> Result<Json<BlockInfo>, (StatusCode, Json<ErrorResponse>)> {
    // Parse the submitted authorization
    let json_value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("invalid JSON: {}", e)))?;

    let authorization = ao_json::from_json(&json_value)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("invalid DataItem: {}", e)))?;

    let store = state.store.lock().unwrap();
    let meta = store.load_chain_meta().unwrap()
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "chain not loaded"))?;

    // Current timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let current_ts = Timestamp::from_unix_seconds(now).raw();

    // Validate
    let validated = validate::validate_assignment(&store, &meta, &authorization, current_ts)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &e.to_string()))?;

    // Construct block
    let constructed = block::construct_block(
        &store, &meta, &state.blockmaker_key,
        vec![validated],
        current_ts,
    ).map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    info!(
        height = constructed.height,
        hash = hex::encode(constructed.block_hash),
        "Block recorded"
    );

    Ok(Json(BlockInfo {
        height: constructed.height,
        hash: hex::encode(constructed.block_hash),
        timestamp: constructed.timestamp,
        shares_out: constructed.new_shares_out.to_string(),
        first_seq: constructed.first_seq,
        seq_count: constructed.seq_count,
    }))
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let config_path = args.get(1).map(|s| s.as_str()).unwrap_or("recorder.toml");

    let cfg = config::load_config(config_path);

    // Open or create chain store
    let store = ChainStore::open(&cfg.db_path).expect("failed to open database");

    // Load genesis if chain not yet initialized
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
            let m = genesis::load_genesis(&store, &genesis_item)
                .expect("failed to load genesis");
            info!(chain_id = hex::encode(m.chain_id), symbol = %m.symbol, "Genesis loaded");
            m
        }
    };

    let chain_id_hex = hex::encode(meta.chain_id);

    // Load blockmaker key
    let seed_hex = cfg.blockmaker_seed.trim();
    let seed_bytes: Vec<u8> = hex::decode(seed_hex).expect("invalid blockmaker seed hex");
    let seed: [u8; 32] = seed_bytes.try_into().expect("blockmaker seed must be 32 bytes");
    let blockmaker_key = SigningKey::from_seed(&seed);

    let state = Arc::new(AppState {
        store: Mutex::new(store),
        blockmaker_key,
    });

    let app = Router::new()
        .route("/chain/{id}/info", get(chain_info))
        .route("/chain/{id}/utxo/{seq_id}", get(get_utxo))
        .route("/chain/{id}/blocks", get(get_blocks))
        .route("/chain/{id}/submit", post(submit_assignment))
        .with_state(state);

    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    info!(%bind_addr, chain_id = %chain_id_hex, "Starting AO Recorder");

    let listener = tokio::net::TcpListener::bind(&bind_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
