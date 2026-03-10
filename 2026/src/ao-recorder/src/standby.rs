//! Hot standby: sync blocks from a primary recorder.
//!
//! When `[standby]` is configured, the recorder runs in read-only mode:
//! - POST/PUT/DELETE requests return 503 Service Unavailable
//! - Blocks are fetched from the primary via GET /chain/{id}/blocks
//! - New blocks are streamed via SSE (GET /chain/{id}/events)
//!
//! To promote to primary: stop standby, remove `[standby]` from config, restart.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use serde::Deserialize;
use tracing::{info, warn, error};

use ao_types::json as ao_json;
use ao_chain::store::ChainStore;

use crate::config::StandbyConfig;
use crate::AppState;

/// Shared sync state for health reporting.
pub struct StandbySyncState {
    /// Whether the SSE connection to the primary is currently active.
    pub sse_connected: AtomicBool,
    /// Highest block height synced across all chains.
    pub synced_height: AtomicU64,
    /// Primary recorder URL (for health reporting).
    pub primary_url: String,
    /// Total blocks synced since startup.
    pub blocks_synced: AtomicU64,
}

impl StandbySyncState {
    pub fn new(primary_url: String) -> Self {
        StandbySyncState {
            sse_connected: AtomicBool::new(false),
            synced_height: AtomicU64::new(0),
            primary_url,
            blocks_synced: AtomicU64::new(0),
        }
    }
}

/// Chain info returned by the primary's GET /chains endpoint.
#[derive(Deserialize)]
#[allow(dead_code)]
struct PrimaryChainEntry {
    chain_id: String,
    symbol: String,
    block_height: u64,
}

/// Chain info returned by the primary's GET /chain/{id}/info endpoint.
#[derive(Deserialize)]
struct PrimaryChainInfo {
    block_height: u64,
}

/// Run the standby sync loop. This is a long-running task that:
/// 1. Discovers chains from the primary
/// 2. Performs initial block sync (batch fetch)
/// 3. Subscribes to SSE for live block updates
///
/// Reconnects automatically on failure with the configured delay.
pub async fn run_standby_sync(
    state: Arc<AppState>,
    cfg: StandbyConfig,
) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let primary_url = cfg.primary_url.trim_end_matches('/').to_string();
    let reconnect_delay = Duration::from_secs(cfg.reconnect_delay_seconds);
    let batch_size = cfg.sync_batch_size;

    loop {
        info!(primary = %primary_url, "Starting standby sync cycle");

        match sync_cycle(&state, &client, &primary_url, batch_size).await {
            Ok(()) => {
                warn!("Standby SSE stream ended, reconnecting...");
            }
            Err(e) => {
                error!("Standby sync error: {}, reconnecting in {}s...", e, cfg.reconnect_delay_seconds);
            }
        }

        if let Some(ref sync_state) = state.standby_sync {
            sync_state.sse_connected.store(false, Ordering::Relaxed);
        }

        tokio::time::sleep(reconnect_delay).await;
    }
}

/// One full sync cycle: discover chains, batch sync, then SSE.
async fn sync_cycle(
    state: &Arc<AppState>,
    client: &reqwest::Client,
    primary_url: &str,
    batch_size: u64,
) -> Result<(), anyhow::Error> {
    // 1. Discover chains from primary
    let chains_url = format!("{}/chains", primary_url);
    let chains: Vec<PrimaryChainEntry> = client.get(&chains_url)
        .send().await?
        .error_for_status()?
        .json().await?;

    if chains.is_empty() {
        info!("Primary has no chains yet, waiting...");
        return Ok(());
    }

    info!(count = chains.len(), "Discovered chains from primary");

    // Get identity header for authenticated streaming sync
    let identity_header = state.recorder_identity
        .as_ref()
        .map(|v| v.to_string());
    let identity_ref = identity_header.as_deref();

    // 2. For each chain, ensure local DB exists and sync blocks (streaming preferred, batch fallback)
    for chain in &chains {
        ensure_chain_db(state, &chain.chain_id).await?;
        if let Err(e) = streaming_or_batch_sync(state, client, primary_url, &chain.chain_id, batch_size, identity_ref).await {
            warn!(chain_id = %chain.chain_id, "Sync failed (continuing): {}", e);
        }
    }

    // 3. Subscribe to SSE for live updates on all chains concurrently.
    // Each chain gets its own SSE task. We wait for any to finish (error/disconnect).
    let mut handles = Vec::new();
    for chain in &chains {
        let state = Arc::clone(state);
        let client = client.clone();
        let primary_url = primary_url.to_string();
        let chain_id = chain.chain_id.clone();
        let handle = tokio::spawn(async move {
            sse_sync_chain(&state, &client, &primary_url, &chain_id, batch_size).await
        });
        handles.push(handle);
    }

    // Wait for any SSE task to complete (they run indefinitely until error)
    if !handles.is_empty() {
        let (result, _, remaining) = futures_util::future::select_all(handles).await;
        // Cancel remaining SSE tasks
        for h in remaining {
            h.abort();
        }
        // Propagate the error from the first completed task
        match result {
            Ok(Ok(())) => {} // SSE ended cleanly
            Ok(Err(e)) => return Err(e),
            Err(e) => return Err(anyhow::anyhow!("SSE task panicked: {}", e)),
        }
    }

    Ok(())
}

