use std::collections::HashMap;

use anyhow::{Result, bail};
use num_bigint::BigInt;
use tracing::info;

use crate::client::RecorderClient;
use crate::config::Config;
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

        Ok(ExchangeEngine {
            wallet,
            chains,
            symbol_to_chain,
            pairs,
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
        let pay_f64 = pay_amount.to_string().parse::<f64>().unwrap_or(0.0);
        let effective_rate = pair.rate * (1.0 + pair.spread / 2.0);
        let sell_f64 = pay_f64 / effective_rate;
        let sell_amount = BigInt::from(sell_f64 as i64);

        if sell_amount <= BigInt::from(0) {
            bail!("sell amount too small after rate conversion");
        }

        let sell_chain_id = pair.sell_chain_id.clone();
        let sell_symbol = pair.sell_symbol.clone();

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

        let result = transfer::execute_transfer(
            sell_client, &sell_chain_id, &[giver], &mut receivers,
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
}
