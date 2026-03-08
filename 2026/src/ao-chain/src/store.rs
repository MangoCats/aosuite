use rusqlite::{Connection, params};
use num_bigint::BigInt;
use num_traits::Zero;

use crate::error::{ChainError, Result};

/// Status of a UTXO entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtxoStatus {
    Unspent,
    Spent,
    Expired,
    Escrowed,
}

impl UtxoStatus {
    fn as_str(&self) -> &'static str {
        match self {
            UtxoStatus::Unspent => "unspent",
            UtxoStatus::Spent => "spent",
            UtxoStatus::Expired => "expired",
            UtxoStatus::Escrowed => "escrowed",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "unspent" => Some(UtxoStatus::Unspent),
            "spent" => Some(UtxoStatus::Spent),
            "expired" => Some(UtxoStatus::Expired),
            "escrowed" => Some(UtxoStatus::Escrowed),
            _ => None,
        }
    }
}

/// A single UTXO record.
#[derive(Debug, Clone)]
pub struct Utxo {
    pub seq_id: u64,
    pub pubkey: [u8; 32],
    pub amount: BigInt,
    pub block_height: u64,
    pub block_timestamp: i64,
    pub status: UtxoStatus,
}

/// A CAA escrow record.
#[derive(Debug, Clone)]
pub struct CaaEscrow {
    pub caa_hash: [u8; 32],
    pub chain_order: u64,
    pub deadline: i64,
    pub status: String,
    pub block_height: u64,
    pub proof_data: Option<Vec<u8>>,
    pub total_chains: u64,
    /// Bond amount forfeited on timeout (zero if last chain or no bond).
    pub bond_amount: BigInt,
}

/// Chain metadata stored in the database.
#[derive(Debug, Clone)]
pub struct ChainMeta {
    pub chain_id: [u8; 32],
    pub symbol: String,
    pub coin_count: BigInt,
    pub shares_out: BigInt,
    pub fee_rate_num: BigInt,
    pub fee_rate_den: BigInt,
    pub expiry_period: i64,
    pub expiry_mode: u64,
    pub tax_start_age: Option<i64>,
    pub tax_doubling_period: Option<i64>,
    pub block_height: u64,
    pub next_seq_id: u64,
    pub last_block_timestamp: i64,
    pub prev_hash: [u8; 32],
}

/// SQLite-backed chain state store.
pub struct ChainStore {
    conn: Connection,
}

