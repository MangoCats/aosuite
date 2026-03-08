use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use num_bigint::BigInt;

/// A pending trade awaiting consumer deposit.
pub struct PendingTrade {
    pub trade_id: String,
    pub pair_index: usize,
    /// Buy chain (consumer deposits here).
    pub buy_chain_id: String,
    pub buy_symbol: String,
    /// Sell chain (agent pays out here).
    pub sell_chain_id: String,
    pub sell_symbol: String,
    /// Expected deposit amount on buy chain.
    pub expected_amount: BigInt,
    /// Agent's deposit key on buy chain — consumer sends shares here.
    pub deposit_pubkey: [u8; 32],
    pub deposit_seed: [u8; 32],
    /// Consumer's receive key on sell chain — agent sends shares here.
    pub receive_pubkey: [u8; 32],
    pub receive_seed: [u8; 32],
    /// Estimated amount consumer will receive on sell chain.
    pub estimated_receive_amount: BigInt,
    /// Unix timestamp when this trade request expires.
    pub expires_at: u64,
}

/// Manages pending trades indexed by deposit pubkey for fast lookup.
pub struct TradeManager {
    /// trade_id → PendingTrade
    trades: HashMap<String, PendingTrade>,
    /// deposit_pubkey (hex) → trade_id for fast UTXO matching
    deposit_index: HashMap<String, String>,
    /// Default trade expiry in seconds (5 minutes).
    pub trade_ttl_secs: u64,
}

impl Default for TradeManager {
    fn default() -> Self { Self::new() }
}

impl TradeManager {
    pub fn new() -> Self {
        TradeManager {
            trades: HashMap::new(),
            deposit_index: HashMap::new(),
            trade_ttl_secs: 300,
        }
    }

    /// Insert a new pending trade.
    pub fn insert(&mut self, trade: PendingTrade) {
        let deposit_hex = hex::encode(trade.deposit_pubkey);
        self.deposit_index.insert(deposit_hex, trade.trade_id.clone());
        self.trades.insert(trade.trade_id.clone(), trade);
    }

    /// Look up a pending trade by deposit pubkey (hex).
    pub fn find_by_deposit(&self, deposit_pubkey_hex: &str) -> Option<&PendingTrade> {
        self.deposit_index.get(deposit_pubkey_hex)
            .and_then(|id| self.trades.get(id))
    }

    /// Remove a trade (after execution or expiry).
    pub fn remove(&mut self, trade_id: &str) -> Option<PendingTrade> {
        if let Some(trade) = self.trades.remove(trade_id) {
            let deposit_hex = hex::encode(trade.deposit_pubkey);
            self.deposit_index.remove(&deposit_hex);
            Some(trade)
        } else {
            None
        }
    }

    /// Remove all expired trades.
    pub fn expire_stale(&mut self) -> usize {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let expired: Vec<String> = self.trades.iter()
            .filter(|(_, t)| t.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();

        let count = expired.len();
        for id in expired {
            self.remove(&id);
        }
        count
    }

    pub fn pending_count(&self) -> usize {
        self.trades.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trade(trade_id: &str, deposit_pub: [u8; 32], expires_at: u64) -> PendingTrade {
        PendingTrade {
            trade_id: trade_id.to_string(),
            pair_index: 0,
            buy_chain_id: "buy_chain".to_string(),
            buy_symbol: "BUY".to_string(),
            sell_chain_id: "sell_chain".to_string(),
            sell_symbol: "SELL".to_string(),
            expected_amount: BigInt::from(1000),
            deposit_pubkey: deposit_pub,
            deposit_seed: [0x01; 32],
            receive_pubkey: [0x02; 32],
            receive_seed: [0x03; 32],
            estimated_receive_amount: BigInt::from(950),
            expires_at,
        }
    }

    #[test]
    fn test_insert_and_find() {
        let mut mgr = TradeManager::new();
        let pub_key = [0xAA; 32];
        mgr.insert(make_trade("t1", pub_key, u64::MAX));

        let found = mgr.find_by_deposit(&hex::encode(pub_key));
        assert!(found.is_some());
        assert_eq!(found.unwrap().trade_id, "t1");
        assert_eq!(mgr.pending_count(), 1);
    }

    #[test]
    fn test_remove() {
        let mut mgr = TradeManager::new();
        let pub_key = [0xBB; 32];
        mgr.insert(make_trade("t2", pub_key, u64::MAX));

        let removed = mgr.remove("t2");
        assert!(removed.is_some());
        assert!(mgr.find_by_deposit(&hex::encode(pub_key)).is_none());
        assert_eq!(mgr.pending_count(), 0);
    }

    #[test]
    fn test_expire_stale() {
        let mut mgr = TradeManager::new();
        mgr.insert(make_trade("old", [0xCC; 32], 0)); // already expired
        mgr.insert(make_trade("new", [0xDD; 32], u64::MAX)); // not expired

        let expired = mgr.expire_stale();
        assert_eq!(expired, 1);
        assert_eq!(mgr.pending_count(), 1);
        assert!(mgr.find_by_deposit(&hex::encode([0xDD; 32])).is_some());
    }
}
