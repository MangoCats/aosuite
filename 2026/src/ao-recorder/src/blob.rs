use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Json},
};
use rusqlite::{Connection, params};
use serde_json::json;

use ao_types::dataitem::DataItem;
use ao_types::typecode;

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
    #[error("blob pruned: {reason}")]
    Pruned { reason: String },
    #[error("database error: {0}")]
    DbError(String),
}

impl IntoResponse for BlobError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            BlobError::TooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            BlobError::NoMimeDelimiter | BlobError::InvalidMime
            | BlobError::InvalidHash | BlobError::MimeNotAllowed(_) => StatusCode::BAD_REQUEST,
            BlobError::QuotaExceeded { .. } => StatusCode::PAYLOAD_TOO_LARGE,
            BlobError::ChainNotFound | BlobError::NotFound => StatusCode::NOT_FOUND,
            BlobError::Pruned { .. } => StatusCode::GONE,
            BlobError::IoError(_) | BlobError::DbError(_) => StatusCode::INTERNAL_SERVER_ERROR,
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

/// Blob metadata row from the `blob_meta` SQLite table.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlobMeta {
    pub hash: String,
    pub chain_id: String,
    pub mime: String,
    pub size: u64,
    pub uploaded_at: i64,  // Unix seconds
}

/// Content-addressed blob storage backed by the filesystem + SQLite metadata.
///
/// Blobs are stored flat in a single directory, named by their SHA-256 hex hash.
/// Metadata (chain ownership, MIME type, upload timestamp) is stored in a SQLite
/// `blob_meta` table for reliable pruning and auditing. Per-chain quota enforcement
/// and chain isolation are backed by this metadata.
pub struct BlobStore {
    dir: PathBuf,
    max_blob_bytes: usize,
    /// Per-chain storage quota in bytes.
    quota_per_chain: u64,
    /// SQLite connection for blob metadata. Protected by mutex for thread safety.
    db: std::sync::Mutex<Connection>,
}

