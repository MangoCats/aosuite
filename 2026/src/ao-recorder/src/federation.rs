//! Recorder federation: streaming block sync, identity authentication, chain redirects.
//!
//! ## Endpoints
//!
//! - `GET /chain/{id}/sync?from=N` — stream blocks as newline-delimited JSON (NDJSON).
//!   No 1000-block cap. Authenticated via `X-Recorder-Identity` header when
//!   `require_sync_auth` is true.
//!
//! ## Authentication
//!
//! The `X-Recorder-Identity` header carries a JSON-encoded RECORDER_IDENTITY DataItem.
//! The handler verifies the self-signature and checks the embedded public key against
//! `trusted_sync_keys`.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;
use tracing::warn;

use ao_types::json as ao_json;

use crate::{AppState, RecorderError, lock_store};
use crate::identity;

/// Query parameters for the sync endpoint.
#[derive(Deserialize)]
pub struct SyncParams {
    /// Starting block height (inclusive). Defaults to 0.
    #[serde(default)]
    pub from: u64,
}

/// Maximum blocks per sync response. Clients re-request with updated `from` to continue.
const SYNC_BATCH_LIMIT: u64 = 10_000;

/// `GET /chain/{id}/sync?from=N` — stream blocks from height N as NDJSON.
///
/// Each line is a JSON object with `height` and `block` (the DataItem JSON).
/// Returns up to SYNC_BATCH_LIMIT blocks per request. Clients check the last
/// height returned and re-request with `from=last+1` to continue.
///
/// Returns 401 if `require_sync_auth` is true and the caller isn't authenticated.
/// Returns 307 if the chain has been redirected.
pub async fn sync_blocks(
    State(state): State<Arc<AppState>>,
    Path(chain_id_hex): Path<String>,
    Query(params): Query<SyncParams>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, RecorderError> {
    // Authenticate if required
    verify_sync_auth(&state, &headers)?;

    // Check redirects
    let chain = state.get_chain_or_redirect(&chain_id_hex)?;

    let from = params.from;

    // Fetch raw block bytes on the blocking pool (short lock hold)
    let raw_blocks: Vec<(u64, Vec<u8>)> = crate::blocking(move || {
        let store = lock_store(&chain)?;
        let meta = store.load_chain_meta()
            .map_err(|e| RecorderError::Internal(e.to_string()))?
            .ok_or(RecorderError::ChainNotFound)?;

        if from > meta.block_height {
            return Ok(Vec::new());
        }

        let to = meta.block_height.min(from.saturating_add(SYNC_BATCH_LIMIT - 1));
        let mut blocks = Vec::new();
        for height in from..=to {
            if let Some(data) = store.get_block(height)
                .map_err(|e| RecorderError::Internal(e.to_string()))?
            {
                blocks.push((height, data));
            }
        }
        Ok(blocks)
    }).await?;

    // Serialize outside the lock (no mutex held, CPU-bound only)
    let mut output = String::new();
    for (height, data) in &raw_blocks {
        match ao_types::dataitem::DataItem::from_bytes(data) {
            Ok(item) => {
                let block_json = ao_json::to_json(&item);
                let line = serde_json::json!({ "height": height, "block": block_json });
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str(&line.to_string());
            }
            Err(e) => {
                warn!(height, "Skipping block with decode error: {:?}", e);
            }
        }
    }

    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/x-ndjson")],
        output,
    ))
}