/// Ensure a local chain database exists. If the chain isn't loaded yet,
/// create an empty DB and register it in AppState. The genesis block will
/// arrive as block 0 during batch sync.
async fn ensure_chain_db(
    state: &Arc<AppState>,
    chain_id: &str,
) -> Result<(), anyhow::Error> {
    // Already loaded?
    if state.get_chain_or_err(chain_id).is_ok() {
        return Ok(());
    }

    // Create a new DB for this chain
    let store = if let Some(ref dir) = state.data_dir {
        let db_path = dir.join(format!("{}.db", chain_id));
        let db_str = db_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 database path"))?;
        let s = ChainStore::open(db_str)?;
        s.init_schema()?;
        s
    } else {
        let s = ChainStore::open_memory()?;
        s.init_schema()?;
        s
    };

    let chain_state = Arc::new(crate::ChainState::new(
        store,
        ao_crypto::sign::SigningKey::from_seed(state.default_blockmaker_key.seed()),
    ));
    state.add_chain(chain_id.to_string(), chain_state);
    info!(chain_id = %chain_id, "Created standby chain database");
    Ok(())
}

/// Try streaming sync first (NDJSON from /chain/{id}/sync), fall back to batch.
async fn streaming_or_batch_sync(
    state: &Arc<AppState>,
    client: &reqwest::Client,
    primary_url: &str,
    chain_id: &str,
    batch_size: u64,
    identity_header: Option<&str>,
) -> Result<(), anyhow::Error> {
    // Determine local starting height
    let chain = state.get_chain_or_err(chain_id)
        .map_err(|_| anyhow::anyhow!("chain {} not found in AppState", chain_id))?;
    let (has_genesis, local_height) = {
        let store = chain.store.lock()
            .map_err(|e| anyhow::anyhow!("store lock: {}", e))?;
        match store.load_chain_meta()? {
            Some(m) => (true, m.block_height),
            None => (false, 0),
        }
    };
    let from = if !has_genesis { 0 } else { local_height + 1 };

    // Try streaming endpoint
    let sync_url = format!("{}/chain/{}/sync?from={}", primary_url, chain_id, from);
    let mut req = client.get(&sync_url);
    if let Some(identity) = identity_header {
        req = req.header("X-Recorder-Identity", identity);
    }

    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await?;
            if body.trim().is_empty() {
                return Ok(());
            }

            // Parse NDJSON lines outside the lock (CPU-bound only)
            let mut parsed_blocks: Vec<(u64, ao_types::dataitem::DataItem, Vec<u8>, [u8; 32])> = Vec::new();
            for (i, line) in body.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let wrapper: serde_json::Value = serde_json::from_str(line)
                    .map_err(|e| anyhow::anyhow!("NDJSON line {} decode: {}", i, e))?;
                // Parse {"height": N, "block": <DataItem JSON>} wrapper
                let height = wrapper.get("height")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| anyhow::anyhow!("NDJSON line {} missing height", i))?;
                let block_json = wrapper.get("block")
                    .ok_or_else(|| anyhow::anyhow!("NDJSON line {} missing block", i))?;
                let item = ao_types::json::from_json(block_json)
                    .map_err(|e| anyhow::anyhow!("NDJSON line {} DataItem: {}", i, e))?;
                let data = item.to_bytes();
                let hash = ao_crypto::hash::sha256(&data);
                parsed_blocks.push((height, item, data, hash));
            }

            if parsed_blocks.is_empty() {
                return Ok(());
            }

            // Store blocks under the lock (brief hold per block)
            let chain_clone = Arc::clone(&chain);
            let state_clone = Arc::clone(state);
            tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
                let store = chain_clone.store.lock()
                    .map_err(|e| anyhow::anyhow!("store lock: {}", e))?;
                let mut count = 0u64;
                let mut max_height = 0u64;
                for (height, item, data, hash) in &parsed_blocks {
                    if *height == 0 {
                        ao_chain::genesis::load_genesis(&store, item)
                            .map_err(|e| anyhow::anyhow!("genesis load error: {}", e))?;
                    } else {
                        let prev_hash = store.load_chain_meta()?
                            .map(|m| m.prev_hash);
                        verify_synced_block(item, hash, prev_hash.as_ref())
                            .map_err(|e| anyhow::anyhow!("block {} integrity: {}", height, e))?;
                        let ts = extract_block_timestamp(item);
                        store.store_block(*height, ts, hash, data)?;
                        store.advance_block(*height, ts, hash)?;
                    }
                    count += 1;
                    max_height = *height;
                }
                if count > 0 {
                    if let Some(ref sync_state) = state_clone.standby_sync {
                        sync_state.synced_height.fetch_max(max_height, Ordering::Relaxed);
                        sync_state.blocks_synced.fetch_add(count, Ordering::Relaxed);
                    }
                }
                Ok(())
            }).await??;
            info!(chain_id = %chain_id, "Streaming sync complete");
            return Ok(());
        }
        Ok(resp) => {
            // Non-success (404, 401, etc.) — fall back to batch
            info!(chain_id = %chain_id, status = %resp.status(), "Streaming sync unavailable, falling back to batch");
        }
        Err(e) => {
            info!(chain_id = %chain_id, "Streaming sync failed ({}), falling back to batch", e);
        }
    }

    // Fallback to batch sync
    batch_sync_chain(state, client, primary_url, chain_id, batch_size).await
}

