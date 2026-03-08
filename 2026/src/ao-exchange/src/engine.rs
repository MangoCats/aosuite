use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, bail};
use num_bigint::BigInt;
use tracing::info;

use crate::client::RecorderClient;
use crate::config::Config;
use crate::trade::{PendingTrade, TradeManager};
use crate::transfer::{self, Giver, Receiver};
use crate::wallet::Wallet;

/// Runtime state for one chain the exchange agent participates in.
pub struct ChainState {
    pub chain_id: String,
    pub symbol: String,
    pub client: RecorderClient,
}

/// A resolved trading pair with chain IDs.
pub struct ResolvedPair {
    pub sell_chain_id: String,
    pub sell_symbol: String,
    pub buy_chain_id: String,
    pub buy_symbol: String,
    pub rate: f64,
    pub spread: f64,
    pub min_trade: Option<u64>,
    pub max_trade: Option<u64>,
}

/// Core exchange engine holding wallet, chain connections, and trading rules.
pub struct ExchangeEngine {
    pub wallet: Wallet,
    pub chains: HashMap<String, ChainState>, // chain_id → state
    pub symbol_to_chain: HashMap<String, String>, // symbol → chain_id
    pub pairs: Vec<ResolvedPair>,
    pub trades: TradeManager,
    /// Tracks last-seen next_seq_id per chain for deposit detection.
    pub last_seq: HashMap<String, u64>,
}

/// Convert a buy amount to sell amount using rate and spread.
/// Returns None if the result is non-positive (too small to trade).
fn compute_sell_amount(buy_amount: &BigInt, rate: f64, spread: f64) -> Option<BigInt> {
    let buy_f64 = buy_amount.to_string().parse::<f64>().unwrap_or(0.0);
    let effective_rate = rate * (1.0 + spread / 2.0);
    if effective_rate <= 0.0 {
        return None;
    }
    let sell_f64 = buy_f64 / effective_rate;
    // Guard: f64 values beyond i64 range would wrap silently
    if sell_f64 < 1.0 || sell_f64 > i64::MAX as f64 {
        return None;
    }
    Some(BigInt::from(sell_f64 as i64))
}

impl ExchangeEngine {
    /// Initialize from config: resolve chain IDs, import keys.
    pub async fn from_config(config: &Config) -> Result<Self> {
        let mut wallet = Wallet::new();
        let mut chains = HashMap::new();
        let mut symbol_to_chain = HashMap::new();

        for chain_cfg in &config.chains {
            let client = RecorderClient::new(&chain_cfg.recorder_url);

            // Resolve chain ID: use configured value or discover from recorder
            let chain_id = if let Some(id) = &chain_cfg.chain_id {
                id.clone()
            } else {
                let chain_list = client.list_chains().await?;
                let entry = chain_list.iter()
                    .find(|c| c.symbol == chain_cfg.symbol)
                    .ok_or_else(|| anyhow::anyhow!(
                        "chain {} not found on recorder {}", chain_cfg.symbol, chain_cfg.recorder_url
                    ))?;
                entry.chain_id.clone()
            };

            // Import signing key
            let seed_bytes = hex::decode(chain_cfg.key_seed.trim())
                .map_err(|e| anyhow::anyhow!("invalid key_seed hex for {}: {}", chain_cfg.symbol, e))?;
            let seed: [u8; 32] = seed_bytes.try_into()
                .map_err(|_| anyhow::anyhow!("key_seed for {} must be 32 bytes", chain_cfg.symbol))?;
            wallet.import_key(seed, &chain_id);

            info!(symbol = %chain_cfg.symbol, chain_id = %chain_id, "Connected to chain");

            symbol_to_chain.insert(chain_cfg.symbol.clone(), chain_id.clone());
            chains.insert(chain_id.clone(), ChainState {
                chain_id: chain_id.clone(),
                symbol: chain_cfg.symbol.clone(),
                client,
            });
        }

        // Resolve trading pairs
        let mut pairs = Vec::new();
        for pair_cfg in &config.pairs {
            let sell_cid = symbol_to_chain.get(&pair_cfg.sell)
                .ok_or_else(|| anyhow::anyhow!("sell chain {} not in [[chains]]", pair_cfg.sell))?;
            let buy_cid = symbol_to_chain.get(&pair_cfg.buy)
                .ok_or_else(|| anyhow::anyhow!("buy chain {} not in [[chains]]", pair_cfg.buy))?;

            pairs.push(ResolvedPair {
                sell_chain_id: sell_cid.clone(),
                sell_symbol: pair_cfg.sell.clone(),
                buy_chain_id: buy_cid.clone(),
                buy_symbol: pair_cfg.buy.clone(),
                rate: pair_cfg.rate,
                spread: pair_cfg.spread,
                min_trade: pair_cfg.min_trade,
                max_trade: pair_cfg.max_trade,
            });
        }

        let mut trades = TradeManager::new();
        trades.trade_ttl_secs = config.trade_ttl_secs;

        Ok(ExchangeEngine {
            wallet,
            chains,
            symbol_to_chain,
            pairs,
            trades,
            last_seq: HashMap::new(),
        })
    }

