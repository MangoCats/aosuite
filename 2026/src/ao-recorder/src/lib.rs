use std::sync::{Arc, Mutex};
use std::convert::Infallible;

use axum::{
    Router,
    extract::{Path, State, WebSocketUpgrade, ws},
    http::StatusCode,
    response::{Json, Sse, sse},
    routing::get,
};
use serde::Serialize;
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use ao_types::dataitem::DataItem;
use ao_types::json as ao_json;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign::SigningKey;
use ao_chain::store::ChainStore;
use ao_chain::{genesis, validate, block};

pub mod config;

/// Shared application state.
pub struct AppState {
    pub store: Mutex<ChainStore>,
    pub blockmaker_key: SigningKey,
    pub block_tx: broadcast::Sender<BlockInfo>,
}

impl AppState {
    pub fn new(store: ChainStore, blockmaker_key: SigningKey) -> Self {
        let (block_tx, _) = broadcast::channel(64);
        AppState {
            store: Mutex::new(store),
            blockmaker_key,
            block_tx,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct BlockInfo {
    pub height: u64,
    pub hash: String,
    pub timestamp: i64,
    pub shares_out: String,
    pub first_seq: u64,
    pub seq_count: u64,
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
struct ErrorResponse {
    error: String,
}

fn error_response(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.to_string() }))
}

/// Build the Axum router for a recorder with the given state.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/chain/{id}/info", get(chain_info))
        .route("/chain/{id}/utxo/{seq_id}", get(get_utxo))
        .route("/chain/{id}/blocks", get(get_blocks))
        .route("/chain/{id}/submit", get(method_not_allowed).post(submit_assignment))
        .route("/chain/{id}/events", get(sse_events))
        .route("/chain/{id}/ws", get(ws_handler))
        .with_state(state)
}

async fn method_not_allowed() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

/// Initialize chain state: load genesis if needed, return chain_id hex.
pub fn init_chain(store: &ChainStore, genesis_item: &DataItem) -> String {
    let meta = match store.load_chain_meta().unwrap() {
        Some(m) => m,
        None => genesis::load_genesis(store, genesis_item).expect("failed to load genesis"),
    };
    hex::encode(meta.chain_id)
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
async fn submit_assignment(
    State(state): State<Arc<AppState>>,
    Path(_chain_id): Path<String>,
    body: String,
) -> Result<Json<BlockInfo>, (StatusCode, Json<ErrorResponse>)> {
    let json_value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("invalid JSON: {}", e)))?;

    let authorization = ao_json::from_json(&json_value)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &format!("invalid DataItem: {}", e)))?;

    let store = state.store.lock().unwrap();
    let meta = store.load_chain_meta().unwrap()
        .ok_or_else(|| error_response(StatusCode::NOT_FOUND, "chain not loaded"))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let current_ts = Timestamp::from_unix_seconds(now).raw();

    let validated = validate::validate_assignment(&store, &meta, &authorization, current_ts)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, &e.to_string()))?;

    let constructed = block::construct_block(
        &store, &meta, &state.blockmaker_key,
        vec![validated],
        current_ts,
    ).map_err(|e| error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let info = BlockInfo {
        height: constructed.height,
        hash: hex::encode(constructed.block_hash),
        timestamp: constructed.timestamp,
        shares_out: constructed.new_shares_out.to_string(),
        first_seq: constructed.first_seq,
        seq_count: constructed.seq_count,
    };

    // Broadcast to SSE/WebSocket subscribers (ignore send errors — no receivers is fine)
    let _ = state.block_tx.send(info.clone());

    Ok(Json(info))
}

/// GET /chain/{id}/events — Server-Sent Events stream of block notifications.
async fn sse_events(
    State(state): State<Arc<AppState>>,
    Path(_chain_id): Path<String>,
) -> Sse<impl tokio_stream::Stream<Item = Result<sse::Event, Infallible>>> {
    let rx = state.block_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(info) => {
                let json = serde_json::to_string(&info).unwrap();
                Some(Ok(sse::Event::default().event("block").data(json)))
            }
            Err(_) => None, // lagged — skip
        }
    });
    Sse::new(stream).keep_alive(sse::KeepAlive::default())
}

/// GET /chain/{id}/ws — WebSocket stream of block notifications.
async fn ws_handler(
    State(state): State<Arc<AppState>>,
    Path(_chain_id): Path<String>,
    ws: WebSocketUpgrade,
) -> axum::response::Response {
    ws.on_upgrade(move |socket| ws_connection(socket, state))
}

async fn ws_connection(mut socket: ws::WebSocket, state: Arc<AppState>) {
    let mut rx = state.block_tx.subscribe();
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(info) => {
                        let json = serde_json::to_string(&info).unwrap();
                        if socket.send(ws::Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(ws::Message::Close(_))) | None => break,
                    _ => {} // ignore client messages
                }
            }
        }
    }
}