/// Batch-sync blocks from primary for a single chain.
/// Fetches blocks in batches of `batch_size` until caught up.
async fn batch_sync_chain(
    state: &Arc<AppState>,
    client: &reqwest::Client,
    primary_url: &str,
    chain_id: &str,
    batch_size: u64,
) -> Result<(), anyhow::Error> {
    let chain = state.get_chain_or_err(chain_id)
        .map_err(|_| anyhow::anyhow!("chain {} not found in AppState", chain_id))?;

    // Get local state: distinguish "no genesis" from "genesis at height 0"
    let (has_genesis, local_height) = {
        let store = chain.store.lock()
            .map_err(|e| anyhow::anyhow!("store lock: {}", e))?;
        match store.load_chain_meta()? {
            Some(m) => (true, m.block_height),
            None => (false, 0),
        }
    };

    // Get primary height
    let info_url = format!("{}/chain/{}/info", primary_url, chain_id);
    let primary_info: PrimaryChainInfo = client.get(&info_url)
        .send().await?
        .error_for_status()?
        .json().await?;

    let from = if !has_genesis {
        0 // Need to start from genesis
    } else if local_height >= primary_info.block_height {
        info!(chain_id = %chain_id, height = local_height, "Chain already synced");
        return Ok(());
    } else {
        local_height + 1
    };

    info!(
        chain_id = %chain_id,
        local = local_height,
        primary = primary_info.block_height,
        has_genesis = has_genesis,
        "Starting batch sync"
    );

    let mut current_from = from;

    while current_from <= primary_info.block_height {
        let to = (current_from + batch_size - 1).min(primary_info.block_height);
        let blocks_url = format!("{}/chain/{}/blocks?from={}&to={}", primary_url, chain_id, current_from, to);

        let blocks: Vec<serde_json::Value> = client.get(&blocks_url)
            .send().await?
            .error_for_status()?
            .json().await?;

        if blocks.is_empty() {
            break;
        }

        let blocks_count = blocks.len();
        let chain_clone = Arc::clone(&chain);
        let state_clone = Arc::clone(state);
        let batch_from = current_from;

        // Store blocks on the blocking pool (M1 fix: don't hold std::sync::Mutex in async)
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let store = chain_clone.store.lock()
                .map_err(|e| anyhow::anyhow!("store lock: {}", e))?;

            for (i, block_json) in blocks.into_iter().enumerate() {
                let height = batch_from + i as u64;
                let item = ao_json::from_json(&block_json)
                    .map_err(|e| anyhow::anyhow!("block {} decode error: {}", height, e))?;
                let data = item.to_bytes();
                let hash = ao_crypto::hash::sha256(&data);

                if height == 0 {
                    // Genesis: use load_genesis which handles block storage + metadata + UTXOs
                    ao_chain::genesis::load_genesis(&store, &item)
                        .map_err(|e| anyhow::anyhow!("genesis load error: {}", e))?;
                } else {
                    // Verify block integrity before storing
                    let prev_hash = store.load_chain_meta()?
                        .map(|m| m.prev_hash);
                    verify_synced_block(&item, &hash, prev_hash.as_ref())
                        .map_err(|e| anyhow::anyhow!("block {} integrity: {}", height, e))?;
                    let ts = extract_block_timestamp(&item);
                    store.store_block(height, ts, &hash, &data)?;
                    store.advance_block(height, ts, &hash)?;
                }
            }

            if let Some(ref sync_state) = state_clone.standby_sync {
                let new_height = batch_from + blocks_count as u64 - 1;
                sync_state.synced_height.fetch_max(new_height, Ordering::Relaxed);
                sync_state.blocks_synced.fetch_add(blocks_count as u64, Ordering::Relaxed);
            }

            Ok(())
        }).await??;

        info!(chain_id = %chain_id, from = current_from, count = blocks_count, "Synced block batch");
        current_from += blocks_count as u64;
    }

    Ok(())
}