    /// Execute a cross-chain trade: receive payment on buy_chain, send on sell_chain.
    /// Returns (sell_block_height, sell_amount).
    pub async fn execute_trade(
        &mut self,
        pair_index: usize,
        pay_amount: &BigInt,
        consumer_recv_pubkey: [u8; 32],
        consumer_recv_seed: [u8; 32],
    ) -> Result<(u64, BigInt)> {
        let pair = &self.pairs[pair_index];

        // Calculate sell amount using f64 arithmetic. Exchange rates are inherently
        // approximate (configured as floats, subject to spread), and this is a
        // unilateral agent decision — not consensus-critical like on-chain fee math.
        let sell_amount = compute_sell_amount(pay_amount, pair.rate, pair.spread)
            .ok_or_else(|| anyhow::anyhow!("sell amount too small after rate conversion"))?;

        let sell_chain_id = pair.sell_chain_id.clone();
        let sell_symbol = pair.sell_symbol.clone();
        let buy_symbol = pair.buy_symbol.clone();

        let sell_client = &self.chains.get(&sell_chain_id)
            .ok_or_else(|| anyhow::anyhow!("sell chain {} not connected", sell_symbol))?
            .client;

        // Find our UTXO on sell chain
        let utxo = self.wallet.find_unspent(&sell_chain_id)
            .ok_or_else(|| anyhow::anyhow!("no unspent UTXO on sell chain {}", sell_symbol))?;

        let change_entry = self.wallet.generate_key(&sell_chain_id);

        let giver = Giver {
            seq_id: utxo.seq_id,
            amount: utxo.amount.clone(),
            seed: utxo.seed,
        };
        let giver_pubkey = utxo.pubkey;

        let mut receivers = vec![
            Receiver {
                pubkey: consumer_recv_pubkey,
                seed: consumer_recv_seed,
                amount: sell_amount.clone(),
            },
            Receiver {
                pubkey: change_entry.pubkey,
                seed: change_entry.seed,
                amount: BigInt::from(0), // adjusted by execute_transfer
            },
        ];

        // Attach EXCHANGE_LISTING: counterpart chain symbol, payment amount, agent label
        let listing = transfer::build_exchange_listing(
            &buy_symbol,
            pay_amount,
            "ao-exchange",
        );

        let result = transfer::execute_transfer(
            sell_client, &sell_chain_id, &[giver], &mut receivers, &[listing],
        ).await?;

        self.wallet.mark_spent(&giver_pubkey);
        let change_seq = result.first_seq + 1;
        self.wallet.register_utxo(&change_entry.pubkey, change_seq, receivers[1].amount.clone());

        info!(
            sell = %sell_symbol, amount = %receivers[0].amount,
            block = result.height, "Trade executed"
        );

        Ok((result.height, receivers[0].amount.clone()))
    }

