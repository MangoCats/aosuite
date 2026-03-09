// Trade history SQLite persistence for the exchange daemon.

use anyhow::{Context, Result};
use rusqlite::{Connection, params};
use serde::Serialize;

/// Persistent trade history store.
pub struct TradeStore {
    conn: Connection,
}

/// A completed or failed trade record.
#[derive(Debug, Clone, Serialize)]
pub struct TradeRecord {
    pub trade_id: String,
    pub buy_symbol: String,
    pub sell_symbol: String,
    pub buy_chain_id: String,
    pub sell_chain_id: String,
    pub buy_amount: String,
    pub sell_amount: String,
    pub rate: f64,
    pub spread: f64,
    pub status: String,       // "completed", "failed", "expired"
    pub requested_at: i64,    // Unix seconds
    pub completed_at: i64,    // Unix seconds (0 if not completed)
    pub error_message: Option<String>,
}

/// Query parameters for trade history.
pub struct TradeQuery {
    pub from_secs: Option<i64>,
    pub to_secs: Option<i64>,
    pub symbol: Option<String>,
    pub status: Option<String>,
    pub limit: usize,
    pub offset: usize,
}

/// Aggregated P&L summary for a symbol pair.
#[derive(Debug, Clone, Serialize)]
pub struct PairPnl {
    pub pair: String,
    pub trade_count: u64,
    pub total_buy: String,
    pub total_sell: String,
}

