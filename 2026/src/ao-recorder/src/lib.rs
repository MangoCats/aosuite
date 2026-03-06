use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use axum::{
    Router,
    extract::{Path, State, WebSocketUpgrade, ws},
    http::StatusCode,
    response::{IntoResponse, Json, Sse, sse},
    routing::get,
};
use serde::{Deserialize, Serialize};
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

// ── Error type ──────────────────────────────────────────────────────

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RecorderError {
    #[error("lock poisoned: {0}")]
    LockPoisoned(String),
    #[error("chain not found")]
    ChainNotFound,
    #[error("{0}")]
    NotFound(String),
    #[error("chain already hosted")]
    ChainConflict,
    #[error("chain not loaded")]
    ChainNotLoaded,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Internal(String),
}

impl IntoResponse for RecorderError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            RecorderError::LockPoisoned(_) | RecorderError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
            RecorderError::ChainNotFound | RecorderError::ChainNotLoaded | RecorderError::NotFound(_) => StatusCode::NOT_FOUND,
            RecorderError::ChainConflict => StatusCode::CONFLICT,
            RecorderError::BadRequest(_) => StatusCode::BAD_REQUEST,
        };
        (status, Json(ErrorResponse { error: self.to_string() })).into_response()
    }
}

// ── Per-chain and shared state ──────────────────────────────────────

/// Per-chain state.
pub struct ChainState {
    pub store: Mutex<ChainStore>,
    pub blockmaker_key: SigningKey,
    pub block_tx: broadcast::Sender<BlockInfo>,
}

impl ChainState {
    pub fn new(store: ChainStore, blockmaker_key: SigningKey) -> Self {
        let (block_tx, _) = broadcast::channel(64);
        ChainState {
            store: Mutex::new(store),
            blockmaker_key,
            block_tx,
        }
    }
}

/// Shared application state — holds all hosted chains.
pub struct AppState {
    pub chains: RwLock<HashMap<String, Arc<ChainState>>>,
    pub data_dir: Option<PathBuf>,
    pub default_blockmaker_key: SigningKey,
}

impl AppState {
    /// Create from a single chain (backward-compatible constructor).
    pub fn new(store: ChainStore, blockmaker_key: SigningKey) -> Self {
        let meta = store.load_chain_meta().unwrap().expect("chain must be initialized");
        let chain_id = hex::encode(meta.chain_id);
        let chain_state = Arc::new(ChainState::new(store, SigningKey::from_seed(blockmaker_key.seed())));
        let mut chains = HashMap::new();
        chains.insert(chain_id, chain_state);
        AppState {
            chains: RwLock::new(chains),
            data_dir: None,
            default_blockmaker_key: blockmaker_key,
        }
    }

    /// Create an empty multi-chain host.
    pub fn new_multi(data_dir: Option<PathBuf>, blockmaker_key: SigningKey) -> Self {
        AppState {
            chains: RwLock::new(HashMap::new()),
            data_dir,
            default_blockmaker_key: blockmaker_key,
        }
    }

    /// Register a chain.
    pub fn add_chain(&self, chain_id: String, chain_state: Arc<ChainState>) {
        self.chains.write().expect("chains write lock").insert(chain_id, chain_state);
    }

    /// Get a chain by ID, or RecorderError::ChainNotFound.
    fn get_chain_or_err(&self, chain_id: &str) -> Result<Arc<ChainState>, RecorderError> {
        self.chains.read()
            .map_err(|e| RecorderError::LockPoisoned(format!("chains read: {}", e)))?
            .get(chain_id)
            .cloned()
            .ok_or(RecorderError::ChainNotFound)
    }
}

fn lock_store(chain: &ChainState) -> Result<std::sync::MutexGuard<'_, ChainStore>, RecorderError> {
    chain.store.lock()
        .map_err(|e| RecorderError::LockPoisoned(format!("store lock: {}", e)))
}