    /// Find a trading pair index for a given (sell_symbol, buy_symbol).
    pub fn find_pair(&self, sell_symbol: &str, buy_symbol: &str) -> Option<usize> {
        self.pairs.iter().position(|p| p.sell_symbol == sell_symbol && p.buy_symbol == buy_symbol)
    }

    /// Get current positions as (symbol, balance) pairs.
    pub fn positions(&self) -> Vec<(String, BigInt)> {
        let mut result = Vec::new();
        for (chain_id, state) in &self.chains {
            let balance = self.wallet.balance(chain_id);
            result.push((state.symbol.clone(), balance));
        }
        result.sort_by(|a, b| a.0.cmp(&b.0));
        result
    }

    /// Create a pending trade request. Returns the trade details for the consumer.
    ///
    /// The consumer must:
    /// 1. Build an assignment on the buy chain with `deposit_pubkey` as a receiver
    ///    (using `deposit_seed` for the receiver signature).
    /// 2. Wait for the exchange agent to detect the deposit and execute the reverse leg.
    /// 3. The consumer receives shares at `receive_pubkey` on the sell chain
    ///    (they hold `receive_seed` to spend later).
    pub fn request_trade(
        &mut self,
        sell_symbol: &str,
        buy_symbol: &str,
        buy_amount: &BigInt,
    ) -> Result<&PendingTrade> {
        let pair_index = self.find_pair(sell_symbol, buy_symbol)
            .ok_or_else(|| anyhow::anyhow!("no trading pair {}/{}", sell_symbol, buy_symbol))?;
        let pair = &self.pairs[pair_index];

        // Validate trade size
        let amount_u64 = buy_amount.to_string().parse::<u64>().unwrap_or(0);
        if let Some(min) = pair.min_trade
            && amount_u64 < min
        {
            bail!("amount {} below minimum trade {}", buy_amount, min);
        }
        if let Some(max) = pair.max_trade
            && amount_u64 > max
        {
            bail!("amount {} above maximum trade {}", buy_amount, max);
        }

        // Check sell-chain inventory
        let sell_balance = self.wallet.balance(&pair.sell_chain_id);
        let estimated_sell = compute_sell_amount(buy_amount, pair.rate, pair.spread)
            .unwrap_or(BigInt::from(0));
        if estimated_sell > sell_balance {
            bail!(
                "insufficient {} inventory: need ~{}, have {}",
                pair.sell_symbol, estimated_sell, sell_balance
            );
        }

        // Generate deposit key (buy chain — consumer sends payment here)
        let deposit_key = self.wallet.generate_key(&pair.buy_chain_id);

        // Generate receive key (sell chain — agent sends payout here, consumer holds seed)
        let receive_key = self.wallet.generate_key(&pair.sell_chain_id);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_secs();

        let trade_id = uuid::Uuid::new_v4().to_string();
        let trade = PendingTrade {
            trade_id: trade_id.clone(),
            pair_index,
            buy_chain_id: pair.buy_chain_id.clone(),
            buy_symbol: pair.buy_symbol.clone(),
            sell_chain_id: pair.sell_chain_id.clone(),
            sell_symbol: pair.sell_symbol.clone(),
            expected_amount: buy_amount.clone(),
            deposit_pubkey: deposit_key.pubkey,
            deposit_seed: deposit_key.seed,
            receive_pubkey: receive_key.pubkey,
            receive_seed: receive_key.seed,
            estimated_receive_amount: estimated_sell,
            expires_at: now + self.trades.trade_ttl_secs,
        };

        self.trades.insert(trade);
        Ok(self.trades.find_by_deposit(&hex::encode(deposit_key.pubkey))
            .expect("just inserted"))
    }

