use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use axum::{
    Router,
    extract::{DefaultBodyLimit, Path, State, WebSocketUpgrade, ws},
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
use ao_chain::{genesis, validate, block, caa};

pub mod config;
pub mod mqtt;

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

/// A registered exchange agent advertising trading pairs on a chain.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExchangeAgentEntry {
    pub name: String,
    pub pairs: Vec<ExchangePairEntry>,
}

/// A trading pair offered by an exchange agent.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ExchangePairEntry {
    pub sell_symbol: String,
    pub buy_symbol: String,
    pub rate: f64,
}

/// Shared application state — holds all hosted chains.
pub struct AppState {
    pub chains: RwLock<HashMap<String, Arc<ChainState>>>,
    pub data_dir: Option<PathBuf>,
    pub default_blockmaker_key: SigningKey,
    /// Optional MQTT publisher for block notifications.
    mqtt: std::sync::OnceLock<mqtt::MqttPublisher>,
    /// Exchange agents registered per chain: chain_id → Vec<ExchangeAgentEntry>.
    exchange_agents: RwLock<HashMap<String, Vec<ExchangeAgentEntry>>>,
    /// Cached validator endorsements per chain: chain_id → Vec<ValidatorEndorsement>.
    validator_cache: RwLock<HashMap<String, Vec<ValidatorEndorsement>>>,
    /// Known recorder public keys for CAA proof verification: chain_id bytes → pubkey bytes.
    pub known_recorders: std::collections::HashMap<[u8; 32], [u8; 32]>,
}

/// Cached result from polling a validator's GET /validate/{chain_id}.
#[derive(Serialize, Clone, Debug)]
pub struct ValidatorEndorsement {
    pub url: String,
    pub label: Option<String>,
    pub validated_height: u64,
    pub rolled_hash: String,
    pub status: String,
    pub last_checked: i64,
}

impl AppState {
    /// Create from a single chain (backward-compatible constructor).
    pub fn new(store: ChainStore, blockmaker_key: SigningKey) -> Self {
        let meta = store.load_chain_meta()
            .expect("failed to query chain metadata")
            .expect("chain must be initialized before calling AppState::new()");
        let chain_id = hex::encode(meta.chain_id);
        let chain_state = Arc::new(ChainState::new(store, SigningKey::from_seed(blockmaker_key.seed())));
        let mut chains = HashMap::new();
        chains.insert(chain_id, chain_state);
        AppState {
            chains: RwLock::new(chains),
            data_dir: None,
            default_blockmaker_key: blockmaker_key,
            mqtt: std::sync::OnceLock::new(),
            exchange_agents: RwLock::new(HashMap::new()),
            validator_cache: RwLock::new(HashMap::new()),
            known_recorders: std::collections::HashMap::new(),
        }
    }

    /// Create an empty multi-chain host.
    pub fn new_multi(data_dir: Option<PathBuf>, blockmaker_key: SigningKey) -> Self {
        AppState {
            chains: RwLock::new(HashMap::new()),
            data_dir,
            default_blockmaker_key: blockmaker_key,
            mqtt: std::sync::OnceLock::new(),
            exchange_agents: RwLock::new(HashMap::new()),
            validator_cache: RwLock::new(HashMap::new()),
            known_recorders: std::collections::HashMap::new(),
        }
    }

    /// Set the MQTT publisher. Must be called before serving requests.
    pub fn set_mqtt(&self, publisher: mqtt::MqttPublisher) {
        let _ = self.mqtt.set(publisher);
    }