impl TradeStore {
    /// Open or create the trade history database.
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)
            .context("failed to open trade history database")?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
            .context("failed to set database pragmas")?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS trade_history (
                trade_id TEXT PRIMARY KEY,
                buy_symbol TEXT NOT NULL,
                sell_symbol TEXT NOT NULL,
                buy_chain_id TEXT NOT NULL,
                sell_chain_id TEXT NOT NULL,
                buy_amount TEXT NOT NULL,
                sell_amount TEXT NOT NULL,
                rate REAL NOT NULL,
                spread REAL NOT NULL,
                status TEXT NOT NULL,
                requested_at INTEGER NOT NULL,
                completed_at INTEGER NOT NULL DEFAULT 0,
                error_message TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_trade_completed_at ON trade_history(completed_at);
            CREATE INDEX IF NOT EXISTS idx_trade_status ON trade_history(status);",
        ).context("failed to initialize trade_history schema")?;

        Ok(TradeStore { conn })
    }

    /// Open an in-memory database (for tests).
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        Self::open(":memory:")
    }

    /// Record a completed trade.
    pub fn insert_trade(&self, record: &TradeRecord) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO trade_history
             (trade_id, buy_symbol, sell_symbol, buy_chain_id, sell_chain_id,
              buy_amount, sell_amount, rate, spread, status,
              requested_at, completed_at, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                record.trade_id, record.buy_symbol, record.sell_symbol,
                record.buy_chain_id, record.sell_chain_id,
                record.buy_amount, record.sell_amount,
                record.rate, record.spread, record.status,
                record.requested_at, record.completed_at, record.error_message,
            ],
        )?;
        Ok(())
    }

    /// Query trade history with optional filters.
    pub fn query_trades(&self, q: &TradeQuery) -> Result<Vec<TradeRecord>> {
        let mut sql = String::from(
            "SELECT trade_id, buy_symbol, sell_symbol, buy_chain_id, sell_chain_id,
                    buy_amount, sell_amount, rate, spread, status,
                    requested_at, completed_at, error_message
             FROM trade_history WHERE 1=1"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(from) = q.from_secs {
            sql.push_str(&format!(" AND completed_at >= ?{}", param_values.len() + 1));
            param_values.push(Box::new(from));
        }
        if let Some(to) = q.to_secs {
            sql.push_str(&format!(" AND completed_at <= ?{}", param_values.len() + 1));
            param_values.push(Box::new(to));
        }
        if let Some(ref sym) = q.symbol {
            let idx = param_values.len() + 1;
            sql.push_str(&format!(" AND (buy_symbol = ?{} OR sell_symbol = ?{})", idx, idx));
            param_values.push(Box::new(sym.clone()));
        }
        if let Some(ref status) = q.status {
            sql.push_str(&format!(" AND status = ?{}", param_values.len() + 1));
            param_values.push(Box::new(status.clone()));
        }

        sql.push_str(" ORDER BY completed_at DESC");
        sql.push_str(&format!(" LIMIT {} OFFSET {}", q.limit.min(1000), q.offset));

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok(TradeRecord {
                trade_id: row.get(0)?,
                buy_symbol: row.get(1)?,
                sell_symbol: row.get(2)?,
                buy_chain_id: row.get(3)?,
                sell_chain_id: row.get(4)?,
                buy_amount: row.get(5)?,
                sell_amount: row.get(6)?,
                rate: row.get(7)?,
                spread: row.get(8)?,
                status: row.get(9)?,
                requested_at: row.get(10)?,
                completed_at: row.get(11)?,
                error_message: row.get(12)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    /// Count total trades matching query (for pagination).
    pub fn count_trades(&self, q: &TradeQuery) -> Result<u64> {
        let mut sql = String::from("SELECT COUNT(*) FROM trade_history WHERE 1=1");
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(from) = q.from_secs {
            sql.push_str(&format!(" AND completed_at >= ?{}", param_values.len() + 1));
            param_values.push(Box::new(from));
        }
        if let Some(to) = q.to_secs {
            sql.push_str(&format!(" AND completed_at <= ?{}", param_values.len() + 1));
            param_values.push(Box::new(to));
        }
        if let Some(ref sym) = q.symbol {
            let idx = param_values.len() + 1;
            sql.push_str(&format!(" AND (buy_symbol = ?{} OR sell_symbol = ?{})", idx, idx));
            param_values.push(Box::new(sym.clone()));
        }
        if let Some(ref status) = q.status {
            sql.push_str(&format!(" AND status = ?{}", param_values.len() + 1));
            param_values.push(Box::new(status.clone()));
        }

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let count: u64 = self.conn.query_row(&sql, params_refs.as_slice(), |row| row.get(0))?;
        Ok(count)
    }

    /// Aggregate trade volume by symbol pair.
    ///
    /// Sums buy/sell amounts in Rust using BigInt to avoid i64 overflow
    /// that would occur with SQL CAST(AS INTEGER) on large amounts.
    pub fn pair_pnl(&self, from_secs: Option<i64>, to_secs: Option<i64>) -> Result<Vec<PairPnl>> {
        let mut sql = String::from(
            "SELECT buy_symbol || '/' || sell_symbol AS pair,
                    buy_amount, sell_amount
             FROM trade_history WHERE status = 'completed'"
        );
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(from) = from_secs {
            sql.push_str(&format!(" AND completed_at >= ?{}", param_values.len() + 1));
            param_values.push(Box::new(from));
        }
        if let Some(to) = to_secs {
            sql.push_str(&format!(" AND completed_at <= ?{}", param_values.len() + 1));
            param_values.push(Box::new(to));
        }

        sql.push_str(" ORDER BY pair");

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        use std::collections::BTreeMap;
        use num_bigint::BigInt;

        let mut agg: BTreeMap<String, (u64, BigInt, BigInt)> = BTreeMap::new();
        for row in rows {
            let (pair, buy_str, sell_str) = row?;
            let buy: BigInt = buy_str.parse().unwrap_or_default();
            let sell: BigInt = sell_str.parse().unwrap_or_default();
            let entry = agg.entry(pair).or_insert((0, BigInt::ZERO, BigInt::ZERO));
            entry.0 += 1;
            entry.1 += buy;
            entry.2 += sell;
        }

        Ok(agg.into_iter().map(|(pair, (count, total_buy, total_sell))| {
            PairPnl {
                pair,
                trade_count: count,
                total_buy: total_buy.to_string(),
                total_sell: total_sell.to_string(),
            }
        }).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(id: &str, status: &str) -> TradeRecord {
        TradeRecord {
            trade_id: id.to_string(),
            buy_symbol: "BCG".to_string(),
            sell_symbol: "RMF".to_string(),
            buy_chain_id: "chain_bcg".to_string(),
            sell_chain_id: "chain_rmf".to_string(),
            buy_amount: "1000".to_string(),
            sell_amount: "495".to_string(),
            rate: 2.0,
            spread: 0.02,
            status: status.to_string(),
            requested_at: 1000,
            completed_at: 1010,
            error_message: None,
        }
    }

    #[test]
    fn test_insert_and_query() {
        let store = TradeStore::open_memory().unwrap();
        store.insert_trade(&sample_record("t1", "completed")).unwrap();
        store.insert_trade(&sample_record("t2", "completed")).unwrap();

        let trades = store.query_trades(&TradeQuery {
            from_secs: None, to_secs: None, symbol: None,
            status: None, limit: 100, offset: 0,
        }).unwrap();
        assert_eq!(trades.len(), 2);
    }

    #[test]
    fn test_query_by_status() {
        let store = TradeStore::open_memory().unwrap();
        store.insert_trade(&sample_record("t1", "completed")).unwrap();
        store.insert_trade(&sample_record("t2", "failed")).unwrap();

        let trades = store.query_trades(&TradeQuery {
            from_secs: None, to_secs: None, symbol: None,
            status: Some("completed".to_string()), limit: 100, offset: 0,
        }).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].trade_id, "t1");
    }

    #[test]
    fn test_query_by_time_range() {
        let store = TradeStore::open_memory().unwrap();
        let mut r1 = sample_record("t1", "completed");
        r1.completed_at = 100;
        let mut r2 = sample_record("t2", "completed");
        r2.completed_at = 200;
        store.insert_trade(&r1).unwrap();
        store.insert_trade(&r2).unwrap();

        let trades = store.query_trades(&TradeQuery {
            from_secs: Some(150), to_secs: None, symbol: None,
            status: None, limit: 100, offset: 0,
        }).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].trade_id, "t2");
    }

    #[test]
    fn test_query_by_symbol() {
        let store = TradeStore::open_memory().unwrap();
        store.insert_trade(&sample_record("t1", "completed")).unwrap();
        let mut r2 = sample_record("t2", "completed");
        r2.buy_symbol = "XYZ".to_string();
        r2.sell_symbol = "ABC".to_string();
        store.insert_trade(&r2).unwrap();

        let trades = store.query_trades(&TradeQuery {
            from_secs: None, to_secs: None, symbol: Some("BCG".to_string()),
            status: None, limit: 100, offset: 0,
        }).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].trade_id, "t1");
    }

    #[test]
    fn test_count_trades() {
        let store = TradeStore::open_memory().unwrap();
        store.insert_trade(&sample_record("t1", "completed")).unwrap();
        store.insert_trade(&sample_record("t2", "failed")).unwrap();
        store.insert_trade(&sample_record("t3", "completed")).unwrap();

        let count = store.count_trades(&TradeQuery {
            from_secs: None, to_secs: None, symbol: None,
            status: Some("completed".to_string()), limit: 100, offset: 0,
        }).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_pair_pnl() {
        let store = TradeStore::open_memory().unwrap();
        store.insert_trade(&sample_record("t1", "completed")).unwrap();
        store.insert_trade(&sample_record("t2", "completed")).unwrap();

        let pnl = store.pair_pnl(None, None).unwrap();
        assert_eq!(pnl.len(), 1);
        assert_eq!(pnl[0].pair, "BCG/RMF");
        assert_eq!(pnl[0].trade_count, 2);
        assert_eq!(pnl[0].total_buy, "2000");
        assert_eq!(pnl[0].total_sell, "990");
    }

    #[test]
    fn test_upsert_on_conflict() {
        let store = TradeStore::open_memory().unwrap();
        store.insert_trade(&sample_record("t1", "completed")).unwrap();
        let mut updated = sample_record("t1", "failed");
        updated.error_message = Some("test error".to_string());
        store.insert_trade(&updated).unwrap();

        let trades = store.query_trades(&TradeQuery {
            from_secs: None, to_secs: None, symbol: None,
            status: None, limit: 100, offset: 0,
        }).unwrap();
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].status, "failed");
    }
}
