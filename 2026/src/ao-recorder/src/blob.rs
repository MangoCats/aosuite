use std::collections::HashMap;
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

// ── Constants ───────────────────────────────────────────────────────

/// Allowed MIME type prefixes for blob upload. Only image and PDF are accepted.
const ALLOWED_MIME_PREFIXES: &[&str] = &["image/", "application/pdf"];

/// Default per-chain blob storage quota: 100 MB.
pub const DEFAULT_BLOB_QUOTA_PER_CHAIN: u64 = 100 * 1024 * 1024;

// ── Error type ──────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("blob too large: {size} bytes exceeds limit of {max} bytes")]
    TooLarge { size: usize, max: usize },
    #[error("no MIME delimiter (NUL byte) found in blob data")]
    NoMimeDelimiter,
    #[error("invalid MIME type prefix")]
    InvalidMime,
    #[error("MIME type not allowed: {0}; accepted: image/*, application/pdf")]
    MimeNotAllowed(String),
    #[error("chain blob quota exceeded: {used} + {adding} > {quota} bytes")]
    QuotaExceeded { used: u64, adding: u64, quota: u64 },
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
            BlobError::NoMimeDelimiter | BlobError::InvalidMime
            | BlobError::InvalidHash | BlobError::MimeNotAllowed(_) => StatusCode::BAD_REQUEST,
            BlobError::QuotaExceeded { .. } => StatusCode::PAYLOAD_TOO_LARGE,
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
///
/// Blobs are stored flat in a single directory, named by their SHA-256 hex hash.
/// A per-chain quota is enforced via an in-memory usage tracker (chain_id → bytes used).
/// Blob-chain ownership is tracked so retrieval can enforce chain isolation.
pub struct BlobStore {
    dir: PathBuf,
    max_blob_bytes: usize,
    /// Per-chain storage quota in bytes.
    quota_per_chain: u64,
    /// Tracks total blob bytes per chain. Protected by a mutex for thread safety.
    chain_usage: std::sync::Mutex<HashMap<String, u64>>,
    /// Maps blob hash → owning chain_id. Used to enforce cross-chain isolation on read.
    blob_owners: std::sync::Mutex<HashMap<String, String>>,
}