impl BlobStore {
    /// Create a new BlobStore. Creates the directory if it does not exist.
    /// Opens (or creates) the blob metadata SQLite database.
    /// Cleans up any leftover `.tmp_*` files from prior crashes.
    pub fn new(dir: PathBuf, max_blob_bytes: usize) -> Result<Self, BlobError> {
        std::fs::create_dir_all(&dir)?;

        // Open SQLite database for blob metadata
        let db_path = dir.join("blob_meta.db");
        let conn = Connection::open(&db_path)
            .map_err(|e| BlobError::DbError(format!("open blob_meta.db: {}", e)))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .map_err(|e| BlobError::DbError(format!("pragma: {}", e)))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS blob_meta (
                hash TEXT PRIMARY KEY,
                chain_id TEXT NOT NULL,
                mime TEXT NOT NULL,
                size INTEGER NOT NULL,
                uploaded_at INTEGER NOT NULL
            )"
        ).map_err(|e| BlobError::DbError(format!("create table: {}", e)))?;
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_blob_chain ON blob_meta(chain_id)"
        ).map_err(|e| BlobError::DbError(format!("create index: {}", e)))?;

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
            db: std::sync::Mutex::new(conn),
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

        // Extract MIME type for metadata
        let mime = extract_mime(data)
            .map(|(m, _)| m.to_string())
            .unwrap_or_else(|| "application/octet-stream".to_string());

        // Check if already stored (idempotent).
        if target.exists() {
            // Ensure metadata exists
            let db = self.db.lock().expect("db lock");
            let _ = db.execute(
                "INSERT OR IGNORE INTO blob_meta (hash, chain_id, mime, size, uploaded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![hash_hex, chain_id, mime, data.len() as i64, unix_now()],
            );
            return Ok(hash_hex);
        }

        // Enforce per-chain quota and reserve the metadata row atomically.
        // Holding the db lock through quota check + insert prevents TOCTOU races
        // where concurrent uploads both pass the check.
        {
            let db = self.db.lock().expect("db lock");
            let current = chain_usage_from_db(&db, chain_id);
            let adding = data.len() as u64;
            if current + adding > self.quota_per_chain {
                return Err(BlobError::QuotaExceeded {
                    used: current,
                    adding,
                    quota: self.quota_per_chain,
                });
            }
            // Reserve the metadata row before writing the file.
            db.execute(
                "INSERT OR REPLACE INTO blob_meta (hash, chain_id, mime, size, uploaded_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![hash_hex, chain_id, mime, data.len() as i64, unix_now()],
            ).map_err(|e| BlobError::DbError(format!("insert meta: {}", e)))?;
        }

        // Atomic write: write to temp file then rename.
        let tmp_path = self.dir.join(format!(".tmp_{}", hash_hex));
        if let Err(e) = std::fs::write(&tmp_path, data) {
            // Roll back metadata on file write failure
            let db = self.db.lock().expect("db lock");
            let _ = db.execute("DELETE FROM blob_meta WHERE hash = ?1", params![hash_hex]);
            return Err(BlobError::IoError(e));
        }
        match std::fs::rename(&tmp_path, &target) {
            Ok(()) => {}
            Err(_) if target.exists() => {
                let _ = std::fs::remove_file(&tmp_path);
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp_path);
                // Roll back metadata on rename failure
                let db = self.db.lock().expect("db lock");
                let _ = db.execute("DELETE FROM blob_meta WHERE hash = ?1", params![hash_hex]);
                return Err(BlobError::IoError(e));
            }
        }

        Ok(hash_hex)
    }

    /// Retrieve blob data by its SHA-256 hex hash.
    /// Enforces chain isolation: only the owning chain can read the blob.
    /// Returns 410 Gone (via BlobError::Pruned) if metadata exists but file is deleted.
    pub fn get(&self, hash_hex: &str, chain_id: &str) -> Result<Option<Vec<u8>>, BlobError> {
        validate_hash_hex(hash_hex)?;

        // Enforce chain isolation via metadata.
        let meta = {
            let db = self.db.lock().expect("db lock");
            get_blob_meta(&db, hash_hex)
        };
        if let Some(ref m) = meta {
            if m.chain_id != chain_id {
                return Ok(None); // Cross-chain isolation
            }
        }

        let path = self.dir.join(hash_hex);
        match std::fs::read(&path) {
            Ok(data) => Ok(Some(data)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File gone but metadata exists → pruned
                if meta.is_some() {
                    return Err(BlobError::Pruned {
                        reason: "blob was pruned per retention policy".into(),
                    });
                }
                Ok(None)
            }
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
    pub fn get_size_for_chain(&self, hash_hex: &str, chain_id: &str) -> Result<Option<u64>, BlobError> {
        if validate_hash_hex(hash_hex).is_err() {
            return Ok(None);
        }
        let db = self.db.lock().expect("db lock");
        if let Some(meta) = get_blob_meta(&db, hash_hex) {
            if meta.chain_id != chain_id {
                return Ok(None);
            }
            return Ok(Some(meta.size));
        }
        // Fallback to filesystem for blobs stored before metadata tracking
        let path = self.dir.join(hash_hex);
        match std::fs::metadata(&path) {
            Ok(meta) => Ok(Some(meta.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(BlobError::IoError(e)),
        }
    }

    /// Return current storage usage for a chain in bytes.
    pub fn chain_usage(&self, chain_id: &str) -> u64 {
        let db = self.db.lock().expect("db lock");
        chain_usage_from_db(&db, chain_id)
    }

    /// Get metadata for a specific blob.
    pub fn get_meta(&self, hash_hex: &str) -> Option<BlobMeta> {
        let db = self.db.lock().expect("db lock");
        get_blob_meta(&db, hash_hex)
    }

    /// List all blob metadata for a chain (paginated).
    pub fn list_chain_blobs(&self, chain_id: &str, offset: u64, limit: u64) -> Vec<BlobMeta> {
        let db = self.db.lock().expect("db lock");
        let mut stmt = match db.prepare(
            "SELECT hash, chain_id, mime, size, uploaded_at FROM blob_meta
             WHERE chain_id = ?1 ORDER BY uploaded_at DESC LIMIT ?2 OFFSET ?3"
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        match stmt.query_map(params![chain_id, limit as i64, offset as i64], |row| {
            Ok(BlobMeta {
                hash: row.get(0)?,
                chain_id: row.get(1)?,
                mime: row.get(2)?,
                size: row.get::<_, i64>(3)? as u64,
                uploaded_at: row.get(4)?,
            })
        }) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Prune expired blobs based on chain policies.
    /// Returns a list of prune events (blobs pruned or eligible for pruning).
    pub fn prune(&self, policies: &HashMap<String, BlobPolicy>, dry_run: bool) -> Vec<PruneEvent> {
        let now = unix_now();

        // Phase 1: Read all metadata under the lock, then release it.
        let all_blobs = {
            let db = self.db.lock().expect("db lock");
            let mut stmt = match db.prepare(
                "SELECT hash, chain_id, mime, size, uploaded_at FROM blob_meta"
            ) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            let result: Vec<BlobMeta> = match stmt.query_map([], |row| {
                Ok(BlobMeta {
                    hash: row.get(0)?,
                    chain_id: row.get(1)?,
                    mime: row.get(2)?,
                    size: row.get::<_, i64>(3)? as u64,
                    uploaded_at: row.get(4)?,
                })
            }) {
                Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
                Err(_) => return Vec::new(),
            };
            result
        }; // Lock released here

        // Phase 2: Evaluate policies and delete files without holding the lock.
        let mut events = Vec::new();

        for blob in &all_blobs {
            let policy = match policies.get(&blob.chain_id) {
                Some(p) => p,
                None => continue,
            };

            let rule = match policy.find_rule(&blob.mime) {
                Some(r) => r,
                None => continue,
            };

            let retention_secs = match rule.retention_seconds() {
                Some(s) => s,
                None => continue,
            };

            let age_secs = now - blob.uploaded_at;
            if age_secs > retention_secs {
                let event = PruneEvent {
                    hash: blob.hash.clone(),
                    chain_id: blob.chain_id.clone(),
                    mime: blob.mime.clone(),
                    age_days: age_secs as f64 / 86400.0,
                    rule_pattern: rule.mime_pattern.clone(),
                };

                if !dry_run {
                    let path = self.dir.join(&blob.hash);
                    let _ = std::fs::remove_file(&path);
                    // Keep metadata so we can return 410 Gone
                }

                events.push(event);
            }
        }

        events
    }

    /// Force a WAL checkpoint on the blob metadata database.
    /// Returns Ok(()) on success, Err on failure.
    pub fn wal_checkpoint(&self) -> Result<(), BlobError> {
        let db = self.db.lock()
            .map_err(|e| BlobError::DbError(format!("lock poisoned: {}", e)))?;
        db.execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
            .map_err(|e| BlobError::DbError(format!("wal checkpoint: {}", e)))?;
        Ok(())
    }

    /// Run a quick integrity check on the blob metadata database.
    pub fn integrity_check(&self) -> Result<(), BlobError> {
        let db = self.db.lock()
            .map_err(|e| BlobError::DbError(format!("lock poisoned: {}", e)))?;
        let result: String = db.query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|e| BlobError::DbError(format!("integrity check: {}", e)))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(BlobError::DbError(format!("integrity check failed: {}", result)))
        }
    }
}

/// A record of a blob that was (or would be) pruned.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PruneEvent {
    pub hash: String,
    pub chain_id: String,
    pub mime: String,
    pub age_days: f64,
    pub rule_pattern: String,
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn chain_usage_from_db(db: &Connection, chain_id: &str) -> u64 {
    db.query_row(
        "SELECT COALESCE(SUM(size), 0) FROM blob_meta WHERE chain_id = ?1",
        params![chain_id],
        |row| row.get::<_, i64>(0),
    ).unwrap_or(0) as u64
}

fn get_blob_meta(db: &Connection, hash_hex: &str) -> Option<BlobMeta> {
    db.query_row(
        "SELECT hash, chain_id, mime, size, uploaded_at FROM blob_meta WHERE hash = ?1",
        params![hash_hex],
        |row| Ok(BlobMeta {
            hash: row.get(0)?,
            chain_id: row.get(1)?,
            mime: row.get(2)?,
            size: row.get::<_, i64>(3)? as u64,
            uploaded_at: row.get(4)?,
        }),
    ).ok()
}

/// Validate that a hash string is exactly 64 lowercase hex characters.
fn validate_hash_hex(hash_hex: &str) -> Result<(), BlobError> {
    if hash_hex.len() != 64
        || !hash_hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
    {
        return Err(BlobError::InvalidHash);
    }
    Ok(())
}

// ── Blob Policy ─────────────────────────────────────────────────────

/// A parsed blob retention rule from a chain's BLOB_POLICY.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlobRule {
    pub mime_pattern: String,
    pub max_blob_size: Option<u64>,
    /// Raw AO timestamp value (Unix seconds × 189_000_000). Divide by 189_000_000 to get seconds.
    pub retention_raw: Option<i64>,
    pub priority: Option<u64>,
}

impl BlobRule {
    /// Get retention duration in seconds, or None if not set.
    pub fn retention_seconds(&self) -> Option<i64> {
        self.retention_raw.map(|raw| raw / 189_000_000)
    }
}

/// Parsed BLOB_POLICY from a chain's genesis block.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BlobPolicy {
    pub rules: Vec<BlobRule>,
    pub capacity_limit: Option<u64>,
    pub throttle_threshold: Option<u64>,
}

impl BlobPolicy {
    /// Extract BLOB_POLICY from a genesis DataItem. Returns None if no policy exists.
    pub fn from_genesis(genesis: &DataItem) -> Option<Self> {
        let policy = genesis.find_child(typecode::BLOB_POLICY)?;
        let children = policy.children();

        let mut rules = Vec::new();
        for child in children {
            if child.type_code == typecode::BLOB_RULE {
                let mime = child.find_child(typecode::MIME_PATTERN)
                    .and_then(|c| c.as_bytes())
                    .and_then(|b| std::str::from_utf8(b).ok())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                if mime.is_empty() { continue; }

                let max_blob_size = child.find_child(typecode::MAX_BLOB_SIZE)
                    .and_then(|c| c.as_bytes())
                    .and_then(|b| {
                        let (v, _) = ao_types::bigint::decode_bigint(b, 0).ok()?;
                        u64::try_from(v).ok()
                    });

                let retention_raw = child.find_child(typecode::RETENTION_SECS)
                    .and_then(|c| c.as_bytes())
                    .filter(|b| b.len() == 8)
                    .map(|b| i64::from_be_bytes(b.try_into().unwrap()));

                let priority = child.find_child(typecode::PRIORITY)
                    .and_then(|c| c.as_vbc_value());

                rules.push(BlobRule { mime_pattern: mime, max_blob_size, retention_raw, priority });
            }
        }

        let capacity_limit = extract_bigint_u64(policy, typecode::CAPACITY_LIMIT);
        let throttle_threshold = extract_bigint_u64(policy, typecode::THROTTLE_THRESHOLD);

        if rules.is_empty() { return None; }
        Some(BlobPolicy { rules, capacity_limit, throttle_threshold })
    }

    /// Find the first matching rule for a given MIME type. Returns None if no rules match.
    pub fn find_rule(&self, mime: &str) -> Option<&BlobRule> {
        let lower = mime.to_ascii_lowercase();
        self.rules.iter().find(|r| mime_matches(&r.mime_pattern, &lower))
    }
}

/// Check if a MIME type matches a glob-style pattern (only supports `*` wildcards).
fn mime_matches(pattern: &str, mime: &str) -> bool {
    let pat = pattern.to_ascii_lowercase();
    if pat == "*/*" { return true; }
    if let Some(prefix) = pat.strip_suffix("/*") {
        // Match "image/*" against "image/png" but not "imagery/foo"
        let expected = format!("{}/", prefix);
        return mime.starts_with(&expected);
    }
    pat == mime
}

/// Extract a bigint-encoded u64 child from a DataItem.
fn extract_bigint_u64(parent: &DataItem, code: i64) -> Option<u64> {
    parent.find_child(code)
        .and_then(|c| c.as_bytes())
        .and_then(|b| {
            let (v, _) = ao_types::bigint::decode_bigint(b, 0).ok()?;
            u64::try_from(v).ok()
        })
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

    // Verify chain exists (return value unused — policy comes from cache).
    state.get_chain_or_err(&chain_id).map_err(|_| BlobError::ChainNotFound)?;
    let content_size = data.len() - (mime.len() + 1); // content bytes after MIME NUL
    if let Some(policy) = state.get_blob_policy(&chain_id) {
        if let Some(rule) = policy.find_rule(mime) {
            if let Some(max) = rule.max_blob_size {
                if content_size as u64 > max {
                    return Err(BlobError::TooLarge {
                        size: content_size,
                        max: max as usize,
                    });
                }
            }
        }
        // If no rule matches, the blob's MIME type isn't covered by policy.
        // Default behavior: allow (the global MIME allowlist already filtered).
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

/// HEAD /chain/{id}/blob/{hash} — existence/age check without body transfer.
pub async fn head_blob(
    State(state): State<Arc<AppState>>,
    Path((chain_id, hash)): Path<(String, String)>,
) -> Result<impl IntoResponse, BlobError> {
    state.get_chain_or_err(&chain_id).map_err(|_| BlobError::ChainNotFound)?;

    let blob_store = state
        .blob_store
        .as_ref()
        .ok_or_else(|| BlobError::IoError(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "blob storage not configured",
        )))?;

    validate_hash_hex(&hash)?;

    let meta = blob_store.get_meta(&hash).ok_or(BlobError::NotFound)?;
    if meta.chain_id != chain_id {
        return Err(BlobError::NotFound);
    }

    // Check if file still exists (might be pruned)
    let file_exists = blob_store.exists(&hash);
    let status = if file_exists { StatusCode::OK } else { StatusCode::GONE };

    Ok((
        status,
        [
            // Content-Length reports content-only size (excluding MIME prefix + NUL),
            // matching what GET actually returns in the response body.
            (header::CONTENT_LENGTH, meta.size.saturating_sub(meta.mime.len() as u64 + 1).to_string()),
            (header::CONTENT_TYPE, meta.mime.clone()),
            (header::HeaderName::from_static("x-ao-uploaded-at"), meta.uploaded_at.to_string()),
        ],
    ))
}

/// GET /chain/{id}/blobs/manifest — paginated JSON metadata list.
pub async fn blob_manifest(
    State(state): State<Arc<AppState>>,
    Path(chain_id): Path<String>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<Vec<BlobMeta>>, BlobError> {
    state.get_chain_or_err(&chain_id).map_err(|_| BlobError::ChainNotFound)?;

    let blob_store = state
        .blob_store
        .as_ref()
        .ok_or_else(|| BlobError::IoError(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "blob storage not configured",
        )))?;

    let offset: u64 = params.get("offset").and_then(|s| s.parse().ok()).unwrap_or(0);
    let limit: u64 = params.get("limit").and_then(|s| s.parse().ok()).unwrap_or(100);
    let limit = limit.min(1000); // Cap at 1000

    let blobs = blob_store.list_chain_blobs(&chain_id, offset, limit);
    Ok(Json(blobs))
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

        // Only one blob file on disk (no temp files left behind).
        // Filter out SQLite files (blob_meta.db*).
        let entries: Vec<_> = std::fs::read_dir(&blob_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| !e.file_name().to_str().unwrap_or("").starts_with("blob_meta"))
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
    fn test_mime_matches() {
        assert!(mime_matches("image/*", "image/png"));
        assert!(mime_matches("image/*", "image/jpeg"));
        assert!(mime_matches("Image/*", "image/webp")); // case-insensitive
        assert!(mime_matches("*/*", "anything/goes"));
        assert!(mime_matches("application/pdf", "application/pdf"));
        assert!(!mime_matches("image/*", "application/pdf"));
        assert!(!mime_matches("application/pdf", "image/png"));
    }

    #[test]
    fn test_blob_policy_from_genesis() {
        use ao_types::dataitem::DataItem;
        use ao_types::typecode::*;
        use ao_types::timestamp::Timestamp;
        use num_bigint::BigInt;

        let retention_7y = Timestamp::from_unix_seconds(220_752_000);
        let mut max5m = Vec::new();
        ao_types::bigint::encode_bigint(&BigInt::from(5_242_880u64), &mut max5m);
        let mut cap500g = Vec::new();
        ao_types::bigint::encode_bigint(&BigInt::from(536_870_912_000u64), &mut cap500g);

        let genesis = DataItem::container(GENESIS, vec![
            DataItem::vbc_value(PROTOCOL_VER, 1),
            DataItem::container(BLOB_POLICY, vec![
                DataItem::container(BLOB_RULE, vec![
                    DataItem::bytes(MIME_PATTERN, b"image/*".to_vec()),
                    DataItem::bytes(MAX_BLOB_SIZE, max5m),
                    DataItem::bytes(RETENTION_SECS, retention_7y.to_bytes().to_vec()),
                    DataItem::vbc_value(PRIORITY, 1),
                ]),
                DataItem::container(BLOB_RULE, vec![
                    DataItem::bytes(MIME_PATTERN, b"*/*".to_vec()),
                    DataItem::vbc_value(PRIORITY, 99),
                ]),
                DataItem::bytes(CAPACITY_LIMIT, cap500g),
            ]),
        ]);

        let policy = BlobPolicy::from_genesis(&genesis).unwrap();
        assert_eq!(policy.rules.len(), 2);
        assert_eq!(policy.rules[0].mime_pattern, "image/*");
        assert_eq!(policy.rules[0].max_blob_size, Some(5_242_880));
        assert_eq!(policy.rules[0].priority, Some(1));
        assert_eq!(policy.rules[1].mime_pattern, "*/*");
        assert_eq!(policy.rules[1].max_blob_size, None);
        assert_eq!(policy.capacity_limit, Some(536_870_912_000));

        // Rule matching
        let rule = policy.find_rule("image/png").unwrap();
        assert_eq!(rule.mime_pattern, "image/*");
        let rule2 = policy.find_rule("application/pdf").unwrap();
        assert_eq!(rule2.mime_pattern, "*/*");
    }

    #[test]
    fn test_blob_policy_none_without_policy() {
        use ao_types::dataitem::DataItem;
        use ao_types::typecode::*;

        let genesis = DataItem::container(GENESIS, vec![
            DataItem::vbc_value(PROTOCOL_VER, 1),
        ]);
        assert!(BlobPolicy::from_genesis(&genesis).is_none());
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

    #[test]
    fn test_get_meta_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"), 5_242_880).unwrap();

        let data1 = b"image/png\0aaaa";
        let data2 = b"text/plain\0bbbb";
        let h1 = store.store(data1, "chain_x").unwrap();
        let h2 = store.store(data2, "chain_x").unwrap();
        let _h3 = store.store(b"image/jpeg\0cccc", "chain_y").unwrap();

        // get_meta returns correct info
        let meta = store.get_meta(&h1).unwrap();
        assert_eq!(meta.chain_id, "chain_x");
        assert_eq!(meta.mime, "image/png");
        assert_eq!(meta.size, data1.len() as u64);

        // list_chain_blobs filters by chain
        let list = store.list_chain_blobs("chain_x", 0, 100);
        assert_eq!(list.len(), 2);
        assert!(list.iter().any(|m| m.hash == h1));
        assert!(list.iter().any(|m| m.hash == h2));

        let list_y = store.list_chain_blobs("chain_y", 0, 100);
        assert_eq!(list_y.len(), 1);

        // Pagination
        let page1 = store.list_chain_blobs("chain_x", 0, 1);
        assert_eq!(page1.len(), 1);
        let page2 = store.list_chain_blobs("chain_x", 1, 1);
        assert_eq!(page2.len(), 1);
        assert_ne!(page1[0].hash, page2[0].hash);
    }

    #[test]
    fn test_pruned_blob_returns_410() {
        let dir = tempfile::tempdir().unwrap();
        let blob_dir = dir.path().join("blobs");
        let store = BlobStore::new(blob_dir.clone(), 5_242_880).unwrap();

        let data = b"image/png\0prunable";
        let hash = store.store(data, "chain_p").unwrap();

        // Verify blob is retrievable
        assert!(store.get(&hash, "chain_p").unwrap().is_some());

        // Simulate pruning: delete the file but keep metadata
        std::fs::remove_file(blob_dir.join(&hash)).unwrap();

        // Now get should return Pruned error (410 Gone)
        let result = store.get(&hash, "chain_p");
        assert!(matches!(result, Err(BlobError::Pruned { .. })));

        // Metadata should still exist
        assert!(store.get_meta(&hash).is_some());
    }

    #[test]
    fn test_prune_expired_blobs() {
        let dir = tempfile::tempdir().unwrap();
        let blob_dir = dir.path().join("blobs");
        let store = BlobStore::new(blob_dir.clone(), 5_242_880).unwrap();

        let data = b"image/png\0prunable_content";
        let hash = store.store(data, "chain_pr").unwrap();

        // Backdate the uploaded_at to simulate an old blob (2 days ago)
        {
            let db = store.db.lock().unwrap();
            let old_ts = unix_now() - 2 * 86400;
            db.execute(
                "UPDATE blob_meta SET uploaded_at = ?1 WHERE hash = ?2",
                params![old_ts, hash],
            ).unwrap();
        }

        // Create a policy with 1-day retention for image/*
        let mut policies = HashMap::new();
        policies.insert("chain_pr".to_string(), BlobPolicy {
            rules: vec![BlobRule {
                mime_pattern: "image/*".into(),
                retention_raw: Some(86400 * 189_000_000), // 1 day in AO timestamps
                max_blob_size: None,
                priority: None,
            }],
            capacity_limit: None,
            throttle_threshold: None,
        });

        // Dry run should report without deleting
        let dry_events = store.prune(&policies, true);
        assert_eq!(dry_events.len(), 1);
        assert!(blob_dir.join(&hash).exists()); // File still there

        // Real prune should delete the file
        let events = store.prune(&policies, false);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].hash, hash);
        assert!(!blob_dir.join(&hash).exists()); // File deleted

        // Metadata still exists for 410
        assert!(store.get_meta(&hash).is_some());
    }

    #[test]
    fn test_prune_keeps_unexpired_blobs() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"), 5_242_880).unwrap();

        let data = b"image/png\0fresh";
        let hash = store.store(data, "chain_k").unwrap();

        // Policy with 30-day retention — blob is fresh, should NOT be pruned
        let mut policies = HashMap::new();
        policies.insert("chain_k".to_string(), BlobPolicy {
            rules: vec![BlobRule {
                mime_pattern: "image/*".into(),
                retention_raw: Some(30 * 86400 * 189_000_000),
                max_blob_size: None,
                priority: None,
            }],
            capacity_limit: None,
            throttle_threshold: None,
        });

        let events = store.prune(&policies, false);
        assert!(events.is_empty());
        assert!(store.get(&hash, "chain_k").unwrap().is_some());
    }

    #[test]
    fn test_prune_no_policy_means_no_pruning() {
        let dir = tempfile::tempdir().unwrap();
        let store = BlobStore::new(dir.path().join("blobs"), 5_242_880).unwrap();

        store.store(b"text/plain\0data", "chain_np").unwrap();

        // Empty policies map → nothing pruned
        let policies: HashMap<String, BlobPolicy> = HashMap::new();
        let events = store.prune(&policies, false);
        assert!(events.is_empty());
    }
}