impl ChainStore {
    /// Open or create a chain store at the given path.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Ok(ChainStore { conn })
    }

    /// Create an in-memory store (for testing).
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        Ok(ChainStore { conn })
    }

    /// Initialize the schema.
    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS chain_meta (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS utxos (
                seq_id INTEGER PRIMARY KEY,
                pubkey BLOB NOT NULL,
                amount BLOB NOT NULL,
                block_height INTEGER NOT NULL,
                block_timestamp INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'unspent'
            );
            CREATE INDEX IF NOT EXISTS idx_utxo_status ON utxos(status);
            CREATE INDEX IF NOT EXISTS idx_utxo_pubkey ON utxos(pubkey);
            CREATE TABLE IF NOT EXISTS blocks (
                height INTEGER PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                hash BLOB NOT NULL,
                data BLOB NOT NULL
            );
            CREATE TABLE IF NOT EXISTS refutations (
                agreement_hash BLOB PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS used_keys (
                pubkey BLOB PRIMARY KEY
            );
            CREATE TABLE IF NOT EXISTS caa_escrows (
                caa_hash BLOB PRIMARY KEY,
                chain_order INTEGER NOT NULL,
                deadline INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'escrowed',
                block_height INTEGER NOT NULL,
                proof_data BLOB,
                total_chains INTEGER NOT NULL DEFAULT 0,
                bond_amount BLOB NOT NULL DEFAULT X''
            );
            CREATE TABLE IF NOT EXISTS caa_utxos (
                caa_hash BLOB NOT NULL,
                seq_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                PRIMARY KEY (caa_hash, seq_id)
            );
            CREATE TABLE IF NOT EXISTS escrow_releases (
                seq_id INTEGER PRIMARY KEY,
                released_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS caa_giver_history (
                pubkey BLOB NOT NULL,
                caa_hash BLOB NOT NULL,
                escrowed_at INTEGER NOT NULL,
                PRIMARY KEY (pubkey, caa_hash)
            );
            CREATE INDEX IF NOT EXISTS idx_caa_giver_pubkey ON caa_giver_history(pubkey);
            CREATE TABLE IF NOT EXISTS known_recorder_keys (
                chain_id BLOB NOT NULL,
                pubkey BLOB NOT NULL,
                added_at INTEGER NOT NULL,
                revoked_at INTEGER,
                PRIMARY KEY (chain_id, pubkey)
            );
            CREATE TABLE IF NOT EXISTS vendor_profile (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                name TEXT,
                description TEXT,
                lat REAL,
                lon REAL,
                updated_at INTEGER NOT NULL
            );"
        )?;
        Ok(())
    }

    // --- Chain metadata ---

    pub fn set_meta(&self, key: &str, value: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO chain_meta (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let mut stmt = self.conn.prepare("SELECT value FROM chain_meta WHERE key = ?1")?;
        let mut rows = stmt.query(params![key])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    fn set_meta_u64(&self, key: &str, value: u64) -> Result<()> {
        self.set_meta(key, &value.to_be_bytes())
    }

    fn get_meta_u64(&self, key: &str) -> Result<Option<u64>> {
        match self.get_meta(key)? {
            Some(v) if v.len() == 8 => {
                Ok(Some(u64::from_be_bytes(v.try_into().expect("length matched"))))
            }
            _ => Ok(None),
        }
    }

    fn set_meta_i64(&self, key: &str, value: i64) -> Result<()> {
        self.set_meta(key, &value.to_be_bytes())
    }

    fn get_meta_i64(&self, key: &str) -> Result<Option<i64>> {
        match self.get_meta(key)? {
            Some(v) if v.len() == 8 => {
                Ok(Some(i64::from_be_bytes(v.try_into().expect("length matched"))))
            }
            _ => Ok(None),
        }
    }

    fn set_meta_bigint(&self, key: &str, value: &BigInt) -> Result<()> {
        self.set_meta(key, &value.to_signed_bytes_be())
    }

    fn get_meta_bigint(&self, key: &str) -> Result<Option<BigInt>> {
        match self.get_meta(key)? {
            Some(v) if v.is_empty() => Ok(Some(BigInt::zero())),
            Some(v) => Ok(Some(BigInt::from_signed_bytes_be(&v))),
            None => Ok(None),
        }
    }

    /// Store the full chain metadata after genesis loading.
    pub fn store_chain_meta(&self, meta: &ChainMeta) -> Result<()> {
        self.set_meta("chain_id", &meta.chain_id)?;
        self.set_meta("symbol", meta.symbol.as_bytes())?;
        self.set_meta_bigint("coin_count", &meta.coin_count)?;
        self.set_meta_bigint("shares_out", &meta.shares_out)?;
        self.set_meta_bigint("fee_rate_num", &meta.fee_rate_num)?;
        self.set_meta_bigint("fee_rate_den", &meta.fee_rate_den)?;
        self.set_meta_i64("expiry_period", meta.expiry_period)?;
        self.set_meta_u64("expiry_mode", meta.expiry_mode)?;
        if let Some(v) = meta.tax_start_age {
            self.set_meta_i64("tax_start_age", v)?;
        }
        if let Some(v) = meta.tax_doubling_period {
            self.set_meta_i64("tax_doubling_period", v)?;
        }
        self.set_meta_u64("block_height", meta.block_height)?;
        self.set_meta_u64("next_seq_id", meta.next_seq_id)?;
        self.set_meta_i64("last_block_timestamp", meta.last_block_timestamp)?;
        self.set_meta("prev_hash", &meta.prev_hash)?;
        Ok(())
    }

    /// Load chain metadata.
    pub fn load_chain_meta(&self) -> Result<Option<ChainMeta>> {
        let chain_id = match self.get_meta("chain_id")? {
            Some(v) if v.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&v);
                arr
            }
            _ => return Ok(None),
        };
        let symbol = match self.get_meta("symbol")? {
            Some(v) => String::from_utf8(v)
                .map_err(|_| ChainError::Encoding("chain symbol is not valid UTF-8".into()))?,
            None => return Ok(None),
        };
        let coin_count = self.get_meta_bigint("coin_count")?.unwrap_or_else(BigInt::zero);
        let shares_out = self.get_meta_bigint("shares_out")?.unwrap_or_else(BigInt::zero);
        let fee_rate_num = self.get_meta_bigint("fee_rate_num")?.unwrap_or_else(BigInt::zero);
        let fee_rate_den = self.get_meta_bigint("fee_rate_den")?.unwrap_or_else(BigInt::zero);
        let expiry_period = self.get_meta_i64("expiry_period")?.unwrap_or(0);
        let expiry_mode = self.get_meta_u64("expiry_mode")?.unwrap_or(1);
        let tax_start_age = self.get_meta_i64("tax_start_age")?;
        let tax_doubling_period = self.get_meta_i64("tax_doubling_period")?;
        let block_height = self.get_meta_u64("block_height")?.unwrap_or(0);
        let next_seq_id = self.get_meta_u64("next_seq_id")?.unwrap_or(1);
        let last_block_timestamp = self.get_meta_i64("last_block_timestamp")?.unwrap_or(0);
        let prev_hash = match self.get_meta("prev_hash")? {
            Some(v) if v.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&v);
                arr
            }
            _ => [0u8; 32],
        };

        Ok(Some(ChainMeta {
            chain_id, symbol, coin_count, shares_out,
            fee_rate_num, fee_rate_den, expiry_period, expiry_mode,
            tax_start_age, tax_doubling_period,
            block_height, next_seq_id, last_block_timestamp, prev_hash,
        }))
    }

    /// Update shares_out after fee deduction or expiration.
    pub fn update_shares_out(&self, new_shares_out: &BigInt) -> Result<()> {
        self.set_meta_bigint("shares_out", new_shares_out)
    }

    /// Advance block height, prev_hash, and last_block_timestamp.
    pub fn advance_block(&self, height: u64, timestamp: i64, hash: &[u8; 32]) -> Result<()> {
        self.set_meta_u64("block_height", height)?;
        self.set_meta_i64("last_block_timestamp", timestamp)?;
        self.set_meta("prev_hash", hash)?;
        Ok(())
    }

    /// Update next_seq_id.
    pub fn set_next_seq_id(&self, next: u64) -> Result<()> {
        self.set_meta_u64("next_seq_id", next)
    }

    // --- UTXO operations ---

    /// Insert a new UTXO (receiver in a recorded assignment).
    pub fn insert_utxo(&self, utxo: &Utxo) -> Result<()> {
        let amount_bytes = utxo.amount.to_signed_bytes_be();
        self.conn.execute(
            "INSERT INTO utxos (seq_id, pubkey, amount, block_height, block_timestamp, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                utxo.seq_id as i64,
                &utxo.pubkey[..],
                amount_bytes,
                utxo.block_height as i64,
                utxo.block_timestamp,
                utxo.status.as_str(),
            ],
        )?;
        Ok(())
    }

    /// Get a UTXO by sequence ID.
    pub fn get_utxo(&self, seq_id: u64) -> Result<Option<Utxo>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq_id, pubkey, amount, block_height, block_timestamp, status
             FROM utxos WHERE seq_id = ?1"
        )?;
        let mut rows = stmt.query(params![seq_id as i64])?;
        match rows.next()? {
            Some(row) => {
                let sid: i64 = row.get(0)?;
                let pk: Vec<u8> = row.get(1)?;
                let amt: Vec<u8> = row.get(2)?;
                let bh: i64 = row.get(3)?;
                let bt: i64 = row.get(4)?;
                let st: String = row.get(5)?;
                if pk.len() != 32 {
                    return Err(ChainError::Encoding(
                        format!("UTXO seq {} pubkey blob is {} bytes, expected 32", sid, pk.len())));
                }
                let mut pubkey = [0u8; 32];
                pubkey.copy_from_slice(&pk);
                let amount = if amt.is_empty() { BigInt::zero() } else { BigInt::from_signed_bytes_be(&amt) };
                let status = UtxoStatus::from_str(&st)
                    .ok_or_else(|| ChainError::Encoding(
                        format!("UTXO seq {} has unknown status '{}'", sid, st)))?;
                Ok(Some(Utxo {
                    seq_id: sid as u64,
                    pubkey,
                    amount,
                    block_height: bh as u64,
                    block_timestamp: bt,
                    status,
                }))
            }
            None => Ok(None),
        }
    }

    /// Mark a UTXO as spent (from unspent).
    pub fn mark_spent(&self, seq_id: u64) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE utxos SET status = 'spent' WHERE seq_id = ?1 AND status = 'unspent'",
            params![seq_id as i64],
        )?;
        if updated == 0 {
            return Err(ChainError::UtxoAlreadySpent(seq_id));
        }
        Ok(())
    }

    /// Mark a UTXO as escrowed (from unspent, for CAA).
    pub fn mark_escrowed(&self, seq_id: u64) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE utxos SET status = 'escrowed' WHERE seq_id = ?1 AND status = 'unspent'",
            params![seq_id as i64],
        )?;
        if updated == 0 {
            return Err(ChainError::UtxoAlreadySpent(seq_id));
        }
        Ok(())
    }

    /// Mark an escrowed UTXO as spent (CAA binding finalized).
    pub fn mark_escrowed_spent(&self, seq_id: u64) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE utxos SET status = 'spent' WHERE seq_id = ?1 AND status = 'escrowed'",
            params![seq_id as i64],
        )?;
        if updated == 0 {
            return Err(ChainError::InvalidAssignment(
                format!("UTXO {} is not in escrowed state", seq_id)));
        }
        Ok(())
    }

    /// Release an escrowed UTXO back to unspent (CAA timeout/expired).
    pub fn release_escrow(&self, seq_id: u64) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE utxos SET status = 'unspent' WHERE seq_id = ?1 AND status = 'escrowed'",
            params![seq_id as i64],
        )?;
        if updated == 0 {
            return Err(ChainError::InvalidAssignment(
                format!("UTXO {} is not in escrowed state", seq_id)));
        }
        Ok(())
    }

    /// Mark a UTXO as expired.
    pub fn mark_expired(&self, seq_id: u64) -> Result<()> {
        self.conn.execute(
            "UPDATE utxos SET status = 'expired' WHERE seq_id = ?1 AND status = 'unspent'",
            params![seq_id as i64],
        )?;
        Ok(())
    }

    /// Get all unspent UTXOs that have expired (block_timestamp + expiry_period < current_timestamp).
    pub fn find_expired_utxos(&self, current_timestamp: i64, expiry_period: i64) -> Result<Vec<Utxo>> {
        let cutoff = current_timestamp.saturating_sub(expiry_period);
        let mut stmt = self.conn.prepare(
            "SELECT seq_id, pubkey, amount, block_height, block_timestamp, status
             FROM utxos WHERE status = 'unspent' AND block_timestamp < ?1"
        )?;
        let mut utxos = Vec::new();
        let mut rows = stmt.query(params![cutoff])?;
        while let Some(row) = rows.next()? {
            let sid: i64 = row.get(0)?;
            let pk: Vec<u8> = row.get(1)?;
            let amt: Vec<u8> = row.get(2)?;
            let bh: i64 = row.get(3)?;
            let bt: i64 = row.get(4)?;
            let st: String = row.get(5)?;
            if pk.len() != 32 {
                return Err(ChainError::Encoding(
                    format!("UTXO seq {} pubkey blob is {} bytes, expected 32", sid, pk.len())));
            }
            let mut pubkey = [0u8; 32];
            pubkey.copy_from_slice(&pk);
            let amount = if amt.is_empty() { BigInt::zero() } else { BigInt::from_signed_bytes_be(&amt) };
            let status = UtxoStatus::from_str(&st)
                .ok_or_else(|| ChainError::Encoding(
                    format!("UTXO seq {} has unknown status '{}'", sid, st)))?;
            utxos.push(Utxo {
                seq_id: sid as u64, pubkey, amount,
                block_height: bh as u64, block_timestamp: bt,
                status,
            });
        }
        Ok(utxos)
    }

    // --- Key reuse tracking ---

    /// Check if a public key has been used as a receiver.
    pub fn is_key_used(&self, pubkey: &[u8; 32]) -> Result<bool> {
        let mut stmt = self.conn.prepare("SELECT 1 FROM used_keys WHERE pubkey = ?1")?;
        let mut rows = stmt.query(params![&pubkey[..]])?;
        Ok(rows.next()?.is_some())
    }

    /// Record a public key as used.
    pub fn mark_key_used(&self, pubkey: &[u8; 32]) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO used_keys (pubkey) VALUES (?1)",
            params![&pubkey[..]],
        )?;
        Ok(())
    }

    // --- Refutation tracking ---

    /// Record a refutation for an agreement hash.
    pub fn add_refutation(&self, agreement_hash: &[u8; 32]) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO refutations (agreement_hash) VALUES (?1)",
            params![&agreement_hash[..]],
        )?;
        Ok(())
    }

    /// Check if an agreement has been refuted.
    pub fn is_refuted(&self, agreement_hash: &[u8; 32]) -> Result<bool> {
        let mut stmt = self.conn.prepare("SELECT 1 FROM refutations WHERE agreement_hash = ?1")?;
        let mut rows = stmt.query(params![&agreement_hash[..]])?;
        Ok(rows.next()?.is_some())
    }

    // --- CAA escrow operations ---

    /// Record a CAA escrow entry.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_caa_escrow(
        &self,
        caa_hash: &[u8; 32],
        chain_order: u64,
        deadline: i64,
        block_height: u64,
        proof_data: Option<&[u8]>,
        total_chains: u64,
        bond_amount: &BigInt,
    ) -> Result<()> {
        let bond_bytes = bond_amount.to_signed_bytes_be();
        self.conn.execute(
            "INSERT INTO caa_escrows (caa_hash, chain_order, deadline, status, block_height, proof_data, total_chains, bond_amount)
             VALUES (?1, ?2, ?3, 'escrowed', ?4, ?5, ?6, ?7)",
            params![
                &caa_hash[..],
                chain_order as i64,
                deadline,
                block_height as i64,
                proof_data,
                total_chains as i64,
                bond_bytes,
            ],
        )?;
        Ok(())
    }

    /// Record a UTXO's association with a CAA escrow.
    pub fn insert_caa_utxo(&self, caa_hash: &[u8; 32], seq_id: u64, role: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO caa_utxos (caa_hash, seq_id, role) VALUES (?1, ?2, ?3)",
            params![&caa_hash[..], seq_id as i64, role],
        )?;
        Ok(())
    }

    /// Get CAA escrow status by hash.
    pub fn get_caa_escrow(&self, caa_hash: &[u8; 32]) -> Result<Option<CaaEscrow>> {
        let mut stmt = self.conn.prepare(
            "SELECT caa_hash, chain_order, deadline, status, block_height, proof_data, total_chains, bond_amount
             FROM caa_escrows WHERE caa_hash = ?1"
        )?;
        let mut rows = stmt.query(params![&caa_hash[..]])?;
        match rows.next()? {
            Some(row) => {
                let hash_bytes: Vec<u8> = row.get(0)?;
                let mut hash = [0u8; 32];
                if hash_bytes.len() == 32 { hash.copy_from_slice(&hash_bytes); }
                let bond_bytes: Vec<u8> = row.get(7)?;
                let bond_amount = if bond_bytes.is_empty() { BigInt::zero() } else { BigInt::from_signed_bytes_be(&bond_bytes) };
                Ok(Some(CaaEscrow {
                    caa_hash: hash,
                    chain_order: row.get::<_, i64>(1)? as u64,
                    deadline: row.get(2)?,
                    status: row.get(3)?,
                    block_height: row.get::<_, i64>(4)? as u64,
                    proof_data: row.get(5)?,
                    total_chains: row.get::<_, i64>(6)? as u64,
                    bond_amount,
                }))
            }
            None => Ok(None),
        }
    }

    /// Update CAA escrow status.
    pub fn update_caa_status(&self, caa_hash: &[u8; 32], new_status: &str) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE caa_escrows SET status = ?2 WHERE caa_hash = ?1",
            params![&caa_hash[..], new_status],
        )?;
        if updated == 0 {
            return Err(ChainError::CaaNotFound);
        }
        Ok(())
    }

    /// Store recording proof data for a CAA.
    pub fn set_caa_proof(&self, caa_hash: &[u8; 32], proof_data: &[u8]) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE caa_escrows SET proof_data = ?2 WHERE caa_hash = ?1",
            params![&caa_hash[..], proof_data],
        )?;
        if updated == 0 {
            return Err(ChainError::CaaNotFound);
        }
        Ok(())
    }

    /// Find all expired escrows (deadline < current_timestamp, status = 'escrowed').
    pub fn find_expired_escrows(&self, current_timestamp: i64) -> Result<Vec<CaaEscrow>> {
        let mut stmt = self.conn.prepare(
            "SELECT caa_hash, chain_order, deadline, status, block_height, proof_data, total_chains, bond_amount
             FROM caa_escrows WHERE status = 'escrowed' AND deadline < ?1"
        )?;
        let mut escrows = Vec::new();
        let mut rows = stmt.query(params![current_timestamp])?;
        while let Some(row) = rows.next()? {
            let hash_bytes: Vec<u8> = row.get(0)?;
            let mut hash = [0u8; 32];
            if hash_bytes.len() == 32 { hash.copy_from_slice(&hash_bytes); }
            let bond_bytes: Vec<u8> = row.get(7)?;
            let bond_amount = if bond_bytes.is_empty() { BigInt::zero() } else { BigInt::from_signed_bytes_be(&bond_bytes) };
            escrows.push(CaaEscrow {
                caa_hash: hash,
                chain_order: row.get::<_, i64>(1)? as u64,
                deadline: row.get(2)?,
                status: row.get(3)?,
                block_height: row.get::<_, i64>(4)? as u64,
                proof_data: row.get(5)?,
                total_chains: row.get::<_, i64>(6)? as u64,
                bond_amount,
            });
        }
        Ok(escrows)
    }

    /// Delete a UTXO by sequence ID (for CAA timeout cleanup of phantom receiver UTXOs).
    pub fn delete_utxo(&self, seq_id: u64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM utxos WHERE seq_id = ?1",
            params![seq_id as i64],
        )?;
        Ok(())
    }

    /// Remove a public key from the used_keys table (for CAA timeout cleanup).
    pub fn remove_key_used(&self, pubkey: &[u8; 32]) -> Result<()> {
        self.conn.execute(
            "DELETE FROM used_keys WHERE pubkey = ?1",
            params![&pubkey[..]],
        )?;
        Ok(())
    }

    /// Get all UTXO seq_ids for a given CAA escrow, filtered by role.
    pub fn get_caa_utxo_ids(&self, caa_hash: &[u8; 32], role: &str) -> Result<Vec<u64>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq_id FROM caa_utxos WHERE caa_hash = ?1 AND role = ?2"
        )?;
        let mut ids = Vec::new();
        let mut rows = stmt.query(params![&caa_hash[..], role])?;
        while let Some(row) = rows.next()? {
            let sid: i64 = row.get(0)?;
            ids.push(sid as u64);
        }
        Ok(ids)
    }

    // --- Escrow release cooldown tracking ---

    /// Record that a UTXO was released from escrow at the given timestamp.
    pub fn record_escrow_release(&self, seq_id: u64, released_at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO escrow_releases (seq_id, released_at) VALUES (?1, ?2)",
            params![seq_id as i64, released_at],
        )?;
        Ok(())
    }

    /// Record that a giver pubkey entered escrow for a specific CAA.
    pub fn record_giver_escrow(&self, pubkey: &[u8; 32], caa_hash: &[u8; 32], escrowed_at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO caa_giver_history (pubkey, caa_hash, escrowed_at) VALUES (?1, ?2, ?3)",
            params![&pubkey[..], &caa_hash[..], escrowed_at],
        )?;
        Ok(())
    }

    /// Count active (non-expired, non-finalized) escrows for a giver pubkey.
    pub fn count_active_escrows_for_giver(&self, pubkey: &[u8; 32]) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM caa_giver_history g
             INNER JOIN caa_escrows e ON g.caa_hash = e.caa_hash
             WHERE g.pubkey = ?1 AND e.status = 'escrowed'",
            params![&pubkey[..]],
            |row| row.get(0),
        )?;
        Ok(count as u64)
    }

    /// Get the timestamp when a UTXO was last released from escrow, if any.
    pub fn get_escrow_release_time(&self, seq_id: u64) -> Result<Option<i64>> {
        let mut stmt = self.conn.prepare(
            "SELECT released_at FROM escrow_releases WHERE seq_id = ?1"
        )?;
        let mut rows = stmt.query(params![seq_id as i64])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    // --- Known recorder keys (for CAA proof verification) ---

    /// Add a known recorder public key for a chain.
    pub fn add_recorder_key(&self, chain_id: &[u8; 32], pubkey: &[u8; 32], added_at: i64) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO known_recorder_keys (chain_id, pubkey, added_at, revoked_at)
             VALUES (?1, ?2, ?3, NULL)",
            params![&chain_id[..], &pubkey[..], added_at],
        )?;
        Ok(())
    }

    /// Revoke a recorder key (soft delete — keeps history).
    pub fn revoke_recorder_key(&self, chain_id: &[u8; 32], pubkey: &[u8; 32], revoked_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE known_recorder_keys SET revoked_at = ?3 WHERE chain_id = ?1 AND pubkey = ?2",
            params![&chain_id[..], &pubkey[..], revoked_at],
        )?;
        Ok(())
    }

    /// Get the active (non-revoked) recorder pubkey for a chain. Returns the most recently added.
    pub fn get_active_recorder_key(&self, chain_id: &[u8; 32]) -> Result<Option<[u8; 32]>> {
        let mut stmt = self.conn.prepare(
            "SELECT pubkey FROM known_recorder_keys
             WHERE chain_id = ?1 AND revoked_at IS NULL
             ORDER BY added_at DESC LIMIT 1"
        )?;
        let mut rows = stmt.query(params![&chain_id[..]])?;
        match rows.next()? {
            Some(row) => {
                let pk: Vec<u8> = row.get(0)?;
                if pk.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&pk);
                    Ok(Some(arr))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Load all active recorder keys as a HashMap (chain_id → pubkey).
    pub fn load_all_recorder_keys(&self) -> Result<std::collections::HashMap<[u8; 32], [u8; 32]>> {
        let mut stmt = self.conn.prepare(
            "SELECT chain_id, pubkey FROM known_recorder_keys
             WHERE revoked_at IS NULL
             ORDER BY added_at DESC"
        )?;
        let mut map = std::collections::HashMap::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let cid: Vec<u8> = row.get(0)?;
            let pk: Vec<u8> = row.get(1)?;
            if cid.len() == 32 && pk.len() == 32 {
                let mut chain_id = [0u8; 32];
                chain_id.copy_from_slice(&cid);
                let mut pubkey = [0u8; 32];
                pubkey.copy_from_slice(&pk);
                // First (most recent) key wins per chain
                map.entry(chain_id).or_insert(pubkey);
            }
        }
        Ok(map)
    }

    // --- Block storage ---

    /// Store a block.
    pub fn store_block(&self, height: u64, timestamp: i64, hash: &[u8; 32], data: &[u8]) -> Result<()> {
        self.conn.execute(
            "INSERT INTO blocks (height, timestamp, hash, data) VALUES (?1, ?2, ?3, ?4)",
            params![height as i64, timestamp, &hash[..], data],
        )?;
        Ok(())
    }

    /// Get a block by height.
    pub fn get_block(&self, height: u64) -> Result<Option<Vec<u8>>> {
        let mut stmt = self.conn.prepare("SELECT data FROM blocks WHERE height = ?1")?;
        let mut rows = stmt.query(params![height as i64])?;
        match rows.next()? {
            Some(row) => Ok(Some(row.get(0)?)),
            None => Ok(None),
        }
    }

    /// Get a block hash by height (without loading block data).
    pub fn get_block_hash(&self, height: u64) -> Result<Option<[u8; 32]>> {
        let mut stmt = self.conn.prepare("SELECT hash FROM blocks WHERE height = ?1")?;
        let mut rows = stmt.query(params![height as i64])?;
        match rows.next()? {
            Some(row) => {
                let hash_bytes: Vec<u8> = row.get(0)?;
                if hash_bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&hash_bytes);
                    Ok(Some(arr))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    /// Get block hashes for a range of heights (inclusive).
    pub fn get_block_hashes(&self, from: u64, to: u64) -> Result<Vec<(u64, [u8; 32])>> {
        let mut stmt = self.conn.prepare(
            "SELECT height, hash FROM blocks WHERE height >= ?1 AND height <= ?2 ORDER BY height"
        )?;
        let mut rows = stmt.query(params![from as i64, to as i64])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            let h: i64 = row.get(0)?;
            let hash_bytes: Vec<u8> = row.get(1)?;
            if hash_bytes.len() == 32 {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&hash_bytes);
                result.push((h as u64, arr));
            }
        }
        Ok(result)
    }

    /// Tamper with a stored block by flipping a byte deep inside the data blob.
    /// Targets a byte in the middle of the block, corrupting BLOCK_SIGNED content
    /// while keeping VBC structure intact so the block can still be deserialized.
    /// The validator should detect the hash mismatch.
    /// For testing and simulation only. Returns true if a block was modified.
    /// Enable the `test-support` feature to use this from external crates.
    #[cfg(any(test, feature = "test-support"))]
    pub fn tamper_block(&self, height: u64) -> Result<bool> {
        if let Some(mut data) = self.get_block(height)? {
            // Flip a byte deep in the data (past VBC headers) to corrupt content
            // without breaking VBC structure. Target 2/3 into the block.
            let idx = data.len() * 2 / 3;
            if idx > 0 {
                data[idx] ^= 0x01;
            }
            self.conn.execute(
                "UPDATE blocks SET data = ?1 WHERE height = ?2",
                params![&data[..], height as i64],
            )?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Get current block height.
    pub fn block_count(&self) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM blocks", [], |row| row.get(0)
        )?;
        Ok(count as u64)
    }

    /// Count all UTXOs (any status).
    pub fn count_utxos(&self) -> Result<u64> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM utxos", [], |row| row.get(0)
        )?;
        Ok(count as u64)
    }

    /// Get the database file size in bytes. Returns 0 for in-memory databases.
    pub fn db_file_size(&self) -> Result<u64> {
        let path: String = self.conn.query_row(
            "PRAGMA database_list", [], |row| row.get::<_, String>(2)
        )?;
        if path.is_empty() {
            return Ok(0); // in-memory
        }
        match std::fs::metadata(&path) {
            Ok(m) => Ok(m.len()),
            Err(_) => Ok(0),
        }
    }

    /// Get the timestamp (Unix seconds) of the most recent block.
    pub fn last_block_timestamp(&self) -> Result<Option<i64>> {
        let result: rusqlite::Result<i64> = self.conn.query_row(
            "SELECT timestamp FROM blocks ORDER BY height DESC LIMIT 1",
            [],
            |row| row.get(0),
        );
        match result {
            Ok(ts) => Ok(Some(ts)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Begin a transaction.
    pub fn begin_transaction(&self) -> Result<()> {
        self.conn.execute_batch("BEGIN IMMEDIATE")?;
        Ok(())
    }

    /// Commit a transaction.
    pub fn commit(&self) -> Result<()> {
        self.conn.execute_batch("COMMIT")?;
        Ok(())
    }

    /// Rollback a transaction.
    pub fn rollback(&self) -> Result<()> {
        self.conn.execute_batch("ROLLBACK")?;
        Ok(())
    }

    // --- Vendor profile persistence ---

    /// Get the stored vendor profile. Returns None if not set.
    pub fn get_vendor_profile(&self) -> Result<Option<(Option<String>, Option<String>, Option<f64>, Option<f64>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT name, description, lat, lon FROM vendor_profile WHERE id = 1"
        )?;
        let mut rows = stmt.query([])?;
        match rows.next()? {
            Some(row) => Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))),
            None => Ok(None),
        }
    }

    /// Set or update the vendor profile.
    pub fn set_vendor_profile(
        &self,
        name: Option<&str>,
        description: Option<&str>,
        lat: Option<f64>,
        lon: Option<f64>,
    ) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn.execute(
            "INSERT OR REPLACE INTO vendor_profile (id, name, description, lat, lon, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)",
            params![name, description, lat, lon, now],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_and_load_meta() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let meta = ChainMeta {
            chain_id: [0xAA; 32],
            symbol: "BCG".to_string(),
            coin_count: BigInt::from(10_000_000_000u64),
            shares_out: BigInt::from(1u64) << 86,
            fee_rate_num: BigInt::from(1),
            fee_rate_den: BigInt::from(1_000_000),
            expiry_period: 5_964_386_400_000_000i64,
            expiry_mode: 1,
            tax_start_age: None,
            tax_doubling_period: None,
            block_height: 0,
            next_seq_id: 1,
            last_block_timestamp: 0,
            prev_hash: [0; 32],
        };
        store.store_chain_meta(&meta).unwrap();

        let loaded = store.load_chain_meta().unwrap().unwrap();
        assert_eq!(loaded.chain_id, meta.chain_id);
        assert_eq!(loaded.symbol, "BCG");
        assert_eq!(loaded.shares_out, meta.shares_out);
        assert_eq!(loaded.fee_rate_num, BigInt::from(1));
        assert_eq!(loaded.fee_rate_den, BigInt::from(1_000_000));
        assert_eq!(loaded.expiry_period, meta.expiry_period);
        assert_eq!(loaded.expiry_mode, 1);
    }

    #[test]
    fn test_utxo_lifecycle() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let utxo = Utxo {
            seq_id: 1,
            pubkey: [0xBB; 32],
            amount: BigInt::from(1000),
            block_height: 0,
            block_timestamp: 100,
            status: UtxoStatus::Unspent,
        };
        store.insert_utxo(&utxo).unwrap();

        let loaded = store.get_utxo(1).unwrap().unwrap();
        assert_eq!(loaded.seq_id, 1);
        assert_eq!(loaded.amount, BigInt::from(1000));
        assert_eq!(loaded.status, UtxoStatus::Unspent);

        store.mark_spent(1).unwrap();
        let loaded = store.get_utxo(1).unwrap().unwrap();
        assert_eq!(loaded.status, UtxoStatus::Spent);

        // Can't spend again
        assert!(store.mark_spent(1).is_err());
    }

    #[test]
    fn test_key_reuse_tracking() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let key = [0xCC; 32];
        assert!(!store.is_key_used(&key).unwrap());
        store.mark_key_used(&key).unwrap();
        assert!(store.is_key_used(&key).unwrap());
    }

    #[test]
    fn test_refutation() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let hash = [0xDD; 32];
        assert!(!store.is_refuted(&hash).unwrap());
        store.add_refutation(&hash).unwrap();
        assert!(store.is_refuted(&hash).unwrap());
    }

    #[test]
    fn test_block_hash_retrieval() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let hash1 = [0x11; 32];
        let hash2 = [0x22; 32];
        store.store_block(0, 100, &hash1, b"block0").unwrap();
        store.store_block(1, 200, &hash2, b"block1").unwrap();

        // Single hash lookup
        assert_eq!(store.get_block_hash(0).unwrap(), Some(hash1));
        assert_eq!(store.get_block_hash(1).unwrap(), Some(hash2));
        assert_eq!(store.get_block_hash(2).unwrap(), None);

        // Range lookup
        let hashes = store.get_block_hashes(0, 1).unwrap();
        assert_eq!(hashes.len(), 2);
        assert_eq!(hashes[0], (0, hash1));
        assert_eq!(hashes[1], (1, hash2));

        // Empty range
        let empty = store.get_block_hashes(5, 10).unwrap();
        assert!(empty.is_empty());
    }

    /// B7 regression: malformed pubkey in DB returns an error instead of silently
    /// defaulting to [0u8; 32].
    #[test]
    fn test_malformed_pubkey_returns_error() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        // Insert a UTXO with a 16-byte pubkey (malformed) directly via SQL
        let bad_pk: Vec<u8> = vec![0xAA; 16];
        let amount_bytes: Vec<u8> = BigInt::from(100).to_signed_bytes_be();
        store.conn.execute(
            "INSERT INTO utxos (seq_id, pubkey, amount, block_height, block_timestamp, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![1i64, &bad_pk[..], &amount_bytes[..], 0i64, 100i64, "unspent"],
        ).unwrap();

        // get_utxo should return an error
        let result = store.get_utxo(1);
        assert!(result.is_err(), "Malformed pubkey must return an error");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("pubkey blob"), "Expected pubkey error, got: {}", err_msg);

        // find_expired_utxos should also return an error
        let result2 = store.find_expired_utxos(300, 150);
        assert!(result2.is_err(), "Malformed pubkey in find_expired_utxos must return an error");
    }

    /// B7 regression: unknown status string in DB returns an error instead of
    /// defaulting to Unspent.
    #[test]
    fn test_unknown_status_returns_error() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let pk: Vec<u8> = vec![0xBB; 32];
        let amount_bytes: Vec<u8> = BigInt::from(100).to_signed_bytes_be();
        store.conn.execute(
            "INSERT INTO utxos (seq_id, pubkey, amount, block_height, block_timestamp, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![1i64, &pk[..], &amount_bytes[..], 0i64, 100i64, "bogus_status"],
        ).unwrap();

        // get_utxo should return an error for unknown status
        let result = store.get_utxo(1);
        assert!(result.is_err(), "Unknown status must return an error");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("unknown status"), "Expected status error, got: {}", err_msg);

    }

    #[test]
    fn test_find_expired_utxos() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        // UTXO created at timestamp 100
        store.insert_utxo(&Utxo {
            seq_id: 1, pubkey: [0x01; 32], amount: BigInt::from(500),
            block_height: 0, block_timestamp: 100, status: UtxoStatus::Unspent,
        }).unwrap();

        // UTXO created at timestamp 200
        store.insert_utxo(&Utxo {
            seq_id: 2, pubkey: [0x02; 32], amount: BigInt::from(300),
            block_height: 0, block_timestamp: 200, status: UtxoStatus::Unspent,
        }).unwrap();

        // Expiry period = 150, current time = 300
        // Cutoff = 300 - 150 = 150. UTXOs with block_timestamp < 150 are expired.
        let expired = store.find_expired_utxos(300, 150).unwrap();
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].seq_id, 1);
    }

    #[test]
    fn test_vendor_profile_roundtrip() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        // No profile initially
        assert!(store.get_vendor_profile().unwrap().is_none());

        // Set profile
        store.set_vendor_profile(
            Some("Bob's Curry Goat"),
            Some("Best curry in town"),
            Some(18.1096),
            Some(-77.2975),
        ).unwrap();

        let (name, desc, lat, lon) = store.get_vendor_profile().unwrap().unwrap();
        assert_eq!(name.as_deref(), Some("Bob's Curry Goat"));
        assert_eq!(desc.as_deref(), Some("Best curry in town"));
        assert!((lat.unwrap() - 18.1096).abs() < 1e-6);
        assert!((lon.unwrap() - (-77.2975)).abs() < 1e-6);

        // Overwrite
        store.set_vendor_profile(Some("Updated Name"), None, None, None).unwrap();
        let (name, desc, lat, lon) = store.get_vendor_profile().unwrap().unwrap();
        assert_eq!(name.as_deref(), Some("Updated Name"));
        assert!(desc.is_none());
        assert!(lat.is_none());
        assert!(lon.is_none());
    }

    #[test]
    fn test_init_schema_idempotent() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        // Second call should succeed (CREATE TABLE IF NOT EXISTS)
        store.init_schema().unwrap();
        // Profile operations still work
        store.set_vendor_profile(Some("Test"), None, None, None).unwrap();
        assert!(store.get_vendor_profile().unwrap().is_some());
    }
}