impl BlobStore {
    /// Create a new BlobStore. Creates the directory if it does not exist.
    /// Cleans up any leftover `.tmp_*` files from prior crashes.
    pub fn new(dir: PathBuf, max_blob_bytes: usize) -> Result<Self, BlobError> {
        std::fs::create_dir_all(&dir)?;

        // Startup cleanup: remove stale temp files from prior crashes.
        let mut cleaned = 0u32;
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str()
                    && name.starts_with(".tmp_")
                {
                    let _ = std::fs::remove_file(entry.path());
                    cleaned += 1;
                }
            }
        }
        if cleaned > 0 {
            tracing::info!(count = cleaned, "Cleaned up stale blob temp files");
        }

        Ok(BlobStore {
            dir,
            max_blob_bytes,
            quota_per_chain: DEFAULT_BLOB_QUOTA_PER_CHAIN,
            chain_usage: std::sync::Mutex::new(HashMap::new()),
            blob_owners: std::sync::Mutex::new(HashMap::new()),
        })
    }

    /// Set the per-chain blob quota. Call after construction if configured.
    pub fn set_quota(&mut self, quota: u64) {
        self.quota_per_chain = quota;
    }

    /// Store blob data for a specific chain. Returns the SHA-256 hex hash.
    /// Idempotent: if the file already exists, the write is skipped.
    /// Enforces per-chain quota.
    pub fn store(&self, data: &[u8], chain_id: &str) -> Result<String, BlobError> {
        if data.len() > self.max_blob_bytes {
            return Err(BlobError::TooLarge {
                size: data.len(),
                max: self.max_blob_bytes,
            });
        }

        let hash = ao_crypto::hash::sha256(data);
        let hash_hex = hex::encode(hash);
        let target = self.dir.join(&hash_hex);

        // Check if already stored (idempotent). Register ownership if needed.
        if target.exists() {
            let mut owners = self.blob_owners.lock().expect("blob_owners lock");
            owners.entry(hash_hex.clone()).or_insert_with(|| chain_id.to_string());
            return Ok(hash_hex);
        }

        // Enforce per-chain quota before writing.
        {
            let usage = self.chain_usage.lock().expect("chain_usage lock");
            let current = usage.get(chain_id).copied().unwrap_or(0);
            let adding = data.len() as u64;
            if current + adding > self.quota_per_chain {
                return Err(BlobError::QuotaExceeded {
                    used: current,
                    adding,
                    quota: self.quota_per_chain,
                });
            }
        }

        // Atomic write: write to temp file then rename.
        let tmp_path = self.dir.join(format!(".tmp_{}", hash_hex));
        std::fs::write(&tmp_path, data)?;
        match std::fs::rename(&tmp_path, &target) {
            Ok(()) => {}
            Err(_) if target.exists() => {
                // Another thread already renamed the file — clean up our temp and succeed.
                let _ = std::fs::remove_file(&tmp_path);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_path);
                return Err(BlobError::IoError(e));
            }
        }

        // Update usage and ownership tracking.
        {
            let mut usage = self.chain_usage.lock().expect("chain_usage lock");
            *usage.entry(chain_id.to_string()).or_insert(0) += data.len() as u64;
        }
        {
            let mut owners = self.blob_owners.lock().expect("blob_owners lock");
            owners.entry(hash_hex.clone()).or_insert_with(|| chain_id.to_string());
        }

        Ok(hash_hex)
    }

    /// Retrieve blob data by its SHA-256 hex hash.
    /// Enforces chain isolation: only the owning chain can read the blob.
    pub fn get(&self, hash_hex: &str, chain_id: &str) -> Result<Option<Vec<u8>>, BlobError> {
        validate_hash_hex(hash_hex)?;

        // Enforce chain isolation.
        {
            let owners = self.blob_owners.lock().expect("blob_owners lock");
            if let Some(owner) = owners.get(hash_hex)
                && owner != chain_id
            {
                return Ok(None); // Pretend not found — no cross-chain leakage.
            }
            // If not in the map (e.g., loaded from disk before tracking), allow access.
        }

        let path = self.dir.join(hash_hex);
        match std::fs::read(&path) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BlobError::IoError(e)),
        }
    }

    /// Check if a blob exists by hash (no chain isolation check — internal use only).
    pub fn exists(&self, hash_hex: &str) -> bool {
        if validate_hash_hex(hash_hex).is_err() {
            return false;
        }
        self.dir.join(hash_hex).exists()
    }

    /// Return the size of a blob in bytes, enforcing chain isolation.
    /// Only returns a size if the blob is owned by the specified chain.
    pub fn get_size_for_chain(&self, hash_hex: &str, chain_id: &str) -> Result<Option<u64>, BlobError> {
        if validate_hash_hex(hash_hex).is_err() {
            return Ok(None);
        }
        // Enforce chain isolation: only the owning chain can reference the blob.
        {
            let owners = self.blob_owners.lock().expect("blob_owners lock");
            if let Some(owner) = owners.get(hash_hex)
                && owner != chain_id
            {
                return Ok(None);
            }
        }
        let path = self.dir.join(hash_hex);
        match std::fs::metadata(&path) {
            Ok(meta) => Ok(Some(meta.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BlobError::IoError(e)),
        }
    }

    /// Return current storage usage for a chain in bytes.
    pub fn chain_usage(&self, chain_id: &str) -> u64 {
        self.chain_usage.lock().expect("chain_usage lock")
            .get(chain_id).copied().unwrap_or(0)
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

/// Check if a MIME type is in the allowlist.
fn is_mime_allowed(mime: &str) -> bool {
    let lower = mime.to_ascii_lowercase();
    ALLOWED_MIME_PREFIXES.iter().any(|prefix| lower.starts_with(prefix))
}

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

    // Enforce MIME allowlist: only image/* and application/pdf.
    if !is_mime_allowed(mime) {
        return Err(BlobError::MimeNotAllowed(mime.to_string()));
    }

    let hash_hex = match blob_store.store(data, &chain_id) {
        Ok(hash) => {
            crate::metrics::record_blob_uploaded(&chain_id, data.len(), "ok");
            hash
        }
        Err(e) => {
            let status = match &e {
                BlobError::TooLarge { .. } => "too_large",
                BlobError::QuotaExceeded { .. } => "quota_exceeded",
                _ => "error",
            };
            crate::metrics::record_blob_uploaded(&chain_id, data.len(), status);
            return Err(e);
        }
    };
    tracing::info!(chain = %chain_id, size = data.len(), hash = %hash_hex, "Blob stored");

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
        .get(&hash, &chain_id)?
        .ok_or(BlobError::NotFound)?;

    let (mime, offset) = extract_mime(&data).ok_or(BlobError::NoMimeDelimiter)?;
    tracing::debug!(%hash, "Blob retrieved");
    let content = data[offset..].to_vec();

    // Determine Content-Disposition: inline for images, attachment for everything else.
    let disposition = if mime.starts_with("image/") {
        format!("inline; filename=\"{}\"", &hash[..16])
    } else {
        format!("attachment; filename=\"{}\"", &hash[..16])
    };

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, mime.to_string()),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable".to_string()),
            (header::HeaderName::from_static("x-content-type-options"), "nosniff".to_string()),
            (header::HeaderName::from_static("content-security-policy"), "default-src 'none'; img-src 'self'; style-src 'none'; script-src 'none'".to_string()),
            (header::CONTENT_DISPOSITION, disposition),
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
        let hash = store.store(data, "chain_a").unwrap();

        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        let retrieved = store.get(&hash, "chain_a").unwrap().unwrap();
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
        match store.store(&data, "chain_a") {
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

        let data = b"image/png\0hello world";
        let hash1 = store.store(data, "chain_a").unwrap();
        let hash2 = store.store(data, "chain_a").unwrap();
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
        assert!(matches!(store.get("abcd", "c"), Err(BlobError::InvalidHash)));

        // Right length but not hex
        let bad = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz";
        assert!(matches!(store.get(bad, "c"), Err(BlobError::InvalidHash)));

        // Good format but not found
        let good = "a".repeat(64);
        assert!(store.get(&good, "c").unwrap().is_none());
    }

    #[test]
    fn test_mime_allowlist() {
        assert!(is_mime_allowed("image/png"));
        assert!(is_mime_allowed("image/jpeg"));
        assert!(is_mime_allowed("image/webp"));
        assert!(is_mime_allowed("Image/PNG")); // case-insensitive
        assert!(is_mime_allowed("application/pdf"));
        assert!(!is_mime_allowed("text/html"));
        assert!(!is_mime_allowed("application/javascript"));
        assert!(!is_mime_allowed("text/plain"));
    }

    #[test]
    fn test_per_chain_quota() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = BlobStore::new(dir.path().join("blobs"), 5_242_880).unwrap();
        store.set_quota(100); // 100-byte quota

        // First blob fits (image/png\0 = 10 bytes + 50 = 60 total).
        let data1 = b"image/png\0".iter().chain(&[0xAA; 50]).copied().collect::<Vec<u8>>();
        assert!(store.store(&data1, "chain_a").is_ok());
        assert_eq!(store.chain_usage("chain_a"), 60);

        // Second blob of 60 bytes would exceed 100 quota.
        let data2 = b"image/png\0".iter().chain(&[0xBB; 50]).copied().collect::<Vec<u8>>();
        match store.store(&data2, "chain_a") {
            Err(BlobError::QuotaExceeded { used, adding, quota }) => {
                assert_eq!(used, 60);
                assert_eq!(adding, 60);
                assert_eq!(quota, 100);
            }
            other => panic!("expected QuotaExceeded, got {:?}", other),
        }

        // Different chain has its own quota.
        assert!(store.store(&data2, "chain_b").is_ok());
    }

    #[test]
    fn test_cross_chain_isolation() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"), 5_242_880).unwrap();

        let data = b"image/png\0secret image data";
        let hash = store.store(data, "chain_a").unwrap();

        // Same chain can read it.
        assert!(store.get(&hash, "chain_a").unwrap().is_some());

        // Different chain gets None (not found), not the data.
        assert!(store.get(&hash, "chain_b").unwrap().is_none());
    }

    #[test]
    fn test_temp_file_cleanup_on_startup() {
        let dir = tempfile::tempdir().unwrap();
        let blob_dir = dir.path().join("blobs");
        std::fs::create_dir_all(&blob_dir).unwrap();

        // Create some stale temp files.
        std::fs::write(blob_dir.join(".tmp_abc123"), b"stale").unwrap();
        std::fs::write(blob_dir.join(".tmp_def456"), b"stale2").unwrap();
        // Create a normal blob file that should NOT be deleted.
        let normal_name = "a".repeat(64);
        std::fs::write(blob_dir.join(&normal_name), b"real blob").unwrap();

        // Creating BlobStore should clean up .tmp_ files.
        let _store = BlobStore::new(blob_dir.clone(), 5_242_880).unwrap();

        assert!(!blob_dir.join(".tmp_abc123").exists());
        assert!(!blob_dir.join(".tmp_def456").exists());
        assert!(blob_dir.join(&normal_name).exists());
    }
}