    /// Update cached validator endorsements for a chain.
    pub fn set_validator_cache(&self, chain_id: String, endorsements: Vec<ValidatorEndorsement>) {
        if let Ok(mut cache) = self.validator_cache.write() {
            cache.insert(chain_id, endorsements);
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
    #[serde(skip_serializing_if = "Vec::is_empty")]
    validators: Vec<ValidatorEndorsement>,
}

#[derive(Serialize)]
struct ChainListEntry {
    chain_id: String,
    symbol: String,
    block_height: u64,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    exchange_agents: Vec<ExchangeAgentEntry>,
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

/// Maximum request body size (256 KB). Assignments are compact wire-format
/// structures; anything larger is almost certainly malicious.
const MAX_BODY_SIZE: usize = 256 * 1024;

/// Maximum number of blocks returned by a single GET /blocks request.
const MAX_BLOCK_RANGE: u64 = 1000;

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
        .route("/chain/{id}/exchange-agent", axum::routing::post(register_exchange_agent))
        .route("/chain/{id}/refute", axum::routing::post(submit_refutation))
        .route("/chain/{id}/caa/submit", axum::routing::post(caa_submit))
        .route("/chain/{id}/caa/bind", axum::routing::post(caa_bind))
        .route("/chain/{id}/caa/{caa_hash}", get(caa_status))
        .layer(DefaultBodyLimit::max(MAX_BODY_SIZE))
        .with_state(state)
}

async fn method_not_allowed() -> StatusCode {
    StatusCode::METHOD_NOT_ALLOWED
}

/// Initialize chain state: load genesis if needed, return chain_id hex.
pub fn init_chain(store: &ChainStore, genesis_item: &DataItem) -> String {
    let meta = match store.load_chain_meta().expect("failed to query chain metadata") {
        Some(m) => m,
        None => genesis::load_genesis(store, genesis_item).expect("failed to load genesis"),
    };
    hex::encode(meta.chain_id)
}

// ── Handlers ────────────────────────────────────────────────────────

/// Run a blocking closure on the tokio blocking thread pool.
/// Wraps `spawn_blocking` with RecorderError handling.
async fn blocking<F, T>(f: F) -> Result<T, RecorderError>
where
    F: FnOnce() -> Result<T, RecorderError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| RecorderError::Internal(format!("blocking task failed: {}", e)))?
}