    /// Poll chains for new UTXOs. When a deposit matches a pending trade,
    /// execute the reverse-leg trade automatically.
    ///
    /// Returns a list of (trade_id, result) for trades attempted this cycle.
    pub async fn check_deposits(&mut self) -> Vec<(String, Result<(u64, BigInt)>)> {
        // Expire stale trade requests
        let expired = self.trades.expire_stale();
        if expired > 0 {
            tracing::info!(expired, "Expired stale trade requests");
        }

        // Collect chain IDs to check (avoid borrow issues)
        let chain_ids: Vec<String> = self.chains.keys().cloned().collect();
        let mut matched_trades: Vec<(String, BigInt)> = Vec::new();

        for chain_id in &chain_ids {
            let chain_state = &self.chains[chain_id];
            let info = match chain_state.client.chain_info(chain_id).await {
                Ok(info) => info,
                Err(e) => {
                    tracing::warn!(
                        chain = %chain_state.symbol, "poll failed: {}", e
                    );
                    continue;
                }
            };

            let prev_seq = *self.last_seq.get(chain_id).unwrap_or(&info.next_seq_id);
            let current_seq = info.next_seq_id;
            self.last_seq.insert(chain_id.clone(), current_seq);

            // Check new UTXOs
            for seq_id in prev_seq..current_seq {
                let utxo = match chain_state.client.get_utxo(chain_id, seq_id).await {
                    Ok(u) => u,
                    Err(e) => {
                        tracing::debug!(seq_id, "utxo fetch failed: {}", e);
                        continue;
                    }
                };

                if utxo.status != "Unspent" {
                    continue;
                }

                // Check if this UTXO's pubkey matches a pending trade deposit
                if let Some(trade) = self.trades.find_by_deposit(&utxo.pubkey) {
                    let amount: BigInt = match utxo.amount.parse() {
                        Ok(a) => a,
                        Err(_) => continue,
                    };
                    info!(
                        trade_id = %trade.trade_id,
                        chain = %trade.buy_symbol,
                        amount = %amount,
                        "Deposit detected"
                    );
                    // Register the deposited UTXO in wallet so agent can spend it later
                    let deposit_pub = trade.deposit_pubkey;
                    self.wallet.register_utxo(&deposit_pub, seq_id, amount.clone());

                    matched_trades.push((trade.trade_id.clone(), amount));
                }
            }
        }

        // Execute matched trades
        let mut results = Vec::new();
        for (trade_id, pay_amount) in matched_trades {
            // Remove trade from pending (consume it)
            let trade = match self.trades.remove(&trade_id) {
                Some(t) => t,
                None => continue,
            };

            let result = self.execute_trade(
                trade.pair_index,
                &pay_amount,
                trade.receive_pubkey,
                trade.receive_seed,
            ).await;

            match &result {
                Ok((height, amount)) => {
                    info!(
                        trade_id = %trade_id,
                        sell = %trade.sell_symbol,
                        amount = %amount,
                        block = height,
                        "Auto-trade completed"
                    );
                }
                Err(e) => {
                    tracing::error!(
                        trade_id = %trade_id,
                        sell = %trade.sell_symbol,
                        "Auto-trade failed: {}", e
                    );
                }
            }

            results.push((trade_id, result));
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_sell_amount_normal() {
        // rate=2.0, spread=0.02: effective_rate = 2.0 * 1.01 = 2.02
        // sell = 1000 / 2.02 ≈ 495
        let result = compute_sell_amount(&BigInt::from(1000), 2.0, 0.02);
        assert!(result.is_some());
        let sell = result.unwrap();
        assert!(sell > BigInt::from(0));
        assert!(sell < BigInt::from(1000));
    }

    #[test]
    fn test_compute_sell_amount_too_small() {
        // 1 share at rate 1000 → sell would be ~0
        let result = compute_sell_amount(&BigInt::from(1), 1000.0, 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_compute_sell_amount_zero_rate() {
        let result = compute_sell_amount(&BigInt::from(1000), 0.0, 0.0);
        assert!(result.is_none());
    }

    #[test]
    fn test_compute_sell_amount_negative_rate() {
        let result = compute_sell_amount(&BigInt::from(1000), -1.0, 0.0);
        assert!(result.is_none());
    }
}
