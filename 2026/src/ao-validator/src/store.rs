use rusqlite::{Connection, params};
use anyhow::Result;

/// Per-chain validation state.
#[derive(Debug, Clone)]
pub struct ChainValidationState {
    pub chain_id: String,
    pub validated_height: u64,
    pub rolled_hash: [u8; 32],
    pub last_poll_timestamp: i64,
    pub status: String,
    pub alert_message: Option<String>,
}

/// An external anchor record.
#[derive(Debug, Clone)]
pub struct AnchorRecord {
    pub chain_id: String,
    pub height: u64,
    pub rolled_hash: [u8; 32],
    pub anchor_ref: String,
    pub anchor_timestamp: i64,
}

/// SQLite-backed store for validator state.
pub struct ValidatorStore {
    conn: Connection,
}

impl ValidatorStore {
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        let store = ValidatorStore { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = ValidatorStore { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chain_state (
                chain_id TEXT PRIMARY KEY,
                validated_height INTEGER NOT NULL DEFAULT 0,
                rolled_hash BLOB NOT NULL,
                last_poll_timestamp INTEGER NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'ok',
                alert_message TEXT
            );
            CREATE TABLE IF NOT EXISTS anchors (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                chain_id TEXT NOT NULL,
                height INTEGER NOT NULL,
                rolled_hash BLOB NOT NULL,
                anchor_ref TEXT NOT NULL,
                anchor_timestamp INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_anchors_chain ON anchors(chain_id, height);"
        )?;
        Ok(())
    }

    /// Get validation state for a chain, or None if not yet tracked.
    pub fn get_chain_state(&self, chain_id: &str) -> Result<Option<ChainValidationState>> {
        let mut stmt = self.conn.prepare(
            "SELECT chain_id, validated_height, rolled_hash, last_poll_timestamp, status, alert_message
             FROM chain_state WHERE chain_id = ?1"
        )?;
        let mut rows = stmt.query(params![chain_id])?;
        match rows.next()? {
            Some(row) => {
                let hash_bytes: Vec<u8> = row.get(2)?;
                let rolled_hash: [u8; 32] = hash_bytes.try_into()
                    .map_err(|_| anyhow::anyhow!("rolled_hash in DB is not 32 bytes"))?;
                Ok(Some(ChainValidationState {
                    chain_id: row.get(0)?,
                    validated_height: row.get::<_, i64>(1)? as u64,
                    rolled_hash,
                    last_poll_timestamp: row.get(3)?,
                    status: row.get(4)?,
                    alert_message: row.get(5)?,
                }))
            }
            None => Ok(None),
        }
    }

    /// Initialize or update chain validation state.
    pub fn update_chain_state(
        &self,
        chain_id: &str,
        validated_height: u64,
        rolled_hash: &[u8; 32],
        status: &str,
        alert_message: Option<&str>,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn.execute(
            "INSERT INTO chain_state (chain_id, validated_height, rolled_hash, last_poll_timestamp, status, alert_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(chain_id) DO UPDATE SET
                validated_height = ?2, rolled_hash = ?3, last_poll_timestamp = ?4,
                status = ?5, alert_message = ?6",
            params![chain_id, validated_height as i64, &rolled_hash[..], now, status, alert_message],
        )?;
        Ok(())
    }

    /// Record an external anchor.
    pub fn record_anchor(
        &self,
        chain_id: &str,
        height: u64,
        rolled_hash: &[u8; 32],
        anchor_ref: &str,
        anchor_timestamp: i64,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO anchors (chain_id, height, rolled_hash, anchor_ref, anchor_timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![chain_id, height as i64, &rolled_hash[..], anchor_ref, anchor_timestamp],
        )?;
        Ok(())
    }

    /// Get the latest anchor for a chain.
    pub fn get_latest_anchor(&self, chain_id: &str) -> Result<Option<AnchorRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT chain_id, height, rolled_hash, anchor_ref, anchor_timestamp
             FROM anchors WHERE chain_id = ?1 ORDER BY height DESC LIMIT 1"
        )?;
        let mut rows = stmt.query(params![chain_id])?;
        match rows.next()? {
            Some(row) => {
                let hash_bytes: Vec<u8> = row.get(2)?;
                let rolled_hash: [u8; 32] = hash_bytes.try_into()
                    .map_err(|_| anyhow::anyhow!("rolled_hash in DB is not 32 bytes"))?;
                Ok(Some(AnchorRecord {
                    chain_id: row.get(0)?,
                    height: row.get::<_, i64>(1)? as u64,
                    rolled_hash,
                    anchor_ref: row.get(3)?,
                    anchor_timestamp: row.get(4)?,
                }))
            }
            None => Ok(None),
        }
    }

    /// Get all tracked chain IDs.
    pub fn all_chain_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT chain_id FROM chain_state ORDER BY chain_id")?;
        let mut rows = stmt.query([])?;
        let mut ids = Vec::new();
        while let Some(row) = rows.next()? {
            ids.push(row.get(0)?);
        }
        Ok(ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_state_lifecycle() {
        let store = ValidatorStore::open_memory().unwrap();

        // Initially empty
        assert!(store.get_chain_state("abc123").unwrap().is_none());

        // Insert
        let hash = [0xAA; 32];
        store.update_chain_state("abc123", 10, &hash, "ok", None).unwrap();

        let state = store.get_chain_state("abc123").unwrap().unwrap();
        assert_eq!(state.validated_height, 10);
        assert_eq!(state.rolled_hash, hash);
        assert_eq!(state.status, "ok");
        assert!(state.alert_message.is_none());

        // Update (upsert)
        let hash2 = [0xBB; 32];
        store.update_chain_state("abc123", 20, &hash2, "alert", Some("tampered at 15")).unwrap();

        let state = store.get_chain_state("abc123").unwrap().unwrap();
        assert_eq!(state.validated_height, 20);
        assert_eq!(state.rolled_hash, hash2);
        assert_eq!(state.status, "alert");
        assert_eq!(state.alert_message.as_deref(), Some("tampered at 15"));
    }

    #[test]
    fn test_anchor_storage() {
        let store = ValidatorStore::open_memory().unwrap();

        // No anchors initially
        assert!(store.get_latest_anchor("abc123").unwrap().is_none());

        let hash1 = [0x11; 32];
        let hash2 = [0x22; 32];
        store.record_anchor("abc123", 100, &hash1, "file:anchor_100.json", 1000).unwrap();
        store.record_anchor("abc123", 200, &hash2, "file:anchor_200.json", 2000).unwrap();

        let latest = store.get_latest_anchor("abc123").unwrap().unwrap();
        assert_eq!(latest.height, 200);
        assert_eq!(latest.rolled_hash, hash2);
        assert_eq!(latest.anchor_ref, "file:anchor_200.json");
    }

    #[test]
    fn test_all_chain_ids() {
        let store = ValidatorStore::open_memory().unwrap();
        store.update_chain_state("chain_b", 0, &[0; 32], "ok", None).unwrap();
        store.update_chain_state("chain_a", 0, &[0; 32], "ok", None).unwrap();

        let ids = store.all_chain_ids().unwrap();
        assert_eq!(ids, vec!["chain_a", "chain_b"]);
    }
}