/// GET /chains — list all hosted chains.
async fn list_chains(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ChainListEntry>>, RecorderError> {
    // Collect chain refs under read lock (fast, non-blocking)
    let chain_refs: Vec<(String, Arc<ChainState>)> = {
        let chains = state.chains.read()
            .map_err(|e| RecorderError::LockPoisoned(format!("chains read: {}", e)))?;
        chains.iter().map(|(id, cs)| (id.clone(), Arc::clone(cs))).collect()
    };

    // Snapshot exchange agent registry
    let agents_snapshot: HashMap<String, Vec<ExchangeAgentEntry>> = {
        let agents = state.exchange_agents.read()
            .map_err(|e| RecorderError::LockPoisoned(format!("agents read: {}", e)))?;
        agents.clone()
    };

    // Query each store on the blocking pool
    let entries = blocking(move || {
        let mut entries = Vec::new();
        for (id, cs) in chain_refs {
            let store = lock_store(&cs)?;
            let meta = store.load_chain_meta()
                .map_err(|e| RecorderError::Internal(e.to_string()))?
                .ok_or(RecorderError::ChainNotLoaded)?;
            let agents = agents_snapshot.get(&id).cloned().unwrap_or_default();
            entries.push(ChainListEntry {
                chain_id: id,
                symbol: meta.symbol,
                block_height: meta.block_height,
                exchange_agents: agents,
            });
        }
        entries.sort_by(|a, b| a.chain_id.cmp(&b.chain_id));
        Ok(entries)
    }).await?;
    Ok(Json(entries))
}

/// POST /chains — create a new chain from a genesis block.
async fn create_chain(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateChainRequest>,
) -> Result<(StatusCode, Json<ChainInfo>), RecorderError> {
    // Parse and validate inputs on the async side
    let genesis_item = ao_json::from_json(&req.genesis)
        .map_err(|e| RecorderError::BadRequest(format!("invalid genesis: {}", e)))?;

    let blockmaker_key = if let Some(seed_hex) = &req.blockmaker_seed {
        let seed_bytes = hex::decode(seed_hex.trim())
            .map_err(|e| RecorderError::BadRequest(format!("invalid seed hex: {}", e)))?;
        let seed: [u8; 32] = seed_bytes.try_into()
            .map_err(|_| RecorderError::BadRequest("seed must be 32 bytes".into()))?;
        SigningKey::try_from_seed(&seed)
            .map_err(|e| RecorderError::BadRequest(format!("invalid Ed25519 seed: {}", e)))?
    } else {
        SigningKey::from_seed(state.default_blockmaker_key.seed())
    };

    let data_dir = state.data_dir.clone();

    // SQLite work on the blocking pool
    let (store, info, chain_id_hex) = blocking(move || {
        let store = if let Some(dir) = &data_dir {
            // Extract chain ID directly from genesis DataItem (no temp store needed)
            let chain_id = genesis::compute_chain_id(&genesis_item)
                .map_err(|e| RecorderError::BadRequest(format!("genesis error: {}", e)))?;
            let chain_id_hex = hex::encode(chain_id);

            let db_path = dir.join(format!("{}.db", chain_id_hex));
            let db_str = db_path.to_str()
                .ok_or_else(|| RecorderError::Internal("non-UTF-8 database path".into()))?;
            let file_store = ChainStore::open(db_str)
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            file_store.init_schema()
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            genesis::load_genesis(&file_store, &genesis_item)
                .map_err(|e| RecorderError::BadRequest(format!("genesis error: {}", e)))?;
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
            validators: Vec::new(),
        };

        Ok((store, info, chain_id_hex))
    }).await?;

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

    // Snapshot validator cache for this chain
    let validators = state.validator_cache.read()
        .map_err(|e| RecorderError::LockPoisoned(format!("validator cache read: {}", e)))?
        .get(&chain_id_hex)
        .cloned()
        .unwrap_or_default();

    let info = blocking(move || {
        let store = lock_store(&chain)?;
        let meta = store.load_chain_meta()
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or(RecorderError::ChainNotLoaded)?;
        Ok(ChainInfo {
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
            validators,
        })
    }).await?;
    Ok(Json(info))
}

/// GET /chain/{id}/utxo/{seq_id}
async fn get_utxo(
    State(state): State<Arc<AppState>>,
    Path((chain_id_hex, seq_id)): Path<(String, u64)>,
) -> Result<Json<UtxoInfo>, RecorderError> {
    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let info = blocking(move || {
        let store = lock_store(&chain)?;
        let utxo = store.get_utxo(seq_id)
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or_else(|| RecorderError::NotFound("UTXO not found".into()))?;
        Ok(UtxoInfo {
            seq_id: utxo.seq_id,
            pubkey: hex::encode(utxo.pubkey),
            amount: utxo.amount.to_string(),
            block_height: utxo.block_height,
            block_timestamp: utxo.block_timestamp,
            status: format!("{:?}", utxo.status),
        })
    }).await?;
    Ok(Json(info))
}

/// GET /chain/{id}/blocks?from={height}&to={height}
async fn get_blocks(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<serde_json::Value>>, RecorderError> {
    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let from: u64 = params.get("from").and_then(|s| s.parse().ok()).unwrap_or(0);
    let requested_to: Option<u64> = params.get("to").and_then(|s| s.parse().ok());

    let blocks = blocking(move || {
        let store = lock_store(&chain)?;
        let meta = store.load_chain_meta()
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or(RecorderError::ChainNotLoaded)?;

        let to = requested_to.unwrap_or(meta.block_height);
        // Cap range to prevent loading entire chain into memory
        let to = to.min(from.saturating_add(MAX_BLOCK_RANGE - 1));

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
        Ok(blocks)
    }).await?;

    Ok(Json(blocks))
}

/// POST /chain/{id}/submit
async fn submit_assignment(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    body: String,
) -> Result<Json<BlockInfo>, RecorderError> {
    // Parse JSON on the async side (CPU-only, no blocking I/O)
    let json_value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| RecorderError::BadRequest(format!("invalid JSON: {}", e)))?;
    let authorization = ao_json::from_json(&json_value)
        .map_err(|e| RecorderError::BadRequest(format!("invalid DataItem: {}", e)))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;
    let wall_ts = Timestamp::try_from_unix_seconds(now)
        .ok_or_else(|| RecorderError::Internal("system clock out of AO timestamp range".into()))?
        .raw();

    let chain = state.get_chain_or_err(&chain_id_hex)?;

    // All SQLite work runs on the blocking pool
    let info = blocking(move || {
        let store = lock_store(&chain)?;
        let meta = store.load_chain_meta()
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or(RecorderError::ChainNotLoaded)?;

        // Ensure block timestamp is strictly greater than previous block.
        // Wall clock has second resolution; multiple submissions within the
        // same second need monotonically increasing timestamps.
        let current_ts = wall_ts.max(meta.last_block_timestamp + 1);

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
        Ok(info)
    }).await?;

    // MQTT publish (non-blocking, fire-and-forget)
    if let Some(mqtt) = state.mqtt.get() {
        mqtt.publish_block(&chain_id_hex, &info).await;
    }

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

/// POST /chain/{id}/exchange-agent — register an exchange agent for a chain.
async fn register_exchange_agent(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    Json(entry): Json<ExchangeAgentEntry>,
) -> Result<StatusCode, RecorderError> {
    // Verify chain exists
    let _chain = state.get_chain_or_err(&chain_id_hex)?;

    let mut agents = state.exchange_agents.write()
        .map_err(|e| RecorderError::LockPoisoned(format!("agents write: {}", e)))?;
    let chain_agents = agents.entry(chain_id_hex).or_default();

    // Replace existing entry with same name, or add new
    if let Some(existing) = chain_agents.iter_mut().find(|a| a.name == entry.name) {
        *existing = entry;
    } else {
        chain_agents.push(entry);
    }

    Ok(StatusCode::OK)
}

// ── Refutation ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RefutationRequest {
    /// SHA2-256 hash of the ASSIGNMENT being refuted (hex).
    agreement_hash: String,
}

/// POST /chain/{id}/refute — record a refutation for an agreement.
///
/// A refutation prevents late recording of an agreement whose deadline has
/// passed. It does not affect agreements submitted before their deadline.
/// Refutations are idempotent — re-submitting the same hash is a no-op.
async fn submit_refutation(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    Json(req): Json<RefutationRequest>,
) -> Result<StatusCode, RecorderError> {
    let hash_bytes = hex::decode(req.agreement_hash.trim())
        .map_err(|e| RecorderError::BadRequest(format!("invalid agreement_hash hex: {}", e)))?;
    if hash_bytes.len() != 32 {
        return Err(RecorderError::BadRequest("agreement_hash must be 32 bytes".into()));
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&hash_bytes);

    let chain = state.get_chain_or_err(&chain_id_hex)?;

    blocking(move || {
        let store = lock_store(&chain)?;
        store.add_refutation(&hash)
            .map_err(|e| RecorderError::Internal(e.to_string()))?;
        Ok(StatusCode::OK)
    }).await
}

// ── CAA types ────────────────────────────────────────────────────────

#[derive(Serialize)]
struct CaaStatusResponse {
    caa_hash: String,
    status: String,
    chain_order: u64,
    deadline: i64,
    block_height: u64,
    has_proof: bool,
}

#[derive(Serialize)]
struct RecordingProofResponse {
    caa_hash: String,
    chain_id: String,
    block_height: u64,
    block_hash: String,
    proof_json: serde_json::Value,
}

// ── CAA Handlers ─────────────────────────────────────────────────────

/// POST /chain/{id}/caa/submit — submit a CAA for escrow recording.
async fn caa_submit(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    body: String,
) -> Result<Json<RecordingProofResponse>, RecorderError> {
    let json_value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| RecorderError::BadRequest(format!("invalid JSON: {}", e)))?;
    let caa_item = ao_json::from_json(&json_value)
        .map_err(|e| RecorderError::BadRequest(format!("invalid DataItem: {}", e)))?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;
    let wall_ts = Timestamp::try_from_unix_seconds(now)
        .ok_or_else(|| RecorderError::Internal("system clock out of AO timestamp range".into()))?
        .raw();

    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let known_recorders = state.known_recorders.clone();

    let response = blocking(move || {
        let store = lock_store(&chain)?;
        let meta = store.load_chain_meta()
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or(RecorderError::ChainNotLoaded)?;

        let current_ts = wall_ts.max(meta.last_block_timestamp + 1);

        // Check for idempotent re-submission
        let caa_hash = caa::compute_caa_hash(&caa_item);
        if let Some(existing) = store.get_caa_escrow(&caa_hash)
            .map_err(|e| RecorderError::Internal(e.to_string()))?
        {
            if let Some(proof_data) = &existing.proof_data {
                let proof_item = ao_types::dataitem::DataItem::from_bytes(proof_data)
                    .map_err(|e| RecorderError::Internal(format!("corrupt proof data: {}", e)))?;
                let proof_json = ao_json::to_json(&proof_item);
                let bh = store.get_block_hash(existing.block_height)
                    .map_err(|e| RecorderError::Internal(e.to_string()))?
                    .unwrap_or([0u8; 32]);
                return Ok(RecordingProofResponse {
                    caa_hash: hex::encode(caa_hash),
                    chain_id: hex::encode(meta.chain_id),
                    block_height: existing.block_height,
                    block_hash: hex::encode(bh),
                    proof_json,
                });
            }
            return Err(RecorderError::BadRequest("CAA already recorded but proof not available".into()));
        }

        // Validate the CAA
        let validated = caa::validate_caa_submit(&store, &meta, &caa_item, current_ts, &known_recorders)
            .map_err(|e| RecorderError::BadRequest(e.to_string()))?;

        // All mutations in a transaction
        store.begin_transaction()
            .map_err(|e| RecorderError::Internal(e.to_string()))?;

        let result = (|| -> Result<RecordingProofResponse, RecorderError> {
            use ao_types::typecode::*;
            use ao_types::bigint;

            let height = meta.block_height + 1;
            let mut next_seq = meta.next_seq_id;
            let first_seq = next_seq;

            // Escrow giver UTXOs
            for (seq_id, _) in &validated.givers {
                store.mark_escrowed(*seq_id)
                    .map_err(|e| RecorderError::Internal(e.to_string()))?;
            }

            // Create receiver UTXOs as escrowed
            for (pk, amount) in &validated.receivers {
                store.insert_utxo(&ao_chain::store::Utxo {
                    seq_id: next_seq,
                    pubkey: *pk,
                    amount: amount.clone(),
                    block_height: height,
                    block_timestamp: current_ts,
                    status: ao_chain::store::UtxoStatus::Escrowed,
                }).map_err(|e| RecorderError::Internal(e.to_string()))?;
                store.mark_key_used(pk)
                    .map_err(|e| RecorderError::Internal(e.to_string()))?;
                next_seq += 1;
            }
            let seq_count = next_seq - first_seq;

            // Record CAA escrow with total_chains
            store.insert_caa_escrow(
                &validated.caa_hash,
                validated.chain_order,
                validated.escrow_deadline,
                height,
                None,
                validated.total_chains,
            ).map_err(|e| RecorderError::Internal(e.to_string()))?;

            // Record UTXO associations
            for (seq_id, _) in &validated.givers {
                store.insert_caa_utxo(&validated.caa_hash, *seq_id, "giver")
                    .map_err(|e| RecorderError::Internal(e.to_string()))?;
            }
            let mut recv_seq = meta.next_seq_id;
            for _ in &validated.receivers {
                store.insert_caa_utxo(&validated.caa_hash, recv_seq, "receiver")
                    .map_err(|e| RecorderError::Internal(e.to_string()))?;
                recv_seq += 1;
            }

            // Construct a real block containing the CAA assignment as a page
            let fee_deducted_shares = &meta.shares_out - &validated.fee_shares;
            let mut shares_bytes = Vec::new();
            bigint::encode_bigint(&fee_deducted_shares, &mut shares_bytes);

            let page = DataItem::container(PAGE, vec![
                DataItem::vbc_value(PAGE_INDEX, 0),
                DataItem::container(AUTHORIZATION, vec![validated.assignment.clone()]),
            ]);

            let block_contents = DataItem::container(BLOCK_CONTENTS, vec![
                DataItem::bytes(PREV_HASH, meta.prev_hash.to_vec()),
                DataItem::vbc_value(FIRST_SEQ, first_seq),
                DataItem::vbc_value(SEQ_COUNT, seq_count),
                DataItem::vbc_value(LIST_SIZE, 1u64),
                DataItem::bytes(SHARES_OUT, shares_bytes),
                page,
            ]);

            let ts = Timestamp::from_raw(current_ts);
            let sig = ao_crypto::sign::sign_dataitem(&chain.blockmaker_key, &block_contents, ts);
            let block_signed = DataItem::container(BLOCK_SIGNED, vec![
                block_contents,
                DataItem::container(AUTH_SIG, vec![
                    DataItem::bytes(ED25519_SIG, sig.to_vec()),
                    DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                    DataItem::bytes(ED25519_PUB, chain.blockmaker_key.public_key_bytes().to_vec()),
                ]),
            ]);

            let block_signed_bytes = block_signed.to_bytes();
            let block_hash = ao_crypto::hash::sha256(&block_signed_bytes);

            let block = DataItem::container(BLOCK, vec![
                block_signed,
                DataItem::bytes(SHA256, block_hash.to_vec()),
            ]);
            let block_bytes = block.to_bytes();

            // Store block and advance chain state
            store.store_block(height, current_ts, &block_hash, &block_bytes)
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            store.advance_block(height, current_ts, &block_hash)
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            store.update_shares_out(&fee_deducted_shares)
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            store.set_next_seq_id(next_seq)
                .map_err(|e| RecorderError::Internal(e.to_string()))?;

            // Build recording proof with real block hash
            let proof = build_recording_proof(
                &meta.chain_id,
                height,
                &block_hash,
                &validated.caa_hash,
                &chain.blockmaker_key,
                current_ts,
            );
            let proof_bytes = proof.to_bytes();
            let proof_json = ao_json::to_json(&proof);

            store.set_caa_proof(&validated.caa_hash, &proof_bytes)
                .map_err(|e| RecorderError::Internal(e.to_string()))?;

            Ok(RecordingProofResponse {
                caa_hash: hex::encode(validated.caa_hash),
                chain_id: hex::encode(meta.chain_id),
                block_height: height,
                block_hash: hex::encode(block_hash),
                proof_json,
            })
        })();

        match &result {
            Ok(_) => store.commit().map_err(|e| RecorderError::Internal(e.to_string()))?,
            Err(_) => { let _ = store.rollback(); }
        }
        result
    }).await?;

    Ok(Json(response))
}

