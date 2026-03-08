use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json},
};
use serde_json::json;

use crate::AppState;

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("blob too large: {size} bytes exceeds limit of {max} bytes")]
    TooLarge { size: usize, max: usize },
    #[error("no MIME delimiter (NUL byte) found in blob data")]
    NoMimeDelimiter,
    #[error("invalid MIME type prefix")]
    InvalidMime,
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("invalid hash format")]
    InvalidHash,
    #[error("chain not found")]
    ChainNotFound,
    #[error("blob not found")]
    NotFound,
}

impl IntoResponse for BlobError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            BlobError::TooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            BlobError::NoMimeDelimiter | BlobError::InvalidMime | BlobError::InvalidHash => {
                StatusCode::BAD_REQUEST
            }
            BlobError::ChainNotFound | BlobError::NotFound => StatusCode::NOT_FOUND,
            BlobError::IoError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
    }
}

// ── MIME extraction ─────────────────────────────────────────────────

/// Extract the MIME type from the wire format: `<mime-type-utf8> NUL <content-bytes>`.
/// Returns `(mime_type, offset_to_content)` where offset is the byte after the NUL delimiter.
/// Returns `None` if no NUL found, the prefix is empty, or the prefix is not valid UTF-8.
pub fn extract_mime(data: &[u8]) -> Option<(&str, usize)> {
    let nul_pos = data.iter().position(|&b| b == 0)?;
    if nul_pos == 0 {
        return None;
    }
    let prefix = std::str::from_utf8(&data[..nul_pos]).ok()?;
    Some((prefix, nul_pos + 1))
}

// ── BlobStore ───────────────────────────────────────────────────────

/// Content-addressed blob storage backed by the filesystem.
pub struct BlobStore {
    dir: PathBuf,
    max_blob_bytes: usize,
}

impl BlobStore {
    /// Create a new BlobStore. Creates the directory if it does not exist.
    pub fn new(dir: PathBuf, max_blob_bytes: usize) -> Result<Self, BlobError> {
        std::fs::create_dir_all(&dir)?;
        Ok(BlobStore { dir, max_blob_bytes })
    }

    /// Store blob data. Returns the SHA-256 hex hash.
    /// Idempotent: if the file already exists, the write is skipped.
    pub fn store(&self, data: &[u8]) -> Result<String, BlobError> {
        if data.len() > self.max_blob_bytes {
            return Err(BlobError::TooLarge {
                size: data.len(),
                max: self.max_blob_bytes,
            });
        }

        let hash = ao_crypto::hash::sha256(data);
        let hash_hex = hex::encode(hash);
        let target = self.dir.join(&hash_hex);

        if target.exists() {
            return Ok(hash_hex);
        }

        // Atomic write: write to temp file then rename.
        let tmp_path = self.dir.join(format!(".tmp_{}", hash_hex));
        std::fs::write(&tmp_path, data)?;
        match std::fs::rename(&tmp_path, &target) {
            Ok(()) => Ok(hash_hex),
            Err(_) if target.exists() => {
                // Another thread already renamed the file — clean up our temp and succeed.
                let _ = std::fs::remove_file(&tmp_path);
                Ok(hash_hex)
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_path);
                Err(BlobError::IoError(e))
            }
        }
    }

    /// Retrieve blob data by its SHA-256 hex hash.
    pub fn get(&self, hash_hex: &str) -> Result<Option<Vec<u8>>, BlobError> {
        validate_hash_hex(hash_hex)?;
        let path = self.dir.join(hash_hex);
        match std::fs::read(&path) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BlobError::IoError(e)),
        }
    }

    /// Check if a blob exists by hash.
    pub fn exists(&self, hash_hex: &str) -> bool {
        if validate_hash_hex(hash_hex).is_err() {
            return false;
        }
        self.dir.join(hash_hex).exists()
    }
}

/// Validate that a hash string is exactly 64 lowercase hex characters.
fn validate_hash_hex(hash_hex: &str) -> Result<(), BlobError> {
    if hash_hex.len() != 64 || !hash_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(BlobError::InvalidHash);
    }
    Ok(())
}

// ── Axum handlers ───────────────────────────────────────────────────