/// Verify a synced block's structural integrity:
/// - Declared hash matches recomputed hash
/// - prev_hash matches the chain's last known block hash
/// - Blockmaker Ed25519 signature is valid
///
/// This does NOT verify assignment-level validity (UTXO existence, balances, etc.)
/// because standby nodes don't replicate full UTXO state — only the block chain.
fn verify_synced_block(
    item: &ao_types::dataitem::DataItem,
    computed_hash: &[u8; 32],
    expected_prev_hash: Option<&[u8; 32]>,
) -> Result<(), anyhow::Error> {
    use ao_types::typecode::*;

    // A BLOCK contains [BLOCK_SIGNED, SHA256].
    // The hash in the SHA256 child must match our computed hash.
    let mut block_signed_opt = None;
    let mut declared_hash_opt = None;
    for child in item.children() {
        if child.type_code == BLOCK_SIGNED {
            block_signed_opt = Some(child);
        } else if child.type_code == SHA256 {
            declared_hash_opt = child.as_bytes();
        }
    }
    let block_signed = block_signed_opt
        .ok_or_else(|| anyhow::anyhow!("block missing BLOCK_SIGNED"))?;
    if let Some(declared) = declared_hash_opt {
        // The hash stored in the block vs our computation of BLOCK_SIGNED encoding
        let block_signed_bytes = block_signed.to_bytes();
        let recomputed = ao_crypto::hash::sha256(&block_signed_bytes);
        if recomputed != *computed_hash {
            return Err(anyhow::anyhow!("block hash mismatch: computed vs outer"));
        }
        if declared.len() == 32 && declared != computed_hash.as_slice() {
            return Err(anyhow::anyhow!("declared SHA256 != computed hash"));
        }
    }

    // BLOCK_SIGNED contains [BLOCK_CONTENTS, AUTH_SIG].
    let mut block_contents_opt = None;
    let mut auth_sig_opt = None;
    for child in block_signed.children() {
        if child.type_code == BLOCK_CONTENTS {
            block_contents_opt = Some(child);
        } else if child.type_code == AUTH_SIG {
            auth_sig_opt = Some(child);
        }
    }
    let block_contents = block_contents_opt
        .ok_or_else(|| anyhow::anyhow!("block missing BLOCK_CONTENTS"))?;

    // Check prev_hash if we have an expected value
    if let Some(expected) = expected_prev_hash {
        for child in block_contents.children() {
            if child.type_code == PREV_HASH {
                if let Some(prev) = child.as_bytes() {
                    if prev.len() == 32 && prev != expected.as_slice() {
                        return Err(anyhow::anyhow!(
                            "prev_hash mismatch: expected {}, got {}",
                            hex::encode(expected), hex::encode(prev)
                        ));
                    }
                }
                break;
            }
        }
    }

    // Verify blockmaker signature
    if let Some(auth_sig) = auth_sig_opt {
        let mut sig_opt = None;
        let mut ts_opt = None;
        let mut pk_opt = None;
        for child in auth_sig.children() {
            match child.type_code {
                ED25519_SIG => sig_opt = child.as_bytes(),
                ao_types::typecode::TIMESTAMP => ts_opt = child.as_bytes(),
                ED25519_PUB => pk_opt = child.as_bytes(),
                _ => {}
            }
        }
        if let (Some(sig_bytes), Some(ts_bytes), Some(pk_bytes)) = (sig_opt, ts_opt, pk_opt) {
            if sig_bytes.len() == 64 && ts_bytes.len() == 8 && pk_bytes.len() == 32 {
                let ts = ao_types::timestamp::Timestamp::from_bytes(
                    ts_bytes.try_into().unwrap_or([0u8; 8])
                );
                let mut sig = [0u8; 64];
                sig.copy_from_slice(sig_bytes);
                if !ao_crypto::sign::verify_dataitem(pk_bytes, block_contents, ts, &sig) {
                    return Err(anyhow::anyhow!("blockmaker signature verification failed"));
                }
            }
        }
    }

    Ok(())
}