/// POST /chain/{id}/caa/bind — submit binding proof to finalize a CAA.
async fn caa_bind(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    body: String,
) -> Result<Json<CaaStatusResponse>, RecorderError> {
    let json_value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|e| RecorderError::BadRequest(format!("invalid JSON: {}", e)))?;

    // Parse the binding submission: expect caa_hash and proofs array
    let caa_hash_hex = json_value.get("caa_hash")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RecorderError::BadRequest("missing caa_hash".into()))?;
    let caa_hash_bytes = hex::decode(caa_hash_hex)
        .map_err(|e| RecorderError::BadRequest(format!("invalid caa_hash hex: {}", e)))?;
    if caa_hash_bytes.len() != 32 {
        return Err(RecorderError::BadRequest("caa_hash must be 32 bytes".into()));
    }
    let mut caa_hash = [0u8; 32];
    caa_hash.copy_from_slice(&caa_hash_bytes);

    let proofs_json = json_value.get("proofs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RecorderError::BadRequest("missing proofs array".into()))?;
    let mut proofs = Vec::new();
    for pj in proofs_json {
        let proof_item = ao_json::from_json(pj)
            .map_err(|e| RecorderError::BadRequest(format!("invalid proof DataItem: {}", e)))?;
        proofs.push(proof_item);
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;
    let wall_ts = Timestamp::try_from_unix_seconds(now)
        .ok_or_else(|| RecorderError::Internal("system clock out of AO timestamp range".into()))?
        .raw();

    let chain = state.get_chain_or_err(&chain_id_hex)?;
    let known_recorders = state.known_recorders.clone();

    let response = blocking(move || {
        let store = lock_store(&chain)?;

        let current_ts = wall_ts;

        // Validate the binding
        caa::validate_caa_bind(&store, &caa_hash, &proofs, current_ts, &known_recorders)
            .map_err(|e| RecorderError::BadRequest(e.to_string()))?;

        store.begin_transaction()
            .map_err(|e| RecorderError::Internal(e.to_string()))?;

        let result = (|| -> Result<CaaStatusResponse, RecorderError> {
            // Transition giver UTXOs from escrowed to spent
            let giver_ids = store.get_caa_utxo_ids(&caa_hash, "giver")
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            for seq_id in &giver_ids {
                store.mark_escrowed_spent(*seq_id)
                    .map_err(|e| RecorderError::Internal(e.to_string()))?;
            }

            // Transition receiver UTXOs from escrowed to unspent (now spendable)
            let receiver_ids = store.get_caa_utxo_ids(&caa_hash, "receiver")
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            for seq_id in &receiver_ids {
                store.release_escrow(*seq_id)
                    .map_err(|e| RecorderError::Internal(e.to_string()))?;
            }

            // Update CAA status to finalized
            store.update_caa_status(&caa_hash, "finalized")
                .map_err(|e| RecorderError::Internal(e.to_string()))?;

            let escrow = store.get_caa_escrow(&caa_hash)
                .map_err(|e| RecorderError::Internal(e.to_string()))?
                .ok_or(RecorderError::Internal("escrow disappeared".into()))?;

            Ok(CaaStatusResponse {
                caa_hash: hex::encode(caa_hash),
                status: "finalized".to_string(),
                chain_order: escrow.chain_order,
                deadline: escrow.deadline,
                block_height: escrow.block_height,
                has_proof: escrow.proof_data.is_some(),
            })
        })();

        match &result {
            Ok(_) => store.commit().map_err(|e| RecorderError::Internal(e.to_string()))?,
            Err(_) => { let _ = store.rollback(); }
        }
        result
    }).await?;

    Ok(Json(response))
}

/// GET /chain/{id}/caa/{caa_hash} — query CAA escrow status.
async fn caa_status(
    State(state): State<Arc<AppState>>,
    Path((chain_id_hex, caa_hash_hex)): Path<(String, String)>,
) -> Result<Json<CaaStatusResponse>, RecorderError> {
    let caa_hash_bytes = hex::decode(&caa_hash_hex)
        .map_err(|e| RecorderError::BadRequest(format!("invalid caa_hash hex: {}", e)))?;
    if caa_hash_bytes.len() != 32 {
        return Err(RecorderError::BadRequest("caa_hash must be 32 bytes".into()));
    }
    let mut caa_hash = [0u8; 32];
    caa_hash.copy_from_slice(&caa_hash_bytes);

    let chain = state.get_chain_or_err(&chain_id_hex)?;

    let response = blocking(move || {
        let store = lock_store(&chain)?;
        let escrow = store.get_caa_escrow(&caa_hash)
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or_else(|| RecorderError::NotFound("CAA not found".into()))?;

        Ok(CaaStatusResponse {
            caa_hash: hex::encode(caa_hash),
            status: escrow.status,
            chain_order: escrow.chain_order,
            deadline: escrow.deadline,
            block_height: escrow.block_height,
            has_proof: escrow.proof_data.is_some(),
        })
    }).await?;

    Ok(Json(response))
}

/// Build a RECORDING_PROOF DataItem signed by the recorder.
fn build_recording_proof(
    chain_id: &[u8; 32],
    block_height: u64,
    block_hash: &[u8; 32],
    caa_hash: &[u8; 32],
    blockmaker_key: &SigningKey,
    timestamp: i64,
) -> DataItem {
    use ao_types::typecode::*;
    use ao_crypto::sign;

    let proof_content = vec![
        DataItem::bytes(CHAIN_REF, chain_id.to_vec()),
        DataItem::container(BLOCK_REF, vec![
            DataItem::bytes(CHAIN_REF, chain_id.to_vec()),
            DataItem::vbc_value(BLOCK_HEIGHT, block_height),
            DataItem::bytes(SHA256, block_hash.to_vec()),
        ]),
        DataItem::bytes(CAA_HASH, caa_hash.to_vec()),
    ];

    let proof_to_sign = DataItem::container(RECORDING_PROOF, proof_content.clone());
    let ts = Timestamp::from_raw(timestamp);
    let sig = sign::sign_dataitem(blockmaker_key, &proof_to_sign, ts);

    let mut all_children = proof_content;
    all_children.push(DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, blockmaker_key.public_key_bytes().to_vec()),
    ]));

    DataItem::container(RECORDING_PROOF, all_children)
}

