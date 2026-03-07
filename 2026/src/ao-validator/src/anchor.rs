use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// An anchor entry written to the append-only log.
#[derive(Serialize, Deserialize, Debug)]
struct AnchorEntry {
    chain_id: String,
    height: u64,
    rolled_hash: String,
    timestamp: i64,
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

    /// Append an anchor entry.
    pub fn publish(
        &self,
        chain_id: &str,
        height: u64,
        rolled_hash: &[u8; 32],
    ) -> Result<String> {
        use std::io::Write;

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

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

    /// Verify that an anchor at the given height matches the expected hash.
    pub fn verify(&self, chain_id: &str, height: u64, expected_hash: &[u8; 32]) -> Result<bool> {
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
}