/// Extract the TIMESTAMP from a BLOCK_SIGNED or BLOCK DataItem.
/// Falls back to 0 if no timestamp found (shouldn't happen for valid blocks).
fn extract_block_timestamp(item: &ao_types::dataitem::DataItem) -> i64 {
    use ao_types::typecode::TIMESTAMP;
    // Walk immediate children looking for a TIMESTAMP field
    for child in item.children() {
        if child.type_code == TIMESTAMP {
            if let Some(bytes) = child.as_bytes() {
                if bytes.len() == 8 {
                    let ts = ao_types::timestamp::Timestamp::from_bytes(
                        bytes.try_into().unwrap_or([0u8; 8])
                    );
                    return ts.raw();
                }
            }
        }
        // Also check nested containers (BLOCK inside BLOCK_SIGNED)
        for grandchild in child.children() {
            if grandchild.type_code == TIMESTAMP {
                if let Some(bytes) = grandchild.as_bytes() {
                    if bytes.len() == 8 {
                        let ts = ao_types::timestamp::Timestamp::from_bytes(
                            bytes.try_into().unwrap_or([0u8; 8])
                        );
                        return ts.raw();
                    }
                }
            }
        }
    }
    0
}

/// Subscribe to primary SSE and store new blocks as they arrive.
async fn sse_sync_chain(
    state: &Arc<AppState>,
    client: &reqwest::Client,
    primary_url: &str,
    chain_id: &str,
    batch_size: u64,
) -> Result<(), anyhow::Error> {
    let events_url = format!("{}/chain/{}/events", primary_url, chain_id);

    // Build a client without timeout for SSE (the connection stays open indefinitely)
    let sse_client = reqwest::Client::builder()
        .build()?;

    let response = sse_client.get(&events_url)
        .header("Accept", "text/event-stream")
        .send().await?
        .error_for_status()?;

    if let Some(ref sync_state) = state.standby_sync {
        sync_state.sse_connected.store(true, Ordering::Relaxed);
    }

    info!(chain_id = %chain_id, "SSE connected to primary");

    // Parse SSE manually from the byte stream
    let mut stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut event_type = String::new();
    let mut data_lines = String::new();

    use futures_util::StreamExt;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let text = String::from_utf8_lossy(&chunk);
        // Normalize \r\n to \n for consistent SSE parsing
        buffer.push_str(&text.replace("\r\n", "\n"));

        // Process complete events (terminated by blank line)
        while let Some(pos) = buffer.find("\n\n") {
            let event_text = buffer[..pos].to_string();
            buffer = buffer[pos + 2..].to_string();

            // Parse SSE fields
            event_type.clear();
            data_lines.clear();
            for line in event_text.lines() {
                if let Some(val) = line.strip_prefix("event:") {
                    event_type = val.trim().to_string();
                } else if let Some(val) = line.strip_prefix("data:") {
                    if !data_lines.is_empty() {
                        data_lines.push('\n');
                    }
                    data_lines.push_str(val.trim());
                }
            }

            if event_type == "block" && !data_lines.is_empty() {
                match handle_sse_block(state, client, primary_url, chain_id, &data_lines, batch_size).await {
                    Ok(()) => {}
                    Err(e) => {
                        warn!(chain_id = %chain_id, "Failed to handle SSE block: {}", e);
                    }
                }
            } else if event_type == "lagged" {
                warn!(chain_id = %chain_id, "SSE lagged, re-syncing missed blocks");
                batch_sync_chain(state, client, primary_url, chain_id, batch_size).await?;
            }
        }
    }

    Ok(())
}