// ── Data types ──────────────────────────────────────────────────────

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
struct ChainListEntry {
    chain_id: String,
    symbol: String,
    block_height: u64,
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

#[derive(Deserialize)]
struct CreateChainRequest {
    genesis: serde_json::Value,
    #[serde(default)]
    blockmaker_seed: Option<String>,
}

// ── Router ──────────────────────────────────────────────────────────

/// Build the Axum router for a recorder with the given state.
pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/chains", get(list_chains).post(create_chain))
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

// ── Handlers ────────────────────────────────────────────────────────

/// GET /chains — list all hosted chains.
async fn list_chains(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ChainListEntry>>, RecorderError> {
    // Phase 1: collect chain refs under read lock, then drop it
    let chain_refs: Vec<(String, Arc<ChainState>)> = {
        let chains = state.chains.read()
            .map_err(|e| RecorderError::LockPoisoned(format!("chains read: {}", e)))?;
        chains.iter().map(|(id, cs)| (id.clone(), Arc::clone(cs))).collect()
    };

    // Phase 2: query each store individually (no nested locks)
    let mut entries = Vec::new();
    for (id, cs) in chain_refs {
        let store = lock_store(&cs)?;
        let meta = store.load_chain_meta()
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or(RecorderError::ChainNotLoaded)?;
        entries.push(ChainListEntry {
            chain_id: id,
            symbol: meta.symbol,
            block_height: meta.block_height,
        });
    }
    entries.sort_by(|a, b| a.chain_id.cmp(&b.chain_id));
    Ok(Json(entries))
}

/// POST /chains — create a new chain from a genesis block.
async fn create_chain(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateChainRequest>,
) -> Result<(StatusCode, Json<ChainInfo>), RecorderError> {
    let genesis_item = ao_json::from_json(&req.genesis)
        .map_err(|e| RecorderError::BadRequest(format!("invalid genesis: {}", e)))?;

    let blockmaker_key = if let Some(seed_hex) = &req.blockmaker_seed {
        let seed_bytes = hex::decode(seed_hex.trim())
            .map_err(|e| RecorderError::BadRequest(format!("invalid seed hex: {}", e)))?;
        let seed: [u8; 32] = seed_bytes.try_into()
            .map_err(|_| RecorderError::BadRequest("seed must be 32 bytes".into()))?;
        SigningKey::from_seed(&seed)
    } else {
        SigningKey::from_seed(state.default_blockmaker_key.seed())
    };

    // Open store (file-backed if data_dir is set, otherwise in-memory)
    let store = if let Some(dir) = &state.data_dir {
        let tmp_store = ChainStore::open_memory()
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        tmp_store.init_schema()
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        genesis::load_genesis(&tmp_store, &genesis_item)
            .map_err(|e| RecorderError::BadRequest(format!("genesis error: {}", e)))?;
        let tmp_meta = tmp_store.load_chain_meta()
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or(RecorderError::ChainNotLoaded)?;
        let chain_id_hex = hex::encode(tmp_meta.chain_id);

        let db_path = dir.join(format!("{}.db", chain_id_hex));
        let db_str = db_path.to_str()
            .ok_or_else(|| RecorderError::Internal("non-UTF-8 database path".into()))?;
        let file_store = ChainStore::open(db_str)
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        file_store.init_schema()
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        genesis::load_genesis(&file_store, &genesis_item)
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        file_store
    } else {
        let store = ChainStore::open_memory()
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        store.init_schema()
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        genesis::load_genesis(&store, &genesis_item)
            .map_err(|e| RecorderError::BadRequest(format!("genesis error: {}", e)))?;
        store
    };

    let meta = store.load_chain_meta()
        .map_err(|e| RecorderError::Internal(e.to_string()))?
        .ok_or(RecorderError::ChainNotLoaded)?;
    let chain_id_hex = hex::encode(meta.chain_id);

    let info = ChainInfo {
        chain_id: chain_id_hex.clone(),
        symbol: meta.symbol,
        block_height: meta.block_height,
        shares_out: meta.shares_out.to_string(),
        coin_count: meta.coin_count.to_string(),
        fee_rate_num: meta.fee_rate_num.to_string(),
        fee_rate_den: meta.fee_rate_den.to_string(),
        expiry_period: meta.expiry_period,
        expiry_mode: meta.expiry_mode,
        next_seq_id: meta.next_seq_id,
    };

    // Atomic check-and-insert under a single write lock
    let chain_state = Arc::new(ChainState::new(store, blockmaker_key));
    let mut chains = state.chains.write()
        .map_err(|e| RecorderError::LockPoisoned(format!("chains write: {}", e)))?;
    match chains.entry(chain_id_hex) {
        Entry::Occupied(_) => Err(RecorderError::ChainConflict),
        Entry::Vacant(e) => {
            e.insert(chain_state);
            Ok((StatusCode::CREATED, Json(info)))
        }
    }
}

/// GET /chain/{id}/info
async fn chain_info(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
) -> Result<Json<ChainInfo>, RecorderError> {
    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let store = lock_store(&chain)?;
    let meta = store.load_chain_meta()
        .map_err(|e| RecorderError::Internal(e.to_string()))?
        .ok_or(RecorderError::ChainNotLoaded)?;

    Ok(Json(ChainInfo {
        chain_id: hex::encode(meta.chain_id),
        symbol: meta.symbol,
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
    Path((chain_id_hex, seq_id)): Path<(String, u64)>,
) -> Result<Json<UtxoInfo>, RecorderError> {
    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let store = lock_store(&chain)?;
    let utxo = store.get_utxo(seq_id)
        .map_err(|e| RecorderError::Internal(e.to_string()))?
        .ok_or_else(|| RecorderError::NotFound("UTXO not found".into()))?;

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
    Path(chain_id_hex): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, RecorderError> {
    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let store = lock_store(&chain)?;
    let meta = store.load_chain_meta()
        .map_err(|e| RecorderError::Internal(e.to_string()))?
        .ok_or(RecorderError::ChainNotLoaded)?;

    let from: u64 = params.get("from").and_then(|s| s.parse().ok()).unwrap_or(0);
    let to: u64 = params.get("to").and_then(|s| s.parse().ok()).unwrap_or(meta.block_height);

    let mut blocks = Vec::new();
    for h in from..=to {
        if let Some(data) = store.get_block(h)
            .map_err(|e| RecorderError::Internal(e.to_string()))?
        {
            match DataItem::from_bytes(&data) {
                Ok(item) => blocks.push(ao_json::to_json(&item)),
                Err(e) => return Err(RecorderError::Internal(
                    format!("decode error at height {}: {}", h, e),
                )),
            }
        }
    }

    Ok(Json(blocks))
}

/// POST /chain/{id}/submit
async fn submit_assignment(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    body: String,
) -> Result<Json<BlockInfo>, RecorderError> {
    let json_value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| RecorderError::BadRequest(format!("invalid JSON: {}", e)))?;

    let authorization = ao_json::from_json(&json_value)
        .map_err(|e| RecorderError::BadRequest(format!("invalid DataItem: {}", e)))?;

    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let store = lock_store(&chain)?;
    let meta = store.load_chain_meta()
        .map_err(|e| RecorderError::Internal(e.to_string()))?
        .ok_or(RecorderError::ChainNotLoaded)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;
    let current_ts = Timestamp::from_unix_seconds(now).raw();

    let validated = validate::validate_assignment(&store, &meta, &authorization, current_ts)
        .map_err(|e| RecorderError::BadRequest(e.to_string()))?;

    let constructed = block::construct_block(
        &store, &meta, &chain.blockmaker_key,
        vec![validated],
        current_ts,
    ).map_err(|e| RecorderError::Internal(e.to_string()))?;

    let info = BlockInfo {
        height: constructed.height,
        hash: hex::encode(constructed.block_hash),
        timestamp: constructed.timestamp,
        shares_out: constructed.new_shares_out.to_string(),
        first_seq: constructed.first_seq,
        seq_count: constructed.seq_count,
    };

    let _ = chain.block_tx.send(info.clone());

    Ok(Json(info))
}

/// GET /chain/{id}/events — Server-Sent Events stream of block notifications.
async fn sse_events(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
) -> Result<Sse<impl tokio_stream::Stream<Item = Result<sse::Event, Infallible>>>, RecorderError> {
    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let rx = chain.block_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(info) => {
                let json = serde_json::to_string(&info).expect("BlockInfo always serializable");
                Some(Ok(sse::Event::default().event("block").data(json)))
            }
            Err(_) => None, // lagged — skip
        }
    });
    Ok(Sse::new(stream).keep_alive(sse::KeepAlive::default()))
}

/// GET /chain/{id}/ws — WebSocket stream of block notifications.
async fn ws_handler(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    ws: WebSocketUpgrade,
) -> Result<axum::response::Response, RecorderError> {
    let chain = state.get_chain_or_err(&chain_id_hex)?;
    Ok(ws.on_upgrade(move |socket| ws_connection(socket, chain)))
}

async fn ws_connection(mut socket: ws::WebSocket, chain: Arc<ChainState>) {
    let mut rx = chain.block_tx.subscribe();
    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(info) => {
                        let json = serde_json::to_string(&info).expect("BlockInfo always serializable");
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