/// Background task: poll configured validators and update the endorsement cache.
/// Runs every 60 seconds. Each validator is queried for each hosted chain.
pub async fn poll_validators(state: Arc<AppState>, validators: Vec<config::ValidatorEndpoint>) {
    let http = reqwest::Client::new();
    let interval = std::time::Duration::from_secs(60);

    loop {
        // Snapshot current chain IDs
        let chain_ids: Vec<String> = match state.chains.read() {
            Ok(chains) => chains.keys().cloned().collect(),
            Err(_) => Vec::new(),
        };

        for chain_id in &chain_ids {
            let mut endorsements = Vec::new();
            for v in &validators {
                let url = format!("{}/validate/{}", v.url.trim_end_matches('/'), chain_id);
                let result = http.get(&url).timeout(std::time::Duration::from_secs(10)).send().await;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64;

                match result {
                    Ok(resp) if resp.status().is_success() => {
                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                            endorsements.push(ValidatorEndorsement {
                                url: v.url.clone(),
                                label: v.label.clone(),
                                validated_height: body.get("validated_height")
                                    .and_then(|v| v.as_u64()).unwrap_or(0),
                                rolled_hash: body.get("rolled_hash")
                                    .and_then(|v| v.as_str()).unwrap_or("").to_string(),
                                status: body.get("status")
                                    .and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                                last_checked: now,
                            });
                        }
                    }
                    Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => {
                        // Validator doesn't track this chain — skip silently
                    }
                    _ => {
                        endorsements.push(ValidatorEndorsement {
                            url: v.url.clone(),
                            label: v.label.clone(),
                            validated_height: 0,
                            rolled_hash: String::new(),
                            status: "unreachable".to_string(),
                            last_checked: now,
                        });
                    }
                }
            }
            state.set_validator_cache(chain_id.clone(), endorsements);
        }

        tokio::time::sleep(interval).await;
    }
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