/// Handle a single block notification from SSE.
/// The SSE data is a BlockInfo JSON (height, hash, timestamp, etc).
/// We fetch the actual block data from the primary's blocks endpoint.
async fn handle_sse_block(
    state: &Arc<AppState>,
    client: &reqwest::Client,
    primary_url: &str,
    chain_id: &str,
    block_info_json: &str,
    batch_size: u64,
) -> Result<(), anyhow::Error> {
    let block_info: crate::BlockInfo = serde_json::from_str(block_info_json)?;

    let chain = state.get_chain_or_err(chain_id)
        .map_err(|_| anyhow::anyhow!("chain {} not found", chain_id))?;

    // Check if we already have this block
    let local_height = {
        let store = chain.store.lock()
            .map_err(|e| anyhow::anyhow!("store lock: {}", e))?;
        match store.load_chain_meta()? {
            Some(m) => m.block_height,
            None => return Err(anyhow::anyhow!("no chain metadata — genesis not synced")),
        }
    };

    if block_info.height <= local_height {
        return Ok(()); // Already have it
    }

    // If we're behind by more than 1 block, batch-sync the gap
    if block_info.height > local_height + 1 {
        batch_sync_chain(state, client, primary_url, chain_id, batch_size).await?;
        return Ok(());
    }

    // Fetch the single block
    let blocks_url = format!(
        "{}/chain/{}/blocks?from={}&to={}",
        primary_url, chain_id, block_info.height, block_info.height
    );
    let blocks: Vec<serde_json::Value> = client.get(&blocks_url)
        .send().await?
        .error_for_status()?
        .json().await?;

    if let Some(block_json) = blocks.into_iter().next() {
        let item = ao_json::from_json(&block_json)
            .map_err(|e| anyhow::anyhow!("block decode: {}", e))?;
        let data = item.to_bytes();
        let hash = ao_crypto::hash::sha256(&data);
        let ts = extract_block_timestamp(&item);

        let chain_clone = Arc::clone(&chain);
        let height = block_info.height;
        tokio::task::spawn_blocking(move || -> Result<(), anyhow::Error> {
            let store = chain_clone.store.lock()
                .map_err(|e| anyhow::anyhow!("store lock: {}", e))?;
            let prev_hash = store.load_chain_meta()?
                .map(|m| m.prev_hash);
            verify_synced_block(&item, &hash, prev_hash.as_ref())
                .map_err(|e| anyhow::anyhow!("block {} integrity: {}", height, e))?;
            store.store_block(height, ts, &hash, &data)?;
            store.advance_block(height, ts, &hash)?;
            Ok(())
        }).await??;
    }

    if let Some(ref sync_state) = state.standby_sync {
        sync_state.synced_height.fetch_max(block_info.height, Ordering::Relaxed);
        sync_state.blocks_synced.fetch_add(1, Ordering::Relaxed);
    }

    info!(chain_id = %chain_id, height = block_info.height, "Synced block from SSE");
    Ok(())
}