/// Verify the X-Recorder-Identity header if sync auth is required.
///
/// When `require_sync_auth` is false, this is a no-op.
/// When true, the header must be present, contain a valid signed RECORDER_IDENTITY,
/// and the embedded public key must be in `trusted_sync_keys`.
fn verify_sync_auth(state: &AppState, headers: &HeaderMap) -> Result<(), RecorderError> {
    if !state.require_sync_auth {
        return Ok(());
    }

    let header_value = headers.get("X-Recorder-Identity")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| RecorderError::Unauthorized(
            "X-Recorder-Identity header required for sync".into()
        ))?;

    // Parse the JSON-encoded RECORDER_IDENTITY DataItem
    let json_value: serde_json::Value = serde_json::from_str(header_value)
        .map_err(|e| RecorderError::Unauthorized(format!("invalid identity JSON: {}", e)))?;

    let identity_item = ao_json::from_json(&json_value)
        .map_err(|e| RecorderError::Unauthorized(format!("invalid identity DataItem: {}", e)))?;

    // Verify self-signature
    if !identity::verify_recorder_identity(&identity_item) {
        return Err(RecorderError::Unauthorized("identity signature verification failed".into()));
    }

    // Extract public key and check against trusted list
    let pubkey = identity::extract_recorder_pubkey(&identity_item)
        .ok_or_else(|| RecorderError::Unauthorized("identity missing public key".into()))?;

    let trusted = state.trusted_sync_keys.read()
        .map_err(|e| RecorderError::LockPoisoned(format!("trusted_sync_keys: {}", e)))?;

    if !trusted.iter().any(|k| k == &pubkey) {
        return Err(RecorderError::Unauthorized("recorder not in trusted sync list".into()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ao_types::typecode::*;
    use ao_types::dataitem::DataItem;
    use ao_types::timestamp::Timestamp;
    use ao_crypto::sign::SigningKey;

    #[test]
    fn test_verify_sync_auth_disabled() {
        // When require_sync_auth is false, any header (or none) passes
        let key = SigningKey::generate();
        let state = AppState::new_multi(None, key);
        // Override to false (default is true)
        let state = {
            let mut s = state;
            s.require_sync_auth = false;
            s
        };
        let headers = HeaderMap::new();
        assert!(verify_sync_auth(&state, &headers).is_ok());
    }

    #[test]
    fn test_verify_sync_auth_missing_header() {
        let key = SigningKey::generate();
        let state = AppState::new_multi(None, key);
        let headers = HeaderMap::new();
        assert!(verify_sync_auth(&state, &headers).is_err());
    }

    #[test]
    fn test_verify_sync_auth_valid() {
        let recorder_key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1_772_611_200);
        let identity_item = identity::build_recorder_identity(
            &recorder_key,
            "https://recorder.example.com",
            "Test Recorder",
            ts,
        );
        let identity_json = ao_json::to_json(&identity_item);

        let blockmaker_key = SigningKey::generate();
        let mut state = AppState::new_multi(None, blockmaker_key);
        state.require_sync_auth = true;
        // Add the recorder's pubkey to trusted list
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(recorder_key.public_key_bytes());
        state.trusted_sync_keys.write().unwrap().push(pubkey);

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Recorder-Identity",
            identity_json.to_string().parse().unwrap(),
        );

        assert!(verify_sync_auth(&state, &headers).is_ok());
    }

    #[test]
    fn test_verify_sync_auth_untrusted_key() {
        let recorder_key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1_772_611_200);
        let identity_item = identity::build_recorder_identity(
            &recorder_key,
            "https://recorder.example.com",
            "Test Recorder",
            ts,
        );
        let identity_json = ao_json::to_json(&identity_item);

        let blockmaker_key = SigningKey::generate();
        let mut state = AppState::new_multi(None, blockmaker_key);
        state.require_sync_auth = true;
        // Don't add the key to trusted list

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Recorder-Identity",
            identity_json.to_string().parse().unwrap(),
        );

        assert!(verify_sync_auth(&state, &headers).is_err());
    }

    #[test]
    fn test_verify_sync_auth_tampered_identity() {
        let recorder_key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1_772_611_200);
        let identity_item = identity::build_recorder_identity(
            &recorder_key,
            "https://recorder.example.com",
            "Test Recorder",
            ts,
        );

        // Tamper with the URL
        let mut children: Vec<DataItem> = identity_item.children().to_vec();
        children[1] = DataItem::bytes(RECORDER_URL, b"https://evil.com".to_vec());
        let tampered = DataItem::container(RECORDER_IDENTITY, children);
        let tampered_json = ao_json::to_json(&tampered);

        let blockmaker_key = SigningKey::generate();
        let mut state = AppState::new_multi(None, blockmaker_key);
        state.require_sync_auth = true;
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(recorder_key.public_key_bytes());
        state.trusted_sync_keys.write().unwrap().push(pubkey);

        let mut headers = HeaderMap::new();
        headers.insert(
            "X-Recorder-Identity",
            tampered_json.to_string().parse().unwrap(),
        );

        assert!(verify_sync_auth(&state, &headers).is_err());
    }
}
