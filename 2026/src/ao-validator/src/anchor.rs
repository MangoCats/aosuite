use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// An anchor entry written to the append-only log.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnchorEntry {
    pub chain_id: String,
    pub height: u64,
    pub rolled_hash: String,
    pub timestamp: i64,
}

/// Pluggable anchor backend trait (N29).
/// Implementations publish anchors to durable storage and verify past anchors.
pub trait AnchorBackend: Send + Sync {
    /// Publish an anchor entry, returns backend-specific anchor_ref string.
    fn publish(
        &self,
        chain_id: &str,
        height: u64,
        rolled_hash: &[u8; 32],
    ) -> Result<String>;

    /// Verify that an anchor at the given height matches the expected hash.
    fn verify(
        &self,
        chain_id: &str,
        height: u64,
        expected_hash: &[u8; 32],
    ) -> Result<bool>;
}

/// File-based anchor backend — writes anchors as JSON lines to a local file.
/// Provides tamper-evidence: the file can be independently verified.
pub struct FileAnchor {
    path: PathBuf,
}

impl FileAnchor {
    pub fn new(path: PathBuf) -> Self {
        FileAnchor { path }
    }

    /// Get the file path (for display in anchor_ref).
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl AnchorBackend for FileAnchor {
    fn publish(
        &self,
        chain_id: &str,
        height: u64,
        rolled_hash: &[u8; 32],
    ) -> Result<String> {
        use std::io::Write;

        let timestamp = unix_now();
        let entry = AnchorEntry {
            chain_id: chain_id.to_string(),
            height,
            rolled_hash: hex::encode(rolled_hash),
            timestamp,
        };

        let line = serde_json::to_string(&entry)?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{}", line)?;

        let anchor_ref = format!("file:{}:{}", self.path.display(), height);
        Ok(anchor_ref)
    }

    fn verify(&self, chain_id: &str, height: u64, expected_hash: &[u8; 32]) -> Result<bool> {
        let content = std::fs::read_to_string(&self.path)?;
        let expected_hex = hex::encode(expected_hash);

        for line in content.lines() {
            if let Ok(entry) = serde_json::from_str::<AnchorEntry>(line)
                && entry.chain_id == chain_id && entry.height == height
            {
                return Ok(entry.rolled_hash == expected_hex);
            }
        }

        Ok(false) // no matching anchor found
    }
}

/// Replicated anchor backend — publishes to a primary backend plus replica paths.
/// Addresses disk-failure risk by writing to multiple locations.
pub struct ReplicatedAnchor {
    primary: FileAnchor,
    replicas: Vec<FileAnchor>,
}

impl ReplicatedAnchor {
    pub fn new(primary_path: PathBuf, replica_paths: Vec<PathBuf>) -> Self {
        ReplicatedAnchor {
            primary: FileAnchor::new(primary_path),
            replicas: replica_paths.into_iter().map(FileAnchor::new).collect(),
        }
    }
}

impl AnchorBackend for ReplicatedAnchor {
    fn publish(
        &self,
        chain_id: &str,
        height: u64,
        rolled_hash: &[u8; 32],
    ) -> Result<String> {
        // Primary must succeed
        let anchor_ref = self.primary.publish(chain_id, height, rolled_hash)?;

        // Replicas: best-effort (log errors but don't fail)
        for (i, replica) in self.replicas.iter().enumerate() {
            if let Err(e) = replica.publish(chain_id, height, rolled_hash) {
                tracing::warn!(
                    replica = i,
                    path = %replica.path().display(),
                    "Replica anchor publish failed: {}",
                    e,
                );
            }
        }

        Ok(anchor_ref)
    }

    fn verify(&self, chain_id: &str, height: u64, expected_hash: &[u8; 32]) -> Result<bool> {
        // Verify against primary
        self.primary.verify(chain_id, height, expected_hash)
    }
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_anchor_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchors.jsonl");
        let anchor = FileAnchor::new(path.clone());

        let hash = [0xAA; 32];
        let anchor_ref = anchor.publish("chain1", 100, &hash).unwrap();
        assert!(anchor_ref.contains("100"));

        // Verify correct hash
        assert!(anchor.verify("chain1", 100, &hash).unwrap());

        // Verify wrong hash
        let bad_hash = [0xBB; 32];
        assert!(!anchor.verify("chain1", 100, &bad_hash).unwrap());

        // Verify nonexistent height
        assert!(!anchor.verify("chain1", 200, &hash).unwrap());
    }

    #[test]
    fn test_file_anchor_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchors.jsonl");
        let anchor = FileAnchor::new(path);

        let h1 = [0x11; 32];
        let h2 = [0x22; 32];
        anchor.publish("chain1", 100, &h1).unwrap();
        anchor.publish("chain1", 200, &h2).unwrap();

        assert!(anchor.verify("chain1", 100, &h1).unwrap());
        assert!(anchor.verify("chain1", 200, &h2).unwrap());
    }

    #[test]
    fn test_replicated_anchor() {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("primary.jsonl");
        let replica1 = dir.path().join("replica1.jsonl");
        let replica2 = dir.path().join("replica2.jsonl");

        let anchor = ReplicatedAnchor::new(
            primary.clone(),
            vec![replica1.clone(), replica2.clone()],
        );

        let hash = [0xCC; 32];
        let anchor_ref = anchor.publish("chain1", 50, &hash).unwrap();
        assert!(anchor_ref.contains("50"));

        // Primary should verify
        assert!(anchor.verify("chain1", 50, &hash).unwrap());

        // Replicas should also contain the entry
        let r1 = FileAnchor::new(replica1);
        assert!(r1.verify("chain1", 50, &hash).unwrap());
        let r2 = FileAnchor::new(replica2);
        assert!(r2.verify("chain1", 50, &hash).unwrap());
    }

    #[test]
    fn test_replicated_anchor_replica_failure() {
        let dir = tempfile::tempdir().unwrap();
        let primary = dir.path().join("primary.jsonl");
        // Invalid replica path — should fail gracefully
        let bad_replica = PathBuf::from("/nonexistent/dir/replica.jsonl");

        let anchor = ReplicatedAnchor::new(primary, vec![bad_replica]);

        let hash = [0xDD; 32];
        // Should succeed (primary writes fine, replica fails silently)
        let result = anchor.publish("chain1", 10, &hash);
        assert!(result.is_ok());
        assert!(anchor.verify("chain1", 10, &hash).unwrap());
    }

    #[test]
    fn test_anchor_backend_trait() {
        // Test that FileAnchor can be used as a trait object
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("anchors.jsonl");
        let backend: Box<dyn AnchorBackend> = Box::new(FileAnchor::new(path));

        let hash = [0xEE; 32];
        backend.publish("chain1", 1, &hash).unwrap();
        assert!(backend.verify("chain1", 1, &hash).unwrap());
    }
}