/// POST /chain/{id}/blob — upload a blob.
pub async fn upload_blob(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<String>,
    body: Bytes,
) -> Result<impl IntoResponse, BlobError> {
    // Verify chain exists.
    state.get_chain_or_err(&chain_id).map_err(|_| BlobError::ChainNotFound)?;

    let blob_store = state
        .blob_store
        .as_ref()
        .ok_or_else(|| BlobError::IoError(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "blob storage not configured",
        )))?;

    let data = body.as_ref();

    // Validate MIME delimiter exists.
    let (mime, _) = extract_mime(data).ok_or(BlobError::NoMimeDelimiter)?;

    // Basic MIME validation: must contain a '/'.
    if !mime.contains('/') {
        return Err(BlobError::InvalidMime);
    }

    // Reject MIME types with control characters or excessive length.
    if mime.len() >= 200 || mime.chars().any(|c| c.is_control()) {
        return Err(BlobError::InvalidMime);
    }

    let hash_hex = blob_store.store(data)?;
    tracing::info!(size = data.len(), hash = %hash_hex, "Blob stored");

    Ok(Json(json!({ "hash": hash_hex })))
}

/// GET /chain/{id}/blob/{hash} — retrieve a blob.
pub async fn get_blob(
    State(state): State<Arc<AppState>>,
    Path((chain_id, hash)): Path<(String, String)>,
) -> Result<impl IntoResponse, BlobError> {
    // Verify chain exists.
    state.get_chain_or_err(&chain_id).map_err(|_| BlobError::ChainNotFound)?;

    let blob_store = state
        .blob_store
        .as_ref()
        .ok_or_else(|| BlobError::IoError(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "blob storage not configured",
        )))?;

    let data = blob_store
        .get(&hash)?
        .ok_or(BlobError::NotFound)?;

    let (mime, offset) = extract_mime(&data).ok_or(BlobError::NoMimeDelimiter)?;
    tracing::debug!(%hash, "Blob retrieved");
    let content = data[offset..].to_vec();

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ),
        ],
        content,
    ))
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_retrieve() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"), 5_242_880).unwrap();

        let data = b"image/png\0\x89PNG fake image data";
        let hash = store.store(data).unwrap();

        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        let retrieved = store.get(&hash).unwrap().unwrap();
        assert_eq!(retrieved, data);
    }

    #[test]
    fn test_extract_mime_valid() {
        let data = b"image/jpeg\0\xff\xd8\xff\xe0some bytes";
        let (mime, offset) = extract_mime(data).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert_eq!(offset, 11); // "image/jpeg" is 10 bytes + 1 for NUL
        assert_eq!(data[offset], 0xff);
    }

    #[test]
    fn test_extract_mime_empty_returns_none() {
        let data = b"\0some content after empty mime";
        assert!(extract_mime(data).is_none());
    }

    #[test]
    fn test_extract_mime_no_nul() {
        let data = b"image/jpeg without nul delimiter";
        assert!(extract_mime(data).is_none());
    }

    #[test]
    fn test_reject_oversized() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"), 100).unwrap();

        let data = vec![0u8; 101];
        match store.store(&data) {
            Err(BlobError::TooLarge { size, max }) => {
                assert_eq!(size, 101);
                assert_eq!(max, 100);
            }
            other => panic!("expected TooLarge, got {:?}", other),
        }
    }

    #[test]
    fn test_idempotent_store() {
        let dir = tempfile::tempdir().unwrap();
        let blob_dir = dir.path().join("blobs");
        let store = BlobStore::new(blob_dir.clone(), 5_242_880).unwrap();

        let data = b"text/plain\0hello world";
        let hash1 = store.store(data).unwrap();
        let hash2 = store.store(data).unwrap();
        assert_eq!(hash1, hash2);

        // Only one file on disk (no temp files left behind).
        let entries: Vec<_> = std::fs::read_dir(&blob_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].file_name().to_str().unwrap(), hash1);
    }

    #[test]
    fn test_invalid_hash_format() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"), 5_242_880).unwrap();

        // Too short
        assert!(matches!(store.get("abcd"), Err(BlobError::InvalidHash)));

        // Right length but not hex
        let bad = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
        assert!(matches!(store.get(bad), Err(BlobError::InvalidHash)));

        // Good format but not found
        let good = "a".repeat(64);
        assert!(store.get(&good).unwrap().is_none());
    }
}
