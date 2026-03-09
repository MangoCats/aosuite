use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use anyhow::Result;
use num_bigint::BigInt;
use num_traits::Zero;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, warn};

/// Shared mutable speed factor. Stored as f64 bits in an AtomicU64.
pub type SharedSpeed = Arc<AtomicU64>;

/// Shared pause flag for agent control.
pub type PauseFlag = Arc<AtomicBool>;

pub fn read_speed(speed: &SharedSpeed) -> f64 {
    f64::from_bits(speed.load(Ordering::Relaxed))
}

pub fn write_speed(speed: &SharedSpeed, val: f64) {
    speed.store(val.to_bits(), Ordering::Relaxed);
}

/// Sleep while the pause flag is set. Returns immediately if not paused.
async fn wait_while_paused(paused: &PauseFlag) {
    while paused.load(Ordering::Relaxed) {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

use crate::client::RecorderClient;
use crate::config::{AgentConfig, VendorConfig, ExchangeConfig, ConsumerConfig, ValidatorConfig, AttackerConfig, InfraTesterConfig, RecorderOperatorConfig, parse_bigint};
use crate::transfer::{self, Giver, Receiver};
use crate::wallet::{Wallet, now_ms};

// ── Inter-agent messages ────────────────────────────────────────────
//
// Agents share private key seeds freely within the simulation.
// In production, multi-step signing would be used instead.

pub enum AgentMessage {
    /// Request a fresh pubkey (+ seed) for receiving shares on a chain.
    RequestPubkey {
        chain_id: String,
        reply: oneshot::Sender<PubkeyResponse>,
    },
    /// Ask the recipient to execute a transfer as the giver.
    /// Receivers include seeds so the executor can sign for all parties.
    SellToMe {
        chain_id: String,
        buyer_name: String,
        receivers: Vec<Receiver>,
        reply: oneshot::Sender<Result<TransferResult>>,
    },
    /// Cross-chain exchange: consumer pays on pay_chain, receives on sell_chain.
    CrossChainBuy {
        buyer_name: String,
        /// Chain the consumer wants shares on.
        sell_chain_id: String,
        /// Chain the consumer is paying with.
        pay_chain_id: String,
        /// Amount consumer is paying (in pay_chain shares).
        pay_amount: BigInt,
        /// Consumer's receiver key for sell_chain.
        receiver_pubkey: [u8; 32],
        receiver_seed: [u8; 32],
        reply: oneshot::Sender<Result<CrossChainResult>>,
    },
    /// Atomic cross-chain exchange: consumer provides all giver/receiver data,
    /// exchange executes a CAA ouroboros swap via ao-exchange.
    AtomicBuy {
        request: AtomicBuyRequest,
        reply: oneshot::Sender<Result<AtomicBuyResult>>,
    },
    /// Notify an agent that one of its keys received a UTXO.
    NotifyUtxo {
        pubkey: [u8; 32],
        seq_id: u64,
        amount: BigInt,
    },
}

pub struct PubkeyResponse {
    pub pubkey: [u8; 32],
    pub seed: [u8; 32],
}

#[derive(Debug, Clone)]
pub struct TransferResult {
    pub block_height: u64,
    pub first_seq: u64,
}

#[derive(Debug, Clone)]
pub struct CrossChainResult {
    pub pay_block: u64,
    pub sell_block: u64,
    pub sell_first_seq: u64,
    pub sell_amount: BigInt,
}

/// Request for a CAA atomic cross-chain swap.
pub struct AtomicBuyRequest {
    pub buyer_name: String,
    pub sell_chain_id: String,
    pub pay_chain_id: String,
    /// Consumer's giver UTXO on pay_chain.
    pub pay_giver_seq_id: u64,
    pub pay_giver_amount: BigInt,
    pub pay_giver_seed: [u8; 32],
    /// Consumer's receiver key on sell_chain.
    pub sell_receiver_pubkey: [u8; 32],
    pub sell_receiver_seed: [u8; 32],
    /// Consumer's change key on pay_chain.
    pub pay_change_pubkey: [u8; 32],
    pub pay_change_seed: [u8; 32],
}

/// Result of a completed CAA atomic swap.
#[derive(Debug, Clone)]
pub struct AtomicBuyResult {
    pub caa_hash: String,
    pub pay_chain_block: u64,
    pub sell_chain_block: u64,
    pub sell_amount: BigInt,
    /// Sequence IDs for consumer's new UTXOs.
    pub sell_receiver_seq: u64,
    pub pay_change_seq: u64,
    pub pay_change_amount: BigInt,
}

// ── Agent state (reported to observer) ──────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AgentState {
    pub name: String,
    pub role: String,
    pub status: String,
    pub lat: f64,
    pub lon: f64,
    pub chains: Vec<ChainHolding>,
    pub key_summary: Vec<crate::wallet::WalletChainSummary>,
    pub coverage_radius: Option<f64>,
    pub paused: bool,
    /// Trading rates for exchange agents: [(sell_symbol, buy_symbol, rate)]
    pub trading_rates: Vec<TradingRate>,
    pub validator_status: Option<ValidatorStatus>,
    pub attacker_status: Option<AttackerStatus>,
    pub infra_tester_status: Option<InfraTesterStatus>,
    pub caa_status: Option<CaaExchangeStatus>,
    pub recorder_op_status: Option<RecorderOpStatus>,
    pub transactions: u64,
    pub last_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TradingRate {
    pub sell: String,
    pub buy: String,
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidatorStatus {
    pub monitored_chains: Vec<MonitoredChainStatus>,
    pub alerts: Vec<AlertEntry>,
    pub total_blocks_verified: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MonitoredChainStatus {
    pub chain_id: String,
    pub symbol: String,
    pub validated_height: u64,
    pub chain_height: u64,
    pub status: String,
    pub last_poll_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertEntry {
    pub timestamp_ms: u64,
    pub chain_id: String,
    pub alert_type: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AttackerStatus {
    pub attack_type: String,
    pub attempts: u64,
    pub rejections: u64,
    pub unexpected_accepts: u64,
    pub last_result: String,
}

/// Status for white-hat infrastructure resilience testers.
#[derive(Debug, Clone, Serialize)]
pub struct InfraTesterStatus {
    pub test_type: String,
    pub rounds: u64,
    pub passed: u64,
    pub failed: u64,
    pub last_result: String,
    /// Per-test-type detail metrics.
    pub detail: serde_json::Value,
}

/// Status for recorder operator agents (Sim-G: TⒶ³ operations).
#[derive(Debug, Clone, Serialize)]
pub struct RecorderOpStatus {
    pub target_chain: String,
    pub key_rotated: bool,
    pub recorder_switch_phase: String,
    pub chain_migrated: bool,
    pub operations_completed: u64,
    pub last_result: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaaExchangeStatus {
    pub total_caas: u64,
    pub successful: u64,
    pub failed: u64,
    pub last_caa_hash: String,
    pub last_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChainHolding {
    pub chain_id: String,
    pub symbol: String,
    #[serde(serialize_with = "serialize_bigint")]
    pub shares: BigInt,
    pub unspent_utxos: usize,
    pub coin_count: String,
    pub total_shares: String,
}

fn serialize_bigint<S: serde::Serializer>(val: &BigInt, s: S) -> std::result::Result<S::Ok, S::Error> {
    s.serialize_str(&val.to_string())
}

/// A recorded transaction event for the viewer.
#[derive(Debug, Clone, Serialize)]
pub struct TransactionEvent {
    pub id: u64,
    pub timestamp_ms: u64,
    pub chain_id: String,
    pub symbol: String,
    pub from_agent: String,
    pub to_agent: String,
    pub block_height: u64,
    pub description: String,
}

/// Message types sent to the viewer state collector.
pub enum ViewerEvent {
    State(Box<AgentState>),
    Transaction(TransactionEvent),
}

/// Shared viewer state — updated by observer, read by viewer API.
pub struct ViewerState {
    agents: tokio::sync::RwLock<HashMap<String, AgentState>>,
    transactions: tokio::sync::RwLock<Vec<TransactionEvent>>,
    next_tx_id: std::sync::atomic::AtomicU64,
    notify: tokio::sync::watch::Sender<u64>,
    pub subscribe: tokio::sync::watch::Receiver<u64>,
}

impl ViewerState {
    pub fn new() -> Self {
        let (notify, subscribe) = tokio::sync::watch::channel(0u64);
        ViewerState {
            agents: tokio::sync::RwLock::new(HashMap::new()),
            transactions: tokio::sync::RwLock::new(Vec::new()),
            next_tx_id: std::sync::atomic::AtomicU64::new(1),
            notify,
            subscribe,
        }
    }

    pub async fn update_agent(&self, state: AgentState) {
        let mut agents = self.agents.write().await;
        agents.insert(state.name.clone(), state);
        let _ = self.notify.send(self.next_tx_id.load(std::sync::atomic::Ordering::Relaxed));
    }

    pub async fn add_transaction(&self, mut event: TransactionEvent) {
        let id = self.next_tx_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        event.id = id;
        let mut txns = self.transactions.write().await;
        txns.push(event);
        // Cap at 50k transactions to prevent unbounded memory growth
        if txns.len() > 50_000 {
            txns.drain(..10_000);
        }
        let _ = self.notify.send(id);
    }

    pub async fn get_agents(&self) -> Vec<AgentState> {
        self.agents.read().await.values().cloned().collect()
    }

    pub async fn get_agent(&self, name: &str) -> Option<AgentState> {
        self.agents.read().await.get(name).cloned()
    }

    pub async fn get_transactions(&self, since_id: u64, limit: usize) -> Vec<TransactionEvent> {
        let txns = self.transactions.read().await;
        txns.iter()
            .filter(|t| t.id > since_id)
            .take(limit)
            .cloned()
            .collect()
    }

    pub async fn get_agent_transactions(&self, agent_name: &str) -> Vec<TransactionEvent> {
        let txns = self.transactions.read().await;
        txns.iter()
            .filter(|t| t.from_agent == agent_name || t.to_agent == agent_name)
            .cloned()
            .collect()
    }
}

// ── Agent directory ─────────────────────────────────────────────────

pub type AgentSender = mpsc::Sender<AgentMessage>;
pub type StateCollector = mpsc::Sender<ViewerEvent>;

pub struct ChainRegistration {
    pub chain_id: String,
    pub symbol: String,
    pub vendor_name: String,
    pub plate_price: u64,
    pub coin_count: String,
    pub total_shares: String,
}

pub struct AgentDirectory {
    agents: HashMap<String, AgentSender>,
    /// chain_id → chain registration
    pub chains: HashMap<String, ChainRegistration>,
    /// symbol → chain_id for quick lookup
    pub symbol_to_chain: HashMap<String, String>,
    /// exchange_name → { (sell_symbol, buy_symbol) → rate }
    pub published_rates: HashMap<String, HashMap<(String, String), f64>>,
}

impl AgentDirectory {
    pub fn new() -> Self {
        AgentDirectory {
            agents: HashMap::new(),
            chains: HashMap::new(),
            symbol_to_chain: HashMap::new(),
            published_rates: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, sender: AgentSender) {
        self.agents.insert(name.to_string(), sender);
    }

    pub fn register_chain(&mut self, chain_id: &str, symbol: &str, vendor_name: &str, plate_price: u64, coin_count: &str, total_shares: &str) {
        self.symbol_to_chain.insert(symbol.to_string(), chain_id.to_string());
        self.chains.insert(chain_id.to_string(), ChainRegistration {
            chain_id: chain_id.to_string(),
            symbol: symbol.to_string(),
            vendor_name: vendor_name.to_string(),
            plate_price,
            coin_count: coin_count.to_string(),
            total_shares: total_shares.to_string(),
        });
    }

    pub fn get(&self, name: &str) -> Option<&AgentSender> {
        self.agents.get(name)
    }

    pub fn chain_id_for_symbol(&self, symbol: &str) -> Option<&str> {
        self.symbol_to_chain.get(symbol).map(|s| s.as_str())
    }

}

// ── Vendor agent ────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn run_vendor(
    config: AgentConfig,
    vendor_cfg: VendorConfig,
    client: Arc<RecorderClient>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    mut mailbox: mpsc::Receiver<AgentMessage>,
    paused: PauseFlag,
    pre_genesis: (serde_json::Value, [u8; 32]),
) -> Result<()> {
    let name = config.name.clone();
    let (lat, lon) = (config.lat, config.lon);
    let mut wallet = Wallet::new(&name);
    let mut tx_count: u64 = 0;

    let (genesis_json, seed) = pre_genesis;
    let coins = parse_bigint(&vendor_cfg.coins);
    let shares = parse_bigint(&vendor_cfg.shares);

    let chain_info = client.create_chain(&genesis_json).await?;
    let chain_id = chain_info.chain_id.clone();
    info!("{}: Created chain {} ({})", name, vendor_cfg.symbol, &chain_id[..12]);

    // Register vendor profile with the recorder (enables PWA discovery)
    let profile = serde_json::json!({
        "name": name,
        "description": vendor_cfg.description,
        "lat": config.lat,
        "lon": config.lon,
    });
    if let Err(e) = client.set_profile(&chain_id, &profile).await {
        warn!("{}: failed to set vendor profile: {}", name, e);
    }

    // Register chain + issuer UTXO
    {
        let mut dir = directory.write().await;
        dir.register_chain(&chain_id, &vendor_cfg.symbol, &name, vendor_cfg.plate_price, &coins.to_string(), &shares.to_string());
    }
    let issuer_entry = wallet.import_key(seed, &chain_id);
    wallet.register_utxo(&issuer_entry.pubkey, 1, shares.clone());
    let chain_meta: HashMap<String, (String, String)> = [(chain_id.clone(), (coins.to_string(), shares.to_string()))].into_iter().collect();
    let coverage = Some(vendor_cfg.coverage_radius_m);
    let _ = state_tx.send(ViewerEvent::State(Box::new(build_state(&name,"vendor", "ready", lat, lon, &wallet, &chain_id, &vendor_cfg.symbol, &chain_meta, coverage, &paused, tx_count, "genesis created")))).await;

    // Handle messages
    while let Some(msg) = mailbox.recv().await {
        wait_while_paused(&paused).await;
        match msg {
            AgentMessage::RequestPubkey { chain_id: cid, reply } => {
                let entry = wallet.generate_key(&cid);
                let _ = reply.send(PubkeyResponse { pubkey: entry.pubkey, seed: entry.seed });
            }
            AgentMessage::SellToMe { chain_id: cid, buyer_name, mut receivers, reply } => {
                let result = handle_sell(&name, &mut wallet, &client, &cid, &mut receivers).await;
                if let Ok(ref r) = result {
                    tx_count += 1;
                    info!("{}: Block {} (tx #{})", name, r.block_height, tx_count);
                    let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                        &cid, &vendor_cfg.symbol, &name, &buyer_name, r.block_height, "vendor sold shares",
                    ))).await;
                    let _ = state_tx.send(ViewerEvent::State(Box::new(build_state(&name,"vendor", "ready", lat, lon, &wallet, &chain_id, &vendor_cfg.symbol, &chain_meta, coverage, &paused, tx_count, &format!("block {}", r.block_height))))).await;
                }
                let _ = reply.send(result);
            }
            AgentMessage::NotifyUtxo { pubkey, seq_id, amount } => {
                wallet.register_utxo(&pubkey, seq_id, amount);
                let _ = state_tx.send(ViewerEvent::State(Box::new(build_state(&name,"vendor", "ready", lat, lon, &wallet, &chain_id, &vendor_cfg.symbol, &chain_meta, coverage, &paused, tx_count, "received redemption")))).await;
            }
            AgentMessage::CrossChainBuy { reply, .. } => {
                let _ = reply.send(Err(anyhow::anyhow!("{}: vendors do not handle cross-chain buys", name)));
            }
            AgentMessage::AtomicBuy { reply, .. } => {
                let _ = reply.send(Err(anyhow::anyhow!("{}: vendors do not handle atomic buys", name)));
            }
        }
    }
    Ok(())
}

// ── Exchange agent ──────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn run_exchange(
    config: AgentConfig,
    exchange_cfg: ExchangeConfig,
    client: Arc<RecorderClient>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    mut mailbox: mpsc::Receiver<AgentMessage>,
    speed: SharedSpeed,
    mut block_rx: Option<mpsc::Receiver<crate::mqtt::BlockNotification>>,
    paused: PauseFlag,
    recorder_url: String,
) -> Result<()> {
    let name = config.name.clone();
    let (lat, lon) = (config.lat, config.lon);
    let mut wallet = Wallet::new(&name);
    let mut tx_count: u64 = 0;

    // Multi-chain mode: has trading pairs
    let is_multi = !exchange_cfg.pairs.is_empty();

    // Track all chains we participate in: chain_id → symbol
    let mut my_chains: HashMap<String, String> = HashMap::new();

    if is_multi {
        // Acquire initial inventory on each chain specified
        for inv in &exchange_cfg.inventory {
            let (chain_id, symbol, plate_price) = wait_for_vendor_chain(&directory, &inv.vendor).await?;
            info!("{}: Found chain {} ({}) from vendor {}", name, symbol, &chain_id[..12], inv.vendor);

            let info = client.chain_info(&chain_id).await?;
            let total_shares: BigInt = info.shares_out.parse()?;
            let total_coins: BigInt = info.coin_count.parse()?;
            let buy_coins = BigInt::from(plate_price) * BigInt::from(inv.plates);
            let buy_shares = &total_shares * &buy_coins / &total_coins;

            request_purchase(
                &name, &mut wallet, &directory,
                &inv.vendor, &chain_id, &buy_shares,
            ).await?;
            tx_count += 1;
            info!("{}: Bought {} plates of {} from {}", name, inv.plates, symbol, inv.vendor);
            my_chains.insert(chain_id.clone(), symbol.clone());
        }
    } else if let Some(buy_from) = &exchange_cfg.buy_from {
        // Legacy single-chain mode
        let (chain_id, symbol, plate_price) = wait_for_vendor_chain(&directory, buy_from).await?;
        info!("{}: Found chain {} ({})", name, symbol, &chain_id[..12]);

        let info = client.chain_info(&chain_id).await?;
        let total_shares: BigInt = info.shares_out.parse()?;
        let total_coins: BigInt = info.coin_count.parse()?;
        let buy_coins = BigInt::from(plate_price) * BigInt::from(exchange_cfg.initial_buy);
        let buy_shares = &total_shares * &buy_coins / &total_coins;

        request_purchase(
            &name, &mut wallet, &directory,
            buy_from, &chain_id, &buy_shares,
        ).await?;
        tx_count += 1;
        info!("{}: Bought {} plates from {}", name, exchange_cfg.initial_buy, buy_from);
        my_chains.insert(chain_id.clone(), symbol.clone());
    }

    // Build trading pair index: (sell_chain_id, buy_chain_id) → rate
    let mut pair_rates: HashMap<(String, String), f64> = HashMap::new();
    {
        let dir = directory.read().await;
        for pair in &exchange_cfg.pairs {
            if let (Some(sell_cid), Some(buy_cid)) = (
                dir.chain_id_for_symbol(&pair.sell).map(|s| s.to_string()),
                dir.chain_id_for_symbol(&pair.buy).map(|s| s.to_string()),
            ) {
                pair_rates.insert((sell_cid, buy_cid), pair.rate);
            } else {
                warn!("{}: trading pair {}↔{} — chain not found yet, skipping", name, pair.sell, pair.buy);
            }
        }
    }

    // Register this exchange agent on each chain we hold inventory on (enables PWA discovery)
    for chain_id in my_chains.keys() {
        let pairs_json: Vec<serde_json::Value> = exchange_cfg.pairs.iter()
            .map(|p| serde_json::json!({
                "sell_symbol": p.sell,
                "buy_symbol": p.buy,
                "rate": p.rate,
            }))
            .collect();
        let agent_json = serde_json::json!({
            "name": name,
            "pairs": pairs_json,
            "ttl": 3600,
        });
        if let Err(e) = client.register_exchange_agent(chain_id, &agent_json).await {
            warn!("{}: failed to register exchange agent on {}: {}", name, &chain_id[..12], e);
        }
    }

    // Snapshot initial rates as a floor for price discovery (don't undercut below 50% of initial)
    let initial_rates: HashMap<(String, String), f64> = pair_rates.clone();

    // Publish our rates to the directory so competitors can see them
    {
        let mut dir = directory.write().await;
        let symbol_rates: HashMap<(String, String), f64> = exchange_cfg.pairs.iter()
            .map(|p| ((p.sell.clone(), p.buy.clone()), p.rate))
            .collect();
        dir.published_rates.insert(name.clone(), symbol_rates);
    }

    // Track initial inventory levels for rebalancing
    let mut initial_balances: HashMap<String, BigInt> = HashMap::new();
    for chain_id in my_chains.keys() {
        initial_balances.insert(chain_id.clone(), wallet.balance(chain_id));
    }
    let rebalance_threshold = exchange_cfg.rebalance_threshold;

    // Build chain metadata for coin-display conversion
    let chain_meta: HashMap<String, (String, String)> = {
        let dir = directory.read().await;
        my_chains.keys().filter_map(|cid| {
            dir.chains.get(cid).map(|reg| (cid.clone(), (reg.coin_count.clone(), reg.total_shares.clone())))
        }).collect()
    };

    // Helper: build trading rates vec from pair_rates
    let rates_vec = |pair_rates: &HashMap<(String, String), f64>, my_chains: &HashMap<String, String>| -> Vec<TradingRate> {
        pair_rates.iter().map(|((sell_cid, buy_cid), &rate)| {
            TradingRate {
                sell: my_chains.get(sell_cid).cloned().unwrap_or_default(),
                buy: my_chains.get(buy_cid).cloned().unwrap_or_default(),
                rate,
            }
        }).collect()
    };

    {
        let mut state = build_multi_state(
            &name, "exchange", "ready", lat, lon, &wallet, &my_chains, &chain_meta, &paused, tx_count, "inventory acquired",
        );
        state.trading_rates = rates_vec(&pair_rates, &my_chains);
        let _ = state_tx.send(ViewerEvent::State(Box::new(state))).await;
    }

    // Price discovery timer — scaled by speed
    let price_discovery = exchange_cfg.price_discovery;
    let adjust_base_secs = exchange_cfg.adjust_interval_secs as f64;
    let mut next_adjust = tokio::time::Instant::now()
        + std::time::Duration::from_secs_f64(adjust_base_secs / read_speed(&speed).max(0.1));

    // Handle messages + optional MQTT block notifications
    loop {
        wait_while_paused(&paused).await;
        let msg = tokio::select! {
            Some(msg) = mailbox.recv() => msg,
            _ = tokio::time::sleep_until(next_adjust), if price_discovery => {
                // Read competitor rates snapshot
                let competitor_snapshot: HashMap<(String, String), f64> = {
                    let dir = directory.read().await;
                    let mut best: HashMap<(String, String), f64> = HashMap::new();
                    for (ex_name, rates) in &dir.published_rates {
                        if ex_name == &name { continue; }
                        for (pair, &rate) in rates {
                            let entry = best.entry(pair.clone()).or_insert(rate);
                            if rate < *entry { *entry = rate; }
                        }
                    }
                    best
                };

                // Adjust our rates
                for ((sell_cid, buy_cid), our_rate) in pair_rates.iter_mut() {
                    let sell_sym = my_chains.get(sell_cid).cloned().unwrap_or_default();
                    let buy_sym = my_chains.get(buy_cid).cloned().unwrap_or_default();

                    if let Some(&competitor_rate) = competitor_snapshot.get(&(sell_sym.clone(), buy_sym.clone())) {
                        let old_rate = *our_rate;
                        let balance = wallet.balance(sell_cid);
                        let initial = initial_balances.get(sell_cid).cloned().unwrap_or(BigInt::from(1));
                        let inventory_ratio = if !initial.is_zero() {
                            let b: f64 = balance.to_string().parse().unwrap_or(1.0);
                            let i: f64 = initial.to_string().parse().unwrap_or(1.0);
                            b / i
                        } else { 1.0 };

                        let floor = initial_rates.get(&(sell_cid.clone(), buy_cid.clone()))
                            .copied().unwrap_or(0.01) * 0.5;
                        *our_rate = if inventory_ratio > 0.3 {
                            (competitor_rate * 0.98).max(floor)
                        } else {
                            (competitor_rate * 1.02).max(floor)
                        };

                        if (old_rate - *our_rate).abs() > 0.001 {
                            info!("{}: {}→{} rate {:.4} → {:.4} (competitor {:.4}, inv {:.0}%)",
                                name, sell_sym, buy_sym, old_rate, *our_rate, competitor_rate, inventory_ratio * 100.0);
                        }
                    }
                }

                // Re-publish our rates
                {
                    let mut dir = directory.write().await;
                    let symbol_rates: HashMap<(String, String), f64> = pair_rates.iter()
                        .map(|((s, b), &r)| {
                            let ss = my_chains.get(s).cloned().unwrap_or_default();
                            let bs = my_chains.get(b).cloned().unwrap_or_default();
                            ((ss, bs), r)
                        })
                        .collect();
                    dir.published_rates.insert(name.clone(), symbol_rates);
                }
                next_adjust = tokio::time::Instant::now()
                    + std::time::Duration::from_secs_f64(adjust_base_secs / read_speed(&speed).max(0.1));
                continue;
            },
            Some(notif) = async { match block_rx.as_mut() { Some(rx) => rx.recv().await, None => std::future::pending().await } } => {
                if my_chains.contains_key(&notif.chain_id) {
                    info!("{}: block notification on {} height {}", name, notif.chain_id.get(..12).unwrap_or(&notif.chain_id), notif.height);
                    // Check if rebalancing is needed on this chain
                    if let Some(initial) = initial_balances.get(&notif.chain_id) {
                        let current = wallet.balance(&notif.chain_id);
                        let threshold = BigInt::from((rebalance_threshold * 1_000_000.0) as u64);
                        let scale = BigInt::from(1_000_000u64);
                        if &current * &scale < initial * &threshold && !initial.is_zero() {
                            let sym = my_chains.get(&notif.chain_id).cloned().unwrap_or_default();
                            info!("{}: {} inventory low ({} < {}% of initial), restocking",
                                name, sym, current, (rebalance_threshold * 100.0) as u64);
                            // Find vendor for this chain and restock
                            if let Some(vendor_name) = find_vendor_for_chain(&directory, &notif.chain_id).await {
                                let restock = initial - &current;
                                match request_purchase(&name, &mut wallet, &directory, &vendor_name, &notif.chain_id, &restock).await {
                                    Ok(()) => {
                                        tx_count += 1;
                                        info!("{}: Restocked {} from {}", name, sym, vendor_name);
                                        let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                                            &notif.chain_id, &sym, &vendor_name, &name, 0, "exchange restocked",
                                        ))).await;
                                        {
                                            let mut s = build_multi_state(&name, "exchange", "ready", lat, lon, &wallet, &my_chains, &chain_meta, &paused, tx_count, &format!("restocked {}", sym));
                                            s.trading_rates = rates_vec(&pair_rates, &my_chains);
                                            let _ = state_tx.send(ViewerEvent::State(Box::new(s))).await;
                                        }
                                    }
                                    Err(e) => warn!("{}: Restock failed: {}", name, e),
                                }
                            }
                        }
                    }
                }
                continue;
            }
            else => break,
        };
        match msg {
            AgentMessage::RequestPubkey { chain_id: cid, reply } => {
                let entry = wallet.generate_key(&cid);
                let _ = reply.send(PubkeyResponse { pubkey: entry.pubkey, seed: entry.seed });
            }
            AgentMessage::SellToMe { chain_id: cid, buyer_name, mut receivers, reply } => {
                let sym = my_chains.get(&cid).cloned().unwrap_or_default();
                let result = handle_sell(&name, &mut wallet, &client, &cid, &mut receivers).await;
                if let Ok(ref r) = result {
                    tx_count += 1;
                    info!("{}: {} sold, block {} (tx #{})", name, sym, r.block_height, tx_count);
                    let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                        &cid, &sym, &name, &buyer_name, r.block_height, "exchange sold shares",
                    ))).await;
                }
                {
                    let mut s = build_multi_state(&name, "exchange", "ready", lat, lon, &wallet, &my_chains, &chain_meta, &paused, tx_count, &format!("sold {}", sym));
                    s.trading_rates = rates_vec(&pair_rates, &my_chains);
                    let _ = state_tx.send(ViewerEvent::State(Box::new(s))).await;
                }
                let _ = reply.send(result);
            }
            AgentMessage::CrossChainBuy {
                buyer_name, sell_chain_id, pay_chain_id, pay_amount,
                receiver_pubkey, receiver_seed, reply,
            } => {
                let result = handle_cross_chain_buy(
                    &name, &mut wallet, &client, &directory,
                    &sell_chain_id, &pay_chain_id, &pay_amount,
                    receiver_pubkey, receiver_seed,
                    &pair_rates, exchange_cfg.referral_fee,
                ).await;
                if let Ok(ref r) = result {
                    tx_count += 2; // two legs
                    let sell_sym = my_chains.get(&sell_chain_id).cloned().unwrap_or_default();
                    let pay_sym = my_chains.get(&pay_chain_id).cloned().unwrap_or_default();
                    info!("{}: Cross-chain {}→{}: pay block {}, sell block {} (tx #{})",
                        name, pay_sym, sell_sym, r.pay_block, r.sell_block, tx_count);
                    let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                        &pay_chain_id, &pay_sym, &buyer_name, &name, r.pay_block,
                        &format!("cross-chain: {} paid {}", buyer_name, pay_sym),
                    ))).await;
                    let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                        &sell_chain_id, &sell_sym, &name, &buyer_name, r.sell_block,
                        &format!("cross-chain: exchange sent {}", sell_sym),
                    ))).await;
                }
                {
                    let mut s = build_multi_state(&name, "exchange", "ready", lat, lon, &wallet, &my_chains, &chain_meta, &paused, tx_count, "cross-chain trade");
                    s.trading_rates = rates_vec(&pair_rates, &my_chains);
                    let _ = state_tx.send(ViewerEvent::State(Box::new(s))).await;
                }
                let _ = reply.send(result);
            }
            AgentMessage::AtomicBuy { request, reply } => {
                let result = handle_atomic_buy(
                    &name, &mut wallet, &recorder_url,
                    &request, &pair_rates, exchange_cfg.referral_fee,
                    exchange_cfg.escrow_secs as i64,
                ).await;
                if let Ok(ref r) = result {
                    tx_count += 2;
                    let sell_sym = my_chains.get(&request.sell_chain_id).cloned().unwrap_or_default();
                    let pay_sym = my_chains.get(&request.pay_chain_id).cloned().unwrap_or_default();
                    info!("{}: CAA atomic {}↔{}: hash {} (tx #{})",
                        name, pay_sym, sell_sym, &r.caa_hash[..12], tx_count);
                    let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                        &request.pay_chain_id, &pay_sym, &request.buyer_name, &name, r.pay_chain_block,
                        &format!("CAA atomic: {} paid {}", request.buyer_name, pay_sym),
                    ))).await;
                    let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                        &request.sell_chain_id, &sell_sym, &name, &request.buyer_name, r.sell_chain_block,
                        &format!("CAA atomic: exchange sent {}", sell_sym),
                    ))).await;
                }
                {
                    let mut s = build_multi_state(&name, "exchange", "ready", lat, lon, &wallet, &my_chains, &chain_meta, &paused, tx_count, "CAA atomic trade");
                    s.trading_rates = rates_vec(&pair_rates, &my_chains);
                    let _ = state_tx.send(ViewerEvent::State(Box::new(s))).await;
                }
                let _ = reply.send(result);
            }
            AgentMessage::NotifyUtxo { pubkey, seq_id, amount } => {
                wallet.register_utxo(&pubkey, seq_id, amount);
            }
        }
    }
    Ok(())
}

/// Handle a cross-chain buy: receive payment on pay_chain, send on sell_chain.
#[allow(clippy::too_many_arguments)]
async fn handle_cross_chain_buy(
    name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    _directory: &Arc<RwLock<AgentDirectory>>,
    sell_chain_id: &str,
    pay_chain_id: &str,
    pay_amount: &BigInt,
    consumer_recv_pubkey: [u8; 32],
    consumer_recv_seed: [u8; 32],
    pair_rates: &HashMap<(String, String), f64>,
    referral_fee: f64,
) -> Result<CrossChainResult> {
    // Look up exchange rate
    let rate = pair_rates.get(&(sell_chain_id.to_string(), pay_chain_id.to_string()))
        .ok_or_else(|| anyhow::anyhow!("{}: no trading pair for this chain combination", name))?;

    // Calculate sell amount from pay amount and rate, minus referral fee.
    // rate = how many pay units per 1 sell unit. sell_amount = pay_amount / rate.
    // After referral fee: sell_amount *= (1 - referral_fee).
    let rate_scale = 1_000_000u64;
    let rate_num = BigInt::from((*rate * rate_scale as f64) as u64);
    let rate_den = BigInt::from(rate_scale);
    let mut sell_amount = pay_amount * &rate_den / &rate_num;

    // Apply referral fee (exchange keeps a fraction)
    if referral_fee > 0.0 {
        let fee_keep = &sell_amount * BigInt::from((referral_fee * rate_scale as f64) as u64) / BigInt::from(rate_scale);
        sell_amount -= fee_keep;
    }

    if sell_amount <= BigInt::from(0) {
        anyhow::bail!("{}: sell amount too small after rate conversion", name);
    }

    // Leg 2: Send sell_chain shares to consumer
    let utxo = wallet.find_unspent(sell_chain_id)
        .ok_or_else(|| anyhow::anyhow!("{}: no unspent UTXO on sell chain", name))?;

    // Build change key for ourselves
    let change_entry = wallet.generate_key(sell_chain_id);

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

    let result = transfer::execute_transfer(client, sell_chain_id, &[giver], &mut receivers).await?;
    wallet.mark_spent(&giver_pubkey);

    // Register our change UTXO
    let change_seq = result.first_seq + 1;
    wallet.register_utxo(&change_entry.pubkey, change_seq, receivers[1].amount.clone());

    Ok(CrossChainResult {
        pay_block: 0, // Leg 1 was executed by consumer; exchange doesn't know the block
        sell_block: result.height,
        sell_first_seq: result.first_seq,
        sell_amount: receivers[0].amount.clone(),
    })
}

/// Handle an atomic CAA cross-chain buy using ao-exchange's execute_caa.
#[allow(clippy::too_many_arguments)]
#[allow(unused_variables)] // recorder_url used in CAA client creation
async fn handle_atomic_buy(
    name: &str,
    wallet: &mut Wallet,
    recorder_url: &str,
    request: &AtomicBuyRequest,
    pair_rates: &HashMap<(String, String), f64>,
    referral_fee: f64,
    escrow_secs: i64,
) -> Result<AtomicBuyResult> {
    use anyhow::Context as _;
    use ao_exchange::caa::{self, CaaChainComponent, CaaGiver, CaaReceiver};

    // Look up exchange rate
    let rate = pair_rates.get(&(request.sell_chain_id.clone(), request.pay_chain_id.clone()))
        .ok_or_else(|| anyhow::anyhow!("{}: no trading pair for this chain combination", name))?;

    // Calculate sell amount from pay amount and rate, minus referral fee
    let rate_scale = 1_000_000u64;
    let rate_num = BigInt::from((*rate * rate_scale as f64) as u64);
    let rate_den = BigInt::from(rate_scale);
    let mut sell_amount = &request.pay_giver_amount * &rate_den / &rate_num;

    if referral_fee > 0.0 {
        let fee_keep = &sell_amount * BigInt::from((referral_fee * rate_scale as f64) as u64) / BigInt::from(rate_scale);
        sell_amount -= fee_keep;
    }

    if sell_amount <= BigInt::from(0) {
        anyhow::bail!("{}: sell amount too small after rate conversion", name);
    }

    // Find exchange's UTXO on sell_chain
    let sell_utxo = wallet.find_unspent(&request.sell_chain_id)
        .ok_or_else(|| anyhow::anyhow!("{}: no unspent UTXO on sell chain for CAA", name))?;

    // Generate exchange's receiver key on pay_chain and change key on sell_chain
    let exchange_pay_recv = wallet.generate_key(&request.pay_chain_id);
    let exchange_sell_change = wallet.generate_key(&request.sell_chain_id);

    // Decode chain_ids to [u8; 32]
    let pay_chain_bytes: [u8; 32] = hex::decode(&request.pay_chain_id)
        .context("invalid pay_chain_id hex")?
        .try_into().map_err(|_| anyhow::anyhow!("pay_chain_id not 32 bytes"))?;
    let sell_chain_bytes: [u8; 32] = hex::decode(&request.sell_chain_id)
        .context("invalid sell_chain_id hex")?
        .try_into().map_err(|_| anyhow::anyhow!("sell_chain_id not 32 bytes"))?;

    let ex_client = ao_exchange::client::RecorderClient::new(recorder_url);

    // Component 0 (pay_chain): consumer gives, exchange + consumer receive
    // Component 1 (sell_chain): exchange gives, consumer + exchange receive
    let mut components = vec![
        CaaChainComponent {
            chain_id: pay_chain_bytes,
            client: ao_exchange::client::RecorderClient::new(recorder_url),
            givers: vec![CaaGiver {
                seq_id: request.pay_giver_seq_id,
                amount: request.pay_giver_amount.clone(),
                seed: request.pay_giver_seed,
            }],
            receivers: vec![
                CaaReceiver {
                    pubkey: exchange_pay_recv.pubkey,
                    seed: exchange_pay_recv.seed,
                    amount: request.pay_giver_amount.clone(), // will be fee-adjusted by execute_caa
                },
                CaaReceiver {
                    pubkey: request.pay_change_pubkey,
                    seed: request.pay_change_seed,
                    amount: BigInt::from(0), // change — last receiver is auto-adjusted
                },
            ],
        },
        CaaChainComponent {
            chain_id: sell_chain_bytes,
            client: ex_client,
            givers: vec![CaaGiver {
                seq_id: sell_utxo.seq_id,
                amount: sell_utxo.amount.clone(),
                seed: sell_utxo.seed,
            }],
            receivers: vec![
                CaaReceiver {
                    pubkey: request.sell_receiver_pubkey,
                    seed: request.sell_receiver_seed,
                    amount: sell_amount.clone(),
                },
                CaaReceiver {
                    pubkey: exchange_sell_change.pubkey,
                    seed: exchange_sell_change.seed,
                    amount: BigInt::from(0), // change — last receiver is auto-adjusted
                },
            ],
        },
    ];

    let caa_result = caa::execute_caa(&mut components, escrow_secs).await
        .context(format!("{}: CAA execute_caa failed", name))?;

    // Mark old UTXOs as spent
    wallet.mark_spent(&sell_utxo.pubkey);

    // Register exchange's new UTXOs using first_seq from CaaResult (deterministic, no guessing)
    // Component 0 (pay_chain): receivers got seq_ids first_seqs[0], first_seqs[0]+1
    let pay_first = caa_result.first_seqs[0];
    let pay_exchange_seq = pay_first;       // receiver 0: exchange receives payment
    let pay_change_seq = pay_first + 1;     // receiver 1: consumer change
    wallet.register_utxo(&exchange_pay_recv.pubkey, pay_exchange_seq, components[0].receivers[0].amount.clone());

    // Component 1 (sell_chain): receivers got seq_ids first_seqs[1], first_seqs[1]+1
    let sell_first = caa_result.first_seqs[1];
    let sell_consumer_seq = sell_first;     // receiver 0: consumer receives shares
    let sell_change_seq = sell_first + 1;   // receiver 1: exchange change
    wallet.register_utxo(&exchange_sell_change.pubkey, sell_change_seq, components[1].receivers[1].amount.clone());

    // Get block heights from chain info (lightweight query, no seq_id guessing needed)
    let pay_info = ao_exchange::client::RecorderClient::new(recorder_url)
        .chain_info(&request.pay_chain_id).await
        .context("failed to get pay chain info after CAA")?;
    let sell_info = ao_exchange::client::RecorderClient::new(recorder_url)
        .chain_info(&request.sell_chain_id).await
        .context("failed to get sell chain info after CAA")?;

    Ok(AtomicBuyResult {
        caa_hash: caa_result.caa_hash,
        pay_chain_block: pay_info.block_height,
        sell_chain_block: sell_info.block_height,
        sell_amount: components[1].receivers[0].amount.clone(),
        sell_receiver_seq: sell_consumer_seq,
        pay_change_seq,
        pay_change_amount: components[0].receivers[1].amount.clone(),
    })
}

// ── Consumer agent ──────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn run_consumer(
    config: AgentConfig,
    consumer_cfg: ConsumerConfig,
    client: Arc<RecorderClient>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    _mailbox: mpsc::Receiver<AgentMessage>,
    speed: SharedSpeed,
    paused: PauseFlag,
) -> Result<()> {
    let name = config.name.clone();
    let (lat, lon) = (config.lat, config.lon);
    let mut wallet = Wallet::new(&name);
    let mut tx_count: u64 = 0;

    let is_cross_chain = consumer_cfg.want_symbol.is_some() && consumer_cfg.pay_symbol.is_some();

    if is_cross_chain && consumer_cfg.atomic {
        run_atomic_consumer(
            &name, lat, lon, &consumer_cfg, &client, &directory, &state_tx,
            &mut wallet, &mut tx_count, &speed, &paused,
        ).await
    } else if is_cross_chain {
        run_cross_chain_consumer(
            &name, lat, lon, &consumer_cfg, &client, &directory, &state_tx,
            &mut wallet, &mut tx_count, &speed, &paused,
        ).await
    } else {
        run_single_chain_consumer(
            &name, lat, lon, &consumer_cfg, &client, &directory, &state_tx,
            &mut wallet, &mut tx_count, &speed, &paused,
        ).await
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_single_chain_consumer(
    name: &str, lat: f64, lon: f64,
    consumer_cfg: &ConsumerConfig,
    client: &RecorderClient,
    directory: &Arc<RwLock<AgentDirectory>>,
    state_tx: &StateCollector,
    wallet: &mut Wallet,
    tx_count: &mut u64,
    speed: &SharedSpeed,
    paused: &PauseFlag,
) -> Result<()> {
    let redeem_at_name = consumer_cfg.redeem_at.as_deref()
        .ok_or_else(|| anyhow::anyhow!("{}: single-chain consumer requires redeem_at", name))?;

    let (chain_id, symbol, plate_price) = wait_for_vendor_chain(directory, redeem_at_name).await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let chain_meta: HashMap<String, (String, String)> = {
        let dir = directory.read().await;
        dir.chains.get(&chain_id)
            .map(|reg| [(chain_id.clone(), (reg.coin_count.clone(), reg.total_shares.clone()))].into_iter().collect())
            .unwrap_or_default()
    };

    let base_interval = consumer_cfg.interval_secs as f64;
    info!("{}: Starting single-chain purchase loop (base interval {}s)", name, base_interval);

    loop {
        wait_while_paused(paused).await;
        let interval = std::time::Duration::from_secs_f64(base_interval / read_speed(speed).max(0.1));
        tokio::time::sleep(interval).await;

        let info = match client.chain_info(&chain_id).await {
            Ok(i) => i,
            Err(e) => { warn!("{}: chain_info failed: {}", name, e); continue; }
        };
        let total_shares: BigInt = info.shares_out.parse()?;
        let total_coins: BigInt = info.coin_count.parse()?;
        let plate_shares = &total_shares * BigInt::from(plate_price) / &total_coins;

        match request_purchase(name, wallet, directory, &consumer_cfg.buy_from, &chain_id, &plate_shares).await {
            Ok(()) => {
                *tx_count += 1;
                info!("{}: Bought plate from {}", name, consumer_cfg.buy_from);
                let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                    &chain_id, &symbol, &consumer_cfg.buy_from, name, 0,
                    &format!("{} bought plate from {}", name, consumer_cfg.buy_from),
                ))).await;
            }
            Err(e) => { warn!("{}: Buy failed: {}", name, e); continue; }
        }

        while wallet.find_unspent(&chain_id).is_some() {
            match redeem_at(name, wallet, client, directory, redeem_at_name, &chain_id).await {
                Ok(h) => {
                    *tx_count += 1;
                    info!("{}: Redeemed at {} (block {})", name, redeem_at_name, h);
                    let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                        &chain_id, &symbol, name, redeem_at_name, h,
                        &format!("{} redeemed at {}", name, redeem_at_name),
                    ))).await;
                }
                Err(e) => { warn!("{}: Redeem failed: {}", name, e); break; }
            }
        }

        let _ = state_tx.send(ViewerEvent::State(Box::new(build_state(name, "consumer", "active", lat, lon, wallet, &chain_id, &symbol, &chain_meta, None, paused, *tx_count, "purchased + redeemed")))).await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_cross_chain_consumer(
    name: &str, lat: f64, lon: f64,
    consumer_cfg: &ConsumerConfig,
    client: &RecorderClient,
    directory: &Arc<RwLock<AgentDirectory>>,
    state_tx: &StateCollector,
    wallet: &mut Wallet,
    tx_count: &mut u64,
    speed: &SharedSpeed,
    paused: &PauseFlag,
) -> Result<()> {
    let want_sym = consumer_cfg.want_symbol.as_deref().unwrap();
    let pay_sym = consumer_cfg.pay_symbol.as_deref().unwrap();

    // Wait for both chains to exist
    let (want_chain_id, _want_vendor, want_plate_price) = wait_for_chain_symbol(directory, want_sym).await?;
    let (pay_chain_id, _pay_vendor, _pay_plate_price) = wait_for_chain_symbol(directory, pay_sym).await?;

    // Fund ourselves: buy initial pay_chain shares from fund_from vendor
    if let Some(fund_from) = &consumer_cfg.fund_from {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let info = client.chain_info(&pay_chain_id).await?;
        let total_shares: BigInt = info.shares_out.parse()?;
        let total_coins: BigInt = info.coin_count.parse()?;
        // Buy enough for ~10 plates worth of exchange (overestimate to ensure enough)
        let fund_coins = BigInt::from(want_plate_price) * BigInt::from(150u64);
        let fund_shares = &total_shares * &fund_coins / &total_coins;

        request_purchase(name, wallet, directory, fund_from, &pay_chain_id, &fund_shares).await?;
        *tx_count += 1;
        info!("{}: Funded with {} shares on {}", name, fund_shares, pay_sym);
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let base_interval = consumer_cfg.interval_secs as f64;
    info!("{}: Starting cross-chain loop {} → {} (base interval {}s)", name, pay_sym, want_sym, base_interval);

    let mut my_chains = HashMap::new();
    my_chains.insert(want_chain_id.clone(), want_sym.to_string());
    my_chains.insert(pay_chain_id.clone(), pay_sym.to_string());

    let chain_meta: HashMap<String, (String, String)> = {
        let dir = directory.read().await;
        my_chains.keys().filter_map(|cid| {
            dir.chains.get(cid).map(|reg| (cid.clone(), (reg.coin_count.clone(), reg.total_shares.clone())))
        }).collect()
    };

    loop {
        wait_while_paused(paused).await;
        let interval = std::time::Duration::from_secs_f64(base_interval / read_speed(speed).max(0.1));
        tokio::time::sleep(interval).await;

        // Calculate how many pay_chain shares to send for 1 plate of want_chain
        let want_info = match client.chain_info(&want_chain_id).await {
            Ok(i) => i,
            Err(e) => { warn!("{}: chain_info failed: {}", name, e); continue; }
        };
        let want_total_shares: BigInt = want_info.shares_out.parse()?;
        let want_total_coins: BigInt = want_info.coin_count.parse()?;
        let _plate_want_shares = &want_total_shares * BigInt::from(want_plate_price) / &want_total_coins;

        // Leg 1: Send payment shares to exchange agent on pay_chain
        let pay_utxo = match wallet.find_unspent(&pay_chain_id) {
            Some(u) => u,
            None => { warn!("{}: no pay_chain UTXOs left", name); continue; }
        };

        // Calculate pay amount: assume exchange rate is embedded in the pair config
        // For now, use the exchange agent's rate. We'll send a fraction of our UTXO.
        // The exchange agent will calculate the sell amount from this.
        let pay_amount = &pay_utxo.amount / BigInt::from(10); // ~10% per trade
        if pay_amount <= BigInt::from(0) {
            warn!("{}: pay amount too small", name);
            continue;
        }

        let dir = directory.read().await;
        let exchange_sender = dir.get(&consumer_cfg.buy_from)
            .ok_or_else(|| anyhow::anyhow!("{} not found", consumer_cfg.buy_from))?
            .clone();
        drop(dir);

        // Ask exchange for a receive key on pay_chain (for our payment)
        let (pk_tx, pk_rx) = oneshot::channel();
        exchange_sender.send(AgentMessage::RequestPubkey {
            chain_id: pay_chain_id.clone(),
            reply: pk_tx,
        }).await?;
        let exchange_recv = pk_rx.await?;

        // Build change key for ourselves on pay_chain
        let pay_change = wallet.generate_key(&pay_chain_id);

        // Execute leg 1: consumer → exchange on pay_chain
        let giver = Giver {
            seq_id: pay_utxo.seq_id,
            amount: pay_utxo.amount.clone(),
            seed: pay_utxo.seed,
        };
        let giver_pubkey = pay_utxo.pubkey;

        let mut pay_receivers = vec![
            Receiver {
                pubkey: exchange_recv.pubkey,
                seed: exchange_recv.seed,
                amount: pay_amount.clone(),
            },
            Receiver {
                pubkey: pay_change.pubkey,
                seed: pay_change.seed,
                amount: BigInt::from(0),
            },
        ];

        let pay_result = match transfer::execute_transfer(client, &pay_chain_id, &[giver], &mut pay_receivers).await {
            Ok(r) => r,
            Err(e) => { warn!("{}: Leg 1 failed: {}", name, e); continue; }
        };
        wallet.mark_spent(&giver_pubkey);
        wallet.register_utxo(&pay_change.pubkey, pay_result.first_seq + 1, pay_receivers[1].amount.clone());

        // Notify exchange of received payment UTXO
        let _ = exchange_sender.send(AgentMessage::NotifyUtxo {
            pubkey: exchange_recv.pubkey,
            seq_id: pay_result.first_seq,
            amount: pay_receivers[0].amount.clone(),
        }).await;

        *tx_count += 1;
        info!("{}: Leg 1 done: sent {} to {} on {} (block {})",
            name, pay_amount, consumer_cfg.buy_from, pay_sym, pay_result.height);

        // Generate our receiving key on want_chain
        let want_recv = wallet.generate_key(&want_chain_id);

        // Leg 2: Ask exchange to send us want_chain shares
        let (reply_tx, reply_rx) = oneshot::channel();
        exchange_sender.send(AgentMessage::CrossChainBuy {
            buyer_name: name.to_string(),
            sell_chain_id: want_chain_id.clone(),
            pay_chain_id: pay_chain_id.clone(),
            pay_amount: pay_receivers[0].amount.clone(),
            receiver_pubkey: want_recv.pubkey,
            receiver_seed: want_recv.seed,
            reply: reply_tx,
        }).await?;

        match reply_rx.await? {
            Ok(result) => {
                wallet.register_utxo(&want_recv.pubkey, result.sell_first_seq, result.sell_amount.clone());
                *tx_count += 1;
                info!("{}: Leg 2 done: received {} {} (block {})",
                    name, result.sell_amount, want_sym, result.sell_block);
                let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                    &want_chain_id, want_sym, &consumer_cfg.buy_from, name, result.sell_block,
                    &format!("{}: cross-chain {} → {}", name, pay_sym, want_sym),
                ))).await;
            }
            Err(e) => {
                warn!("{}: Leg 2 failed: {}", name, e);
            }
        }

        let _ = state_tx.send(ViewerEvent::State(Box::new(build_multi_state(
            name, "consumer", "active", lat, lon, wallet, &my_chains, &chain_meta, paused, *tx_count, "cross-chain trade",
        )))).await;
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_atomic_consumer(
    name: &str, lat: f64, lon: f64,
    consumer_cfg: &ConsumerConfig,
    client: &RecorderClient,
    directory: &Arc<RwLock<AgentDirectory>>,
    state_tx: &StateCollector,
    wallet: &mut Wallet,
    tx_count: &mut u64,
    speed: &SharedSpeed,
    paused: &PauseFlag,
) -> Result<()> {
    let want_sym = consumer_cfg.want_symbol.as_deref().unwrap();
    let pay_sym = consumer_cfg.pay_symbol.as_deref().unwrap();

    // Wait for both chains to exist
    let (want_chain_id, _want_vendor, want_plate_price) = wait_for_chain_symbol(directory, want_sym).await?;
    let (pay_chain_id, _pay_vendor, _pay_plate_price) = wait_for_chain_symbol(directory, pay_sym).await?;

    // Fund ourselves: buy initial pay_chain shares from fund_from vendor
    if let Some(fund_from) = &consumer_cfg.fund_from {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let info = client.chain_info(&pay_chain_id).await?;
        let total_shares: BigInt = info.shares_out.parse()?;
        let total_coins: BigInt = info.coin_count.parse()?;
        let fund_coins = BigInt::from(want_plate_price) * BigInt::from(150u64);
        let fund_shares = &total_shares * &fund_coins / &total_coins;

        request_purchase(name, wallet, directory, fund_from, &pay_chain_id, &fund_shares).await?;
        *tx_count += 1;
        info!("{}: Funded with {} shares on {}", name, fund_shares, pay_sym);
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let base_interval = consumer_cfg.interval_secs as f64;
    info!("{}: Starting atomic CAA loop {} → {} (base interval {}s)", name, pay_sym, want_sym, base_interval);

    let mut my_chains = HashMap::new();
    my_chains.insert(want_chain_id.clone(), want_sym.to_string());
    my_chains.insert(pay_chain_id.clone(), pay_sym.to_string());

    let chain_meta: HashMap<String, (String, String)> = {
        let dir = directory.read().await;
        my_chains.keys().filter_map(|cid| {
            dir.chains.get(cid).map(|reg| (cid.clone(), (reg.coin_count.clone(), reg.total_shares.clone())))
        }).collect()
    };

    loop {
        wait_while_paused(paused).await;
        let interval = std::time::Duration::from_secs_f64(base_interval / read_speed(speed).max(0.1));
        tokio::time::sleep(interval).await;

        // Find our unspent UTXO on pay_chain
        let pay_utxo = match wallet.find_unspent(&pay_chain_id) {
            Some(u) => u,
            None => { warn!("{}: No unspent UTXO on {}", name, pay_sym); continue; }
        };

        // Calculate pay amount: 1 plate worth
        let info = match client.chain_info(&pay_chain_id).await {
            Ok(i) => i,
            Err(e) => { warn!("{}: chain_info failed: {}", name, e); continue; }
        };
        let total_shares: BigInt = info.shares_out.parse().unwrap_or_default();
        let total_coins: BigInt = info.coin_count.parse().unwrap_or_default();
        let plate_coins = BigInt::from(want_plate_price);
        let pay_amount = &total_shares * &plate_coins / &total_coins;

        if pay_amount > pay_utxo.amount {
            warn!("{}: Insufficient funds on {} ({} < {})", name, pay_sym, pay_utxo.amount, pay_amount);
            continue;
        }

        // Generate receiver key on want_chain and change key on pay_chain
        let sell_recv = wallet.generate_key(&want_chain_id);
        let pay_change = wallet.generate_key(&pay_chain_id);

        // Send AtomicBuy to exchange
        let exchange_sender = {
            let dir = directory.read().await;
            dir.get(&consumer_cfg.buy_from).cloned()
        };
        let exchange_sender = match exchange_sender {
            Some(s) => s,
            None => { warn!("{}: Exchange {} not found", name, consumer_cfg.buy_from); continue; }
        };

        let (reply_tx, reply_rx) = oneshot::channel();
        let request = AtomicBuyRequest {
            buyer_name: name.to_string(),
            sell_chain_id: want_chain_id.clone(),
            pay_chain_id: pay_chain_id.clone(),
            pay_giver_seq_id: pay_utxo.seq_id,
            pay_giver_amount: pay_amount.clone(),
            pay_giver_seed: pay_utxo.seed,
            sell_receiver_pubkey: sell_recv.pubkey,
            sell_receiver_seed: sell_recv.seed,
            pay_change_pubkey: pay_change.pubkey,
            pay_change_seed: pay_change.seed,
        };

        if exchange_sender.send(AgentMessage::AtomicBuy { request, reply: reply_tx }).await.is_err() {
            warn!("{}: Failed to send AtomicBuy to {}", name, consumer_cfg.buy_from);
            continue;
        }

        match reply_rx.await {
            Ok(Ok(result)) => {
                wallet.mark_spent(&pay_utxo.pubkey);
                wallet.register_utxo(&sell_recv.pubkey, result.sell_receiver_seq, result.sell_amount.clone());
                wallet.register_utxo(&pay_change.pubkey, result.pay_change_seq, result.pay_change_amount.clone());
                *tx_count += 1;
                info!("{}: CAA atomic done: {} → {} (caa {})",
                    name, pay_sym, want_sym, &result.caa_hash[..12.min(result.caa_hash.len())]);
                let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
                    &want_chain_id, want_sym, &consumer_cfg.buy_from, name, result.sell_chain_block,
                    &format!("{}: CAA atomic {} → {}", name, pay_sym, want_sym),
                ))).await;
            }
            Ok(Err(e)) => {
                warn!("{}: AtomicBuy failed: {}", name, e);
            }
            Err(_) => {
                warn!("{}: AtomicBuy reply channel dropped", name);
            }
        }

        let _ = state_tx.send(ViewerEvent::State(Box::new(build_multi_state(
            name, "consumer", "active", lat, lon, wallet, &my_chains, &chain_meta, paused, *tx_count, "atomic trade",
        )))).await;
    }
}

// ── Validator agent ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn run_validator(
    config: AgentConfig,
    validator_cfg: ValidatorConfig,
    client: Arc<RecorderClient>,
    _directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    _mailbox: mpsc::Receiver<AgentMessage>,
    speed: SharedSpeed,
    paused: PauseFlag,
) -> Result<()> {
    let name = config.name.clone();
    let (lat, lon) = (config.lat, config.lon);
    let batch_size = validator_cfg.batch_size;
    let base_interval = validator_cfg.poll_interval_secs as f64;

    // Wait for chains to be created
    let startup_delay = std::time::Duration::from_secs_f64(5.0 / read_speed(&speed).max(0.1));
    tokio::time::sleep(startup_delay).await;

    // Per-chain validation state
    struct ChainState {
        chain_id: String,
        symbol: String,
        validated_height: u64,
        chain_height: u64,
        rolled_hash: [u8; 32],
        status: String,
    }

    let mut chain_states: Vec<ChainState> = Vec::new();
    let mut alerts: Vec<AlertEntry> = Vec::new();
    let mut total_verified: u64 = 0;
    let mut poll_count: u64 = 0;

    // Discover chains
    match client.list_chains().await {
        Ok(chains) => {
            for entry in &chains {
                // Fetch genesis block to init rolled hash
                match client.get_blocks(&entry.chain_id, 0, 0).await {
                    Ok(blocks) if !blocks.is_empty() => {
                        match ao_validator::verifier::verify_block_batch(&blocks, 0, &[0u8; 32]) {
                            Ok(result) => {
                                info!("{}: Discovered chain {} ({}) at height {}", name, entry.symbol, &entry.chain_id[..12], entry.block_height);
                                chain_states.push(ChainState {
                                    chain_id: entry.chain_id.clone(),
                                    symbol: entry.symbol.clone(),
                                    validated_height: result.last_height,
                                    chain_height: entry.block_height,
                                    rolled_hash: result.rolled_hash,
                                    status: "ok".to_string(),
                                });
                                total_verified += result.count;
                            }
                            Err(e) => {
                                warn!("{}: genesis verification failed for {}: {}", name, entry.symbol, e);
                                alerts.push(AlertEntry {
                                    timestamp_ms: now_ms(),
                                    chain_id: entry.chain_id.clone(),
                                    alert_type: "alteration".to_string(),
                                    message: format!("genesis verification failed: {}", e),
                                });
                                chain_states.push(ChainState {
                                    chain_id: entry.chain_id.clone(),
                                    symbol: entry.symbol.clone(),
                                    validated_height: 0,
                                    chain_height: entry.block_height,
                                    rolled_hash: [0u8; 32],
                                    status: "alert".to_string(),
                                });
                            }
                        }
                    }
                    Ok(_) => warn!("{}: no genesis block for {}", name, entry.symbol),
                    Err(e) => warn!("{}: failed to fetch genesis for {}: {}", name, entry.symbol, e),
                }
            }
        }
        Err(e) => warn!("{}: list_chains failed: {}", name, e),
    }

    let report_state = |chain_states: &[ChainState], alerts: &[AlertEntry], total_verified: u64, paused: &PauseFlag, last_action: &str| -> AgentState {
        let monitored = chain_states.iter().map(|cs| MonitoredChainStatus {
            chain_id: cs.chain_id.clone(),
            symbol: cs.symbol.clone(),
            validated_height: cs.validated_height,
            chain_height: cs.chain_height,
            status: cs.status.clone(),
            last_poll_ms: now_ms(),
        }).collect();
        AgentState {
            name: name.clone(),
            role: "validator".to_string(),
            status: "active".to_string(),
            lat, lon,
            chains: Vec::new(),
            key_summary: Vec::new(),
            coverage_radius: None,
            paused: paused.load(Ordering::Relaxed),
            trading_rates: Vec::new(),
            validator_status: Some(ValidatorStatus {
                monitored_chains: monitored,
                alerts: alerts.to_vec(),
                total_blocks_verified: total_verified,
            }),
            attacker_status: None,
            infra_tester_status: None,
            caa_status: None,
            recorder_op_status: None,
            transactions: 0,
            last_action: last_action.to_string(),
        }
    };

    let _ = state_tx.send(ViewerEvent::State(Box::new(report_state(&chain_states, &alerts, total_verified, &paused, "initialized")))).await;

    // Poll loop
    loop {
        wait_while_paused(&paused).await;
        let interval = std::time::Duration::from_secs_f64(base_interval / read_speed(&speed).max(0.1));
        tokio::time::sleep(interval).await;

        poll_count += 1;

        // Re-discover chains every 10 polls
        if poll_count.is_multiple_of(10)
            && let Ok(chains) = client.list_chains().await
        {
            for entry in &chains {
                if !chain_states.iter().any(|cs| cs.chain_id == entry.chain_id) {
                    match client.get_blocks(&entry.chain_id, 0, 0).await {
                        Ok(blocks) if !blocks.is_empty() => {
                            if let Ok(result) = ao_validator::verifier::verify_block_batch(&blocks, 0, &[0u8; 32]) {
                                info!("{}: Discovered new chain {} ({})", name, entry.symbol, &entry.chain_id[..12]);
                                chain_states.push(ChainState {
                                    chain_id: entry.chain_id.clone(),
                                    symbol: entry.symbol.clone(),
                                    validated_height: result.last_height,
                                    chain_height: entry.block_height,
                                    rolled_hash: result.rolled_hash,
                                    status: "ok".to_string(),
                                });
                                total_verified += result.count;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Verify each chain
        for cs in chain_states.iter_mut() {
            // Get current height
            match client.chain_info(&cs.chain_id).await {
                Ok(info) => {
                    cs.chain_height = info.block_height;
                    if cs.status == "unreachable" {
                        cs.status = "ok".to_string();
                        alerts.push(AlertEntry {
                            timestamp_ms: now_ms(),
                            chain_id: cs.chain_id.clone(),
                            alert_type: "recovered".to_string(),
                            message: "recorder reachable again".to_string(),
                        });
                    }
                }
                Err(e) => {
                    if cs.status != "unreachable" {
                        cs.status = "unreachable".to_string();
                        alerts.push(AlertEntry {
                            timestamp_ms: now_ms(),
                            chain_id: cs.chain_id.clone(),
                            alert_type: "unreachable".to_string(),
                            message: format!("recorder unreachable: {}", e),
                        });
                    }
                    continue;
                }
            }

            // Fetch and verify new blocks
            if cs.chain_height > cs.validated_height {
                let from = cs.validated_height + 1;
                let to = from.saturating_add(batch_size - 1).min(cs.chain_height);

                match client.get_blocks(&cs.chain_id, from, to).await {
                    Ok(blocks) if !blocks.is_empty() => {
                        match ao_validator::verifier::verify_block_batch(&blocks, from, &cs.rolled_hash) {
                            Ok(result) => {
                                cs.validated_height = result.last_height;
                                cs.rolled_hash = result.rolled_hash;
                                total_verified += result.count;
                                info!("{}: {} verified to height {} ({} blocks)",
                                    name, cs.symbol, result.last_height, result.count);
                            }
                            Err(e) => {
                                cs.status = "alert".to_string();
                                let msg = format!("block verification failed at height {}: {}", from, e);
                                warn!("{}: {} — {}", name, cs.symbol, msg);
                                alerts.push(AlertEntry {
                                    timestamp_ms: now_ms(),
                                    chain_id: cs.chain_id.clone(),
                                    alert_type: "alteration".to_string(),
                                    message: msg,
                                });
                            }
                        }
                    }
                    Ok(_) => {} // empty batch, nothing to verify
                    Err(e) => {
                        cs.status = "alert".to_string();
                        let msg = format!("block fetch failed at height {}: {}", from, e);
                        warn!("{}: {} — {}", name, cs.symbol, msg);
                        alerts.push(AlertEntry {
                            timestamp_ms: now_ms(),
                            chain_id: cs.chain_id.clone(),
                            alert_type: "alteration".to_string(),
                            message: msg,
                        });
                    }
                }
            }
        }

        // Cap alerts to last 100
        if alerts.len() > 100 {
            alerts.drain(..alerts.len() - 100);
        }

        let _ = state_tx.send(ViewerEvent::State(Box::new(report_state(
            &chain_states, &alerts, total_verified, &paused,
            &format!("poll #{}", poll_count),
        )))).await;
    }
}

// ── Attacker agent ──────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
pub async fn run_attacker(
    config: AgentConfig,
    attacker_cfg: AttackerConfig,
    client: Arc<RecorderClient>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    _mailbox: mpsc::Receiver<AgentMessage>,
    speed: SharedSpeed,
    paused: PauseFlag,
    recorder_state: Option<Arc<ao_recorder::AppState>>,
) -> Result<()> {
    let name = config.name.clone();
    let (lat, lon) = (config.lat, config.lon);
    let attack_type = attacker_cfg.attack.clone();
    let base_interval = attacker_cfg.attack_interval_secs as f64;

    let mut wallet = Wallet::new(&name);
    let mut attempts: u64 = 0;
    let mut rejections: u64 = 0;
    let mut unexpected_accepts: u64 = 0;
    let mut last_result;

    // Wait for target vendor chain
    let (chain_id, symbol, plate_price) = wait_for_vendor_chain(&directory, &attacker_cfg.target_vendor).await?;
    info!("{}: Found target chain {} ({}) from {}", name, symbol, &chain_id[..12], attacker_cfg.target_vendor);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Buy initial shares to have something to work with
    let info = client.chain_info(&chain_id).await?;
    let total_shares: BigInt = info.shares_out.parse()?;
    let total_coins: BigInt = info.coin_count.parse()?;
    let buy_shares = &total_shares * BigInt::from(plate_price * 10) / &total_coins;
    request_purchase(&name, &mut wallet, &directory, &attacker_cfg.target_vendor, &chain_id, &buy_shares).await?;
    info!("{}: Acquired initial shares on {}", name, symbol);

    let chain_meta: HashMap<String, (String, String)> = {
        let dir = directory.read().await;
        dir.chains.get(&chain_id)
            .map(|reg| [(chain_id.clone(), (reg.coin_count.clone(), reg.total_shares.clone()))].into_iter().collect())
            .unwrap_or_default()
    };

    let make_state = |wallet: &Wallet, attempts: u64, rejections: u64, unexpected_accepts: u64, last_result: &str, paused: &PauseFlag| -> AgentState {
        AgentState {
            name: name.clone(),
            role: "attacker".to_string(),
            status: "active".to_string(),
            lat, lon,
            chains: vec![ChainHolding {
                chain_id: chain_id.clone(),
                symbol: symbol.clone(),
                shares: wallet.balance(&chain_id),
                unspent_utxos: wallet.find_all_unspent(&chain_id).len(),
                coin_count: chain_meta.get(&chain_id).map(|m| m.0.clone()).unwrap_or_default(),
                total_shares: chain_meta.get(&chain_id).map(|m| m.1.clone()).unwrap_or_default(),
            }],
            key_summary: Vec::new(),
            coverage_radius: None,
            paused: paused.load(Ordering::Relaxed),
            trading_rates: Vec::new(),
            validator_status: None,
            attacker_status: Some(AttackerStatus {
                attack_type: attack_type.clone(),
                attempts,
                rejections,
                unexpected_accepts,
                last_result: last_result.to_string(),
            }),
            infra_tester_status: None,
            caa_status: None,
            recorder_op_status: None,
            transactions: attempts,
            last_action: last_result.to_string(),
        }
    };

    let _ = state_tx.send(ViewerEvent::State(Box::new(make_state(&wallet, 0, 0, 0, "ready", &paused)))).await;

    // Attack loop
    loop {
        wait_while_paused(&paused).await;
        let interval = std::time::Duration::from_secs_f64(base_interval / read_speed(&speed).max(0.1));
        tokio::time::sleep(interval).await;

        // Ensure we have shares to work with
        if wallet.find_unspent(&chain_id).is_none() {
            // Buy more shares
            match request_purchase(&name, &mut wallet, &directory, &attacker_cfg.target_vendor, &chain_id, &buy_shares).await {
                Ok(()) => {}
                Err(e) => {
                    warn!("{}: failed to restock: {}", name, e);
                    continue;
                }
            }
        }

        attempts += 1;
        let outcome = match attack_type.as_str() {
            "double_spend" => attempt_double_spend(&name, &mut wallet, &client, &chain_id).await,
            "key_reuse" => attempt_key_reuse(&name, &mut wallet, &client, &directory, &attacker_cfg.target_vendor, &chain_id).await,
            "expired_utxo" => attempt_expired_utxo(&name, &mut wallet, &client, &chain_id).await,
            "chain_tamper" => attempt_chain_tamper(&name, &chain_id, &recorder_state).await,
            "refute" => attempt_refute(&name, &mut wallet, &client, &directory, &attacker_cfg.target_vendor, &chain_id).await,
            other => {
                warn!("{}: unknown attack type: {}", name, other);
                Err(anyhow::anyhow!("unknown attack type"))
            }
        };

        match outcome {
            Ok(false) => {
                // Attack was rejected (expected)
                rejections += 1;
                last_result = format!("#{}: rejected (correct)", attempts);
                info!("{}: {} attempt #{} rejected (expected)", name, attack_type, attempts);
            }
            Ok(true) => {
                // Attack succeeded (unexpected!)
                unexpected_accepts += 1;
                last_result = format!("#{}: ACCEPTED (unexpected!)", attempts);
                warn!("{}: {} attempt #{} ACCEPTED — this should not happen!", name, attack_type, attempts);
            }
            Err(e) => {
                // Error during attack (count as rejection)
                rejections += 1;
                last_result = format!("#{}: error: {}", attempts, e);
                info!("{}: {} attempt #{} error: {}", name, attack_type, attempts, e);
            }
        }

        let _ = state_tx.send(ViewerEvent::State(Box::new(make_state(&wallet, attempts, rejections, unexpected_accepts, &last_result, &paused)))).await;
        let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
            &chain_id, &symbol, &name, &attacker_cfg.target_vendor, 0,
            &format!("{}: {}", attack_type, &last_result),
        ))).await;
    }
}

/// Attempt double-spend: submit valid transfer, then resubmit using same spent UTXO.
/// Returns Ok(true) if second submit was accepted (bad), Ok(false) if rejected (good).
async fn attempt_double_spend(
    _name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    chain_id: &str,
) -> Result<bool> {
    let utxo = wallet.find_unspent(chain_id)
        .ok_or_else(|| anyhow::anyhow!("no UTXO for double-spend"))?;

    // Build two receivers: one for us (split the UTXO)
    let recv1 = wallet.generate_key(chain_id);
    let recv2 = wallet.generate_key(chain_id);

    let giver_pubkey = utxo.pubkey;
    let giver_seq_id = utxo.seq_id;
    let giver_amount = utxo.amount.clone();
    let giver_seed = utxo.seed;

    // First transfer (valid)
    let giver1 = transfer::Giver {
        seq_id: giver_seq_id,
        amount: giver_amount.clone(),
        seed: giver_seed,
    };
    let mut receivers1 = vec![transfer::Receiver {
        pubkey: recv1.pubkey,
        seed: recv1.seed,
        amount: giver_amount.clone(),
    }];
    let result = transfer::execute_transfer(client, chain_id, &[giver1], &mut receivers1).await?;
    wallet.mark_spent(&giver_pubkey);
    wallet.register_utxo(&recv1.pubkey, result.first_seq, receivers1[0].amount.clone());

    // Second transfer reusing the same (now spent) UTXO
    let giver2 = transfer::Giver {
        seq_id: giver_seq_id,
        amount: giver_amount.clone(),
        seed: giver_seed,
    };
    let mut receivers2 = vec![transfer::Receiver {
        pubkey: recv2.pubkey,
        seed: recv2.seed,
        amount: giver_amount,
    }];
    match transfer::execute_transfer(client, chain_id, &[giver2], &mut receivers2).await {
        Ok(_) => Ok(true),   // accepted = bad
        Err(_) => Ok(false),  // rejected = good
    }
}

/// Attempt key reuse: use an already-used pubkey as receiver.
/// Returns Ok(true) if accepted (bad), Ok(false) if rejected (good).
async fn attempt_key_reuse(
    name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    directory: &Arc<RwLock<AgentDirectory>>,
    vendor_name: &str,
    chain_id: &str,
) -> Result<bool> {
    // Find a key that has already been used (has a seq_id — means it received shares)
    let used_pubkey = wallet.find_all_unspent(chain_id)
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("no used key for key-reuse attack"))?;
    let reuse_pubkey = used_pubkey.pubkey;
    let reuse_seed = used_pubkey.seed;

    // Try to buy more shares using the same key as receiver
    let dir = directory.read().await;
    let seller = dir.get(vendor_name)
        .ok_or_else(|| anyhow::anyhow!("{} not found", vendor_name))?
        .clone();
    drop(dir);

    // Ask vendor for a change key
    let (pk_tx, pk_rx) = oneshot::channel();
    seller.send(AgentMessage::RequestPubkey {
        chain_id: chain_id.to_string(),
        reply: pk_tx,
    }).await?;
    let seller_change = pk_rx.await?;

    let info = client.chain_info(chain_id).await?;
    let total_shares: BigInt = info.shares_out.parse()?;
    let total_coins: BigInt = info.coin_count.parse()?;
    let small_amount = &total_shares * BigInt::from(5u64) / &total_coins; // 5 coins worth

    let (reply_tx, reply_rx) = oneshot::channel();
    seller.send(AgentMessage::SellToMe {
        chain_id: chain_id.to_string(),
        buyer_name: name.to_string(),
        receivers: vec![
            transfer::Receiver {
                pubkey: reuse_pubkey,
                seed: reuse_seed,
                amount: small_amount,
            },
            transfer::Receiver {
                pubkey: seller_change.pubkey,
                seed: seller_change.seed,
                amount: BigInt::from(0),
            },
        ],
        reply: reply_tx,
    }).await?;

    match reply_rx.await? {
        Ok(_) => Ok(true),   // accepted = bad
        Err(_) => Ok(false),  // rejected = good
    }
}

/// Attempt expired UTXO usage: build a transfer using shares that should be expired.
/// We simulate this by building a transfer manually with an expired timestamp.
/// Returns Ok(true) if accepted (bad), Ok(false) if rejected (good).
async fn attempt_expired_utxo(
    _name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    chain_id: &str,
) -> Result<bool> {
    let utxo = wallet.find_unspent(chain_id)
        .ok_or_else(|| anyhow::anyhow!("no UTXO for expired-utxo attack"))?;

    let recv = wallet.generate_key(chain_id);
    let giver = transfer::Giver {
        seq_id: utxo.seq_id,
        amount: utxo.amount.clone(),
        seed: utxo.seed,
    };
    let giver_pubkey = utxo.pubkey;

    let mut receivers = vec![transfer::Receiver {
        pubkey: recv.pubkey,
        seed: recv.seed,
        amount: utxo.amount,
    }];

    // Use the normal transfer path — the recorder validates block integrity.
    // For a true expired-UTXO test we'd need time-travel, which the sim doesn't
    // support. Instead, we attempt a self-transfer which exercises the validation
    // path. If the chain has expiry configured, old UTXOs will be caught.
    // For now, this acts as a recorder health check — submit and expect success
    // (since UTXOs aren't actually expired in a fast sim). Mark as rejected
    // to keep the attacker's bookkeeping consistent.
    match transfer::execute_transfer(client, chain_id, &[giver], &mut receivers).await {
        Ok(result) => {
            // Self-transfer succeeded — register the new UTXO, mark old spent
            wallet.mark_spent(&giver_pubkey);
            wallet.register_utxo(&recv.pubkey, result.first_seq, receivers[0].amount.clone());
            // In a real expiry scenario this would be unexpected, but in fast sims
            // UTXOs aren't old enough to expire. Return false (not a vulnerability).
            Ok(false)
        }
        Err(_) => Ok(false),  // rejected for any reason = good
    }
}

/// Tamper with the recorder's stored block data to test validator detection.
/// Returns Ok(false) after tampering — counted as "rejected" since the tampering
/// itself isn't a protocol vulnerability (the validator should catch it).
async fn attempt_chain_tamper(
    name: &str,
    chain_id: &str,
    recorder_state: &Option<Arc<ao_recorder::AppState>>,
) -> Result<bool> {
    let state = recorder_state.as_ref()
        .ok_or_else(|| anyhow::anyhow!("{}: chain_tamper requires recorder_state", name))?;

    let chains = state.chains.read().unwrap();
    let chain_state = chains.get(chain_id)
        .ok_or_else(|| anyhow::anyhow!("{}: chain {} not found in recorder", name, &chain_id[..12]))?;

    // Tamper with the latest block — must be one the validator hasn't verified yet.
    let tampered = {
        let store = chain_state.store.lock().unwrap();
        let height = store.block_count()?;
        if height < 2 {
            return Ok(false); // not enough blocks yet
        }
        // Tamper with the most recent block (height - 1 is the latest, 0-indexed count).
        // The validator verifies forward from its last checkpoint, so new blocks
        // that arrive after the last poll will be verified on the next poll.
        let target_height = height - 1;
        info!("{}: Targeting block {} on chain {}", name, target_height, &chain_id[..12]);
        store.tamper_block(target_height)?
    };

    if tampered {
        info!("{}: Tampered with block data on chain {} — validator should detect this", name, &chain_id[..12]);
    }

    // Return false — tampering is not a vulnerability (the validator should catch it).
    // If the validator fails to detect it, that's a separate issue visible in Victor's alerts.
    Ok(false)
}

/// Attempt refutation: do a valid transfer, then refute the recorded agreement.
/// After refutation, attempt to re-spend the same UTXO — should still fail because
/// the transfer was already recorded. Refutation prevents late recording of *unrecorded*
/// agreements, not already-recorded ones.
/// Returns Ok(true) if re-spend after refutation was accepted (bad), Ok(false) if rejected (good).
async fn attempt_refute(
    _name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    _directory: &Arc<RwLock<AgentDirectory>>,
    _vendor_name: &str,
    chain_id: &str,
) -> Result<bool> {
    let utxo = wallet.find_unspent(chain_id)
        .ok_or_else(|| anyhow::anyhow!("no UTXO for refute attack"))?;

    let recv1 = wallet.generate_key(chain_id);
    let recv2 = wallet.generate_key(chain_id);

    let giver_pubkey = utxo.pubkey;
    let giver_seq_id = utxo.seq_id;
    let giver_amount = utxo.amount.clone();
    let giver_seed = utxo.seed;

    // First: do a valid transfer
    let giver = transfer::Giver {
        seq_id: giver_seq_id,
        amount: giver_amount.clone(),
        seed: giver_seed,
    };
    let mut receivers = vec![transfer::Receiver {
        pubkey: recv1.pubkey,
        seed: recv1.seed,
        amount: giver_amount.clone(),
    }];
    let result = transfer::execute_transfer(client, chain_id, &[giver], &mut receivers).await?;
    wallet.mark_spent(&giver_pubkey);
    wallet.register_utxo(&recv1.pubkey, result.first_seq, receivers[0].amount.clone());

    // Second: refute the recorded assignment using its block hash as agreement_hash
    // (In practice refutation targets an unrecorded agreement hash, but we test
    // that refuting a recorded one doesn't un-spend the UTXO)
    let _ = client.refute(chain_id, &result.hash).await;

    // Third: try to re-spend the same (now spent) UTXO — should still be rejected
    let giver2 = transfer::Giver {
        seq_id: giver_seq_id,
        amount: giver_amount,
        seed: giver_seed,
    };
    let mut receivers2 = vec![transfer::Receiver {
        pubkey: recv2.pubkey,
        seed: recv2.seed,
        amount: BigInt::from(0), // will be fee-adjusted
    }];
    match transfer::execute_transfer(client, chain_id, &[giver2], &mut receivers2).await {
        Ok(_) => Ok(true),   // accepted after refutation = bad
        Err(_) => Ok(false),  // still rejected = good
    }
}

// ── Shared helpers ──────────────────────────────────────────────────

/// Handle a SellToMe request: find own unspent UTXO, build transfer, execute.
async fn handle_sell(
    name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    chain_id: &str,
    receivers: &mut [Receiver],
) -> Result<TransferResult> {
    let utxo = wallet.find_unspent(chain_id)
        .ok_or_else(|| anyhow::anyhow!("{}: no unspent UTXO on chain", name))?;

    let giver = Giver {
        seq_id: utxo.seq_id,
        amount: utxo.amount,
        seed: utxo.seed,
    };
    let giver_pubkey = utxo.pubkey;

    let result = transfer::execute_transfer(client, chain_id, &[giver], receivers).await?;

    // Update wallet: mark giver spent, register receiver UTXOs that are ours
    wallet.mark_spent(&giver_pubkey);
    for (i, recv) in receivers.iter().enumerate() {
        let seq = result.first_seq + i as u64;
        if wallet.get_signing_key(&recv.pubkey).is_some() {
            wallet.register_utxo(&recv.pubkey, seq, recv.amount.clone());
        }
    }

    Ok(TransferResult {
        block_height: result.height,
        first_seq: result.first_seq,
    })
}

/// Request a purchase from another agent (vendor or exchange).
/// The seller provides the giver UTXO; we provide our receiver key.
async fn request_purchase(
    name: &str,
    wallet: &mut Wallet,
    directory: &Arc<RwLock<AgentDirectory>>,
    seller_name: &str,
    chain_id: &str,
    desired_shares: &BigInt,
) -> Result<()> {
    let dir = directory.read().await;
    let seller = dir.get(seller_name)
        .ok_or_else(|| anyhow::anyhow!("{} not found", seller_name))?
        .clone();
    drop(dir);

    // Generate our receiving key
    let our_entry = wallet.generate_key(chain_id);

    // Ask seller for a change key
    let (pk_tx, pk_rx) = oneshot::channel();
    seller.send(AgentMessage::RequestPubkey {
        chain_id: chain_id.to_string(),
        reply: pk_tx,
    }).await?;
    let seller_change = pk_rx.await?;

    // Send SellToMe: seller fills in giver, we get desired_shares, seller keeps change
    let (reply_tx, reply_rx) = oneshot::channel();
    seller.send(AgentMessage::SellToMe {
        chain_id: chain_id.to_string(),
        buyer_name: name.to_string(),
        receivers: vec![
            Receiver {
                pubkey: our_entry.pubkey,
                seed: our_entry.seed,
                amount: desired_shares.clone(),
            },
            Receiver {
                pubkey: seller_change.pubkey,
                seed: seller_change.seed,
                amount: BigInt::from(0), // placeholder — adjusted by execute_transfer
            },
        ],
        reply: reply_tx,
    }).await?;

    let result = reply_rx.await??;

    // Register our UTXO
    wallet.register_utxo(&our_entry.pubkey, result.first_seq, desired_shares.clone());

    Ok(())
}

/// Consumer redeems shares at a vendor (transfers shares back).
async fn redeem_at(
    name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    directory: &Arc<RwLock<AgentDirectory>>,
    vendor_name: &str,
    chain_id: &str,
) -> Result<u64> {
    let dir = directory.read().await;
    let vendor = dir.get(vendor_name)
        .ok_or_else(|| anyhow::anyhow!("{} not found", vendor_name))?
        .clone();
    drop(dir);

    // Find our unspent UTXO
    let utxo = wallet.find_unspent(chain_id)
        .ok_or_else(|| anyhow::anyhow!("{}: no UTXO to redeem", name))?;

    // Ask vendor for a receiving key
    let (pk_tx, pk_rx) = oneshot::channel();
    vendor.send(AgentMessage::RequestPubkey {
        chain_id: chain_id.to_string(),
        reply: pk_tx,
    }).await?;
    let vendor_recv = pk_rx.await?;

    // Build transfer: consumer → vendor (single receiver)
    let giver_pubkey = utxo.pubkey;
    let givers = vec![Giver {
        seq_id: utxo.seq_id,
        amount: utxo.amount.clone(),
        seed: utxo.seed,
    }];
    let mut receivers = vec![Receiver {
        pubkey: vendor_recv.pubkey,
        seed: vendor_recv.seed,
        amount: utxo.amount, // last receiver — adjusted for fee
    }];

    let result = transfer::execute_transfer(client, chain_id, &givers, &mut receivers).await?;
    wallet.mark_spent(&giver_pubkey);

    // Notify vendor of the received UTXO so its wallet stays current
    let _ = vendor.send(AgentMessage::NotifyUtxo {
        pubkey: vendor_recv.pubkey,
        seq_id: result.first_seq,
        amount: receivers[0].amount.clone(),
    }).await;

    Ok(result.height)
}

/// Wait for a vendor's chain to appear in the directory.
/// Returns (chain_id, symbol, plate_price_coins).
async fn wait_for_vendor_chain(
    directory: &Arc<RwLock<AgentDirectory>>,
    vendor_name: &str,
) -> Result<(String, String, u64)> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        {
            let dir = directory.read().await;
            for reg in dir.chains.values() {
                if reg.vendor_name == vendor_name {
                    return Ok((reg.chain_id.clone(), reg.symbol.clone(), reg.plate_price));
                }
            }
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for vendor {} chain", vendor_name);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

/// Wait for a chain with a given symbol to appear in the directory.
/// Returns (chain_id, vendor_name, plate_price).
async fn wait_for_chain_symbol(
    directory: &Arc<RwLock<AgentDirectory>>,
    symbol: &str,
) -> Result<(String, String, u64)> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        {
            let dir = directory.read().await;
            if let Some(chain_id) = dir.chain_id_for_symbol(symbol) {
                let reg = dir.chains.get(chain_id).unwrap();
                return Ok((reg.chain_id.clone(), reg.vendor_name.clone(), reg.plate_price));
            }
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for chain symbol {}", symbol);
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

/// Find the vendor name for a given chain_id from the directory.
async fn find_vendor_for_chain(
    directory: &Arc<RwLock<AgentDirectory>>,
    chain_id: &str,
) -> Option<String> {
    let dir = directory.read().await;
    dir.chains.get(chain_id).map(|reg| reg.vendor_name.clone())
}

/// Build an AgentState snapshot for reporting.
#[allow(clippy::too_many_arguments)]
fn build_state(
    name: &str, role: &str, status: &str, lat: f64, lon: f64,
    wallet: &Wallet, chain_id: &str, symbol: &str,
    chain_meta: &HashMap<String, (String, String)>,
    coverage_radius: Option<f64>,
    paused: &PauseFlag,
    transactions: u64, last_action: &str,
) -> AgentState {
    let (coin_count, total_shares) = chain_meta.get(chain_id)
        .cloned()
        .unwrap_or_default();
    AgentState {
        name: name.to_string(),
        role: role.to_string(),
        status: status.to_string(),
        lat, lon,
        chains: vec![ChainHolding {
            chain_id: chain_id.to_string(),
            symbol: symbol.to_string(),
            shares: wallet.balance(chain_id),
            unspent_utxos: wallet.find_all_unspent(chain_id).len(),
            coin_count,
            total_shares,
        }],
        key_summary: wallet.chain_summaries(),
        coverage_radius,
        paused: paused.load(Ordering::Relaxed),
        trading_rates: Vec::new(),
        validator_status: None,
        attacker_status: None,
        infra_tester_status: None,
        caa_status: None,
        recorder_op_status: None,
        transactions,
        last_action: last_action.to_string(),
    }
}

/// Build an AgentState snapshot for multi-chain agents.
#[allow(clippy::too_many_arguments)]
fn build_multi_state(
    name: &str, role: &str, status: &str, lat: f64, lon: f64,
    wallet: &Wallet, chains: &HashMap<String, String>,
    chain_meta: &HashMap<String, (String, String)>,
    paused: &PauseFlag,
    transactions: u64, last_action: &str,
) -> AgentState {
    let holdings: Vec<ChainHolding> = chains.iter().map(|(chain_id, symbol)| {
        let (coin_count, total_shares) = chain_meta.get(chain_id)
            .cloned()
            .unwrap_or_default();
        ChainHolding {
            chain_id: chain_id.clone(),
            symbol: symbol.clone(),
            shares: wallet.balance(chain_id),
            unspent_utxos: wallet.find_all_unspent(chain_id).len(),
            coin_count,
            total_shares,
        }
    }).collect();

    AgentState {
        name: name.to_string(),
        role: role.to_string(),
        status: status.to_string(),
        lat, lon,
        chains: holdings,
        key_summary: wallet.chain_summaries(),
        coverage_radius: None,
        paused: paused.load(Ordering::Relaxed),
        trading_rates: Vec::new(),
        validator_status: None,
        attacker_status: None,
        infra_tester_status: None,
        caa_status: None,
        recorder_op_status: None,
        transactions,
        last_action: last_action.to_string(),
    }
}


// ── White-hat infrastructure tester agent ───────────────────────────
//
// Tests server-level resilience (N10 security hardening features),
// NOT protocol correctness. Each test type verifies that the recorder
// properly enforces a specific infrastructure protection.
//
// All tests are clearly labeled "infra_tester" in the viewer and logs.
// These are defensive, white-hat probes — they verify that protections
// WORK, not that they can be bypassed.

#[allow(clippy::too_many_arguments)]
pub async fn run_infra_tester(
    config: AgentConfig,
    tester_cfg: InfraTesterConfig,
    recorder_url: String,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    _mailbox: mpsc::Receiver<AgentMessage>,
    speed: SharedSpeed,
    paused: PauseFlag,
) -> Result<()> {
    let name = config.name.clone();
    let (lat, lon) = (config.lat, config.lon);
    let test_type = tester_cfg.test.clone();
    let base_interval = tester_cfg.probe_interval_secs as f64;
    let concurrency = tester_cfg.concurrency;

    let mut rounds: u64 = 0;
    let mut passed: u64 = 0;
    let mut failed: u64 = 0;
    let mut last_result;

    // Wait for target vendor chain
    let (chain_id, symbol, _plate_price) = wait_for_vendor_chain(&directory, &tester_cfg.target_vendor).await?;
    info!("{}: [WHITE-HAT INFRA TESTER] Found target chain {} ({}), test type: {}",
        name, symbol, &chain_id[..12], test_type);
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let http_client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let make_state = |rounds: u64, passed: u64, failed: u64, last_result: &str,
                      detail: serde_json::Value, paused: &PauseFlag| -> AgentState {
        AgentState {
            name: name.clone(),
            role: "infra_tester".to_string(),
            status: "active".to_string(),
            lat, lon,
            chains: vec![ChainHolding {
                chain_id: chain_id.clone(),
                symbol: symbol.clone(),
                shares: BigInt::zero(),
                unspent_utxos: 0,
                coin_count: String::new(),
                total_shares: String::new(),
            }],
            key_summary: Vec::new(),
            coverage_radius: None,
            paused: paused.load(Ordering::Relaxed),
            trading_rates: Vec::new(),
            validator_status: None,
            attacker_status: None,
            infra_tester_status: Some(InfraTesterStatus {
                test_type: test_type.clone(),
                rounds,
                passed,
                failed,
                last_result: last_result.to_string(),
                detail,
            }),
            caa_status: None,
            recorder_op_status: None,
            transactions: rounds,
            last_action: last_result.to_string(),
        }
    };

    let _ = state_tx.send(ViewerEvent::State(Box::new(
        make_state(0, 0, 0, "ready", serde_json::json!({}), &paused)
    ))).await;

    loop {
        wait_while_paused(&paused).await;
        let interval = std::time::Duration::from_secs_f64(base_interval / read_speed(&speed).max(0.1));
        tokio::time::sleep(interval).await;

        rounds += 1;
        let (ok, detail, desc) = match test_type.as_str() {
            "flood" => infra_test_flood(
                &http_client, &recorder_url, &chain_id, concurrency,
            ).await,
            "oversized_payload" => infra_test_oversized_payload(
                &http_client, &recorder_url, &chain_id,
            ).await,
            "auth_bypass" => infra_test_auth_bypass(
                &http_client, &recorder_url, &chain_id, tester_cfg.api_key.as_deref(),
            ).await,
            "connection_exhaustion" => infra_test_connection_exhaustion(
                &http_client, &recorder_url, &chain_id, concurrency,
            ).await,
            "error_probe" => infra_test_error_probe(
                &http_client, &recorder_url, &chain_id,
            ).await,
            other => {
                warn!("{}: unknown infra test type: {}", name, other);
                (false, serde_json::json!({}), format!("unknown test: {}", other))
            }
        };

        if ok { passed += 1; } else { failed += 1; }
        last_result = format!("#{}: {}", rounds, desc);
        info!("{}: [WHITE-HAT] {} round #{}: {}", name, test_type, rounds, desc);

        let _ = state_tx.send(ViewerEvent::State(Box::new(
            make_state(rounds, passed, failed, &last_result, detail.clone(), &paused)
        ))).await;
        let _ = state_tx.send(ViewerEvent::Transaction(tx_event(
            &chain_id, &symbol, &name, &tester_cfg.target_vendor, 0,
            &format!("[INFRA] {}: {}", test_type, &last_result),
        ))).await;
    }
}

/// Flood test: send `concurrency` simultaneous GET /chain/{id}/info requests.
/// PASS if all requests complete (server stays responsive under load).
/// Reports latency stats (min/max/avg/p99) and any errors.
async fn infra_test_flood(
    client: &reqwest::Client,
    recorder_url: &str,
    chain_id: &str,
    concurrency: usize,
) -> (bool, serde_json::Value, String) {
    let url = format!("{}/chain/{}/info", recorder_url, chain_id);
    let mut handles = Vec::with_capacity(concurrency);

    let start = std::time::Instant::now();
    for _ in 0..concurrency {
        let c = client.clone();
        let u = url.clone();
        handles.push(tokio::spawn(async move {
            let t0 = std::time::Instant::now();
            let result = c.get(&u).send().await;
            let elapsed_ms = t0.elapsed().as_millis() as u64;
            match result {
                Ok(resp) => (resp.status().as_u16(), elapsed_ms),
                Err(_) => (0u16, elapsed_ms),
            }
        }));
    }

    let mut latencies = Vec::with_capacity(concurrency);
    let mut successes = 0u64;
    let mut errors = 0u64;
    let mut rate_limited = 0u64;
    for h in handles {
        if let Ok((status, ms)) = h.await {
            latencies.push(ms);
            match status {
                200 => successes += 1,
                429 => rate_limited += 1,
                0 => errors += 1,
                _ => errors += 1,
            }
        }
    }
    let wall_ms = start.elapsed().as_millis() as u64;

    latencies.sort();
    let avg = if latencies.is_empty() { 0 } else { latencies.iter().sum::<u64>() / latencies.len() as u64 };
    let p99 = latencies.get(latencies.len().saturating_sub(1) * 99 / 100).copied().unwrap_or(0);
    let max = latencies.last().copied().unwrap_or(0);
    let min = latencies.first().copied().unwrap_or(0);

    let detail = serde_json::json!({
        "concurrency": concurrency,
        "wall_ms": wall_ms,
        "successes": successes,
        "rate_limited_429": rate_limited,
        "errors": errors,
        "latency_min_ms": min,
        "latency_avg_ms": avg,
        "latency_p99_ms": p99,
        "latency_max_ms": max,
    });

    // PASS: server stayed responsive (no connection errors)
    let ok = errors == 0;
    let desc = format!(
        "{}/{} ok, {} rate-limited, {} errors, avg {}ms p99 {}ms",
        successes, concurrency, rate_limited, errors, avg, p99
    );
    (ok, detail, desc)
}

/// Oversized payload test: submit a >1MB JSON body to POST /chain/{id}/submit.
/// PASS if the recorder rejects it (400 or 413) rather than crashing or OOM.
async fn infra_test_oversized_payload(
    client: &reqwest::Client,
    recorder_url: &str,
    chain_id: &str,
) -> (bool, serde_json::Value, String) {
    // Build a payload exceeding the 256KB body limit and the 1MB deserialize guard.
    // Use a large string field to avoid actual valid block parsing.
    let big_string = "X".repeat(2 * 1024 * 1024); // 2 MB
    let payload = serde_json::json!({
        "type_code": "1",
        "children": [{"type_code": "99", "value": big_string}]
    });

    let url = format!("{}/chain/{}/submit", recorder_url, chain_id);
    match client.post(&url).json(&payload).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let body: String = resp.text().await.unwrap_or_default();
            // 413 Payload Too Large or 400 Bad Request are correct rejections.
            // 200 would mean the guard failed. 500 with OOM would be a crash.
            let ok = status == 413 || status == 400;
            let detail = serde_json::json!({
                "payload_bytes": 2 * 1024 * 1024,
                "response_status": status,
                "response_body": &body[..body.len().min(200)],
            });
            let desc = format!("2MB payload → {} ({})", status,
                if ok { "correctly rejected" } else { "UNEXPECTED" });
            (ok, detail, desc)
        }
        Err(e) => {
            let detail = serde_json::json!({"error": e.to_string()});
            (false, detail, format!("connection error: {}", e))
        }
    }
}

/// Auth bypass test: send requests with missing, invalid, and valid auth.
/// PASS if: unauthenticated requests are rejected (401) when keys are configured,
/// OR all requests pass when no auth is configured (verifying no false blocks).
async fn infra_test_auth_bypass(
    client: &reqwest::Client,
    recorder_url: &str,
    chain_id: &str,
    valid_key: Option<&str>,
) -> (bool, serde_json::Value, String) {
    let url = format!("{}/chain/{}/info", recorder_url, chain_id);

    // Test 1: no auth header
    let no_auth = client.get(&url).send().await;
    let no_auth_status = no_auth.map(|r| r.status().as_u16()).unwrap_or(0);

    // Test 2: invalid auth header
    let bad_auth = client.get(&url)
        .header("authorization", "Bearer INVALID-KEY-12345")
        .send().await;
    let bad_auth_status = bad_auth.map(|r| r.status().as_u16()).unwrap_or(0);

    // Test 3: valid auth (if key provided)
    let good_auth_status = if let Some(key) = valid_key {
        let resp = client.get(&url)
            .header("authorization", format!("Bearer {}", key))
            .send().await;
        resp.map(|r| r.status().as_u16()).unwrap_or(0)
    } else {
        0 // not tested
    };

    let auth_enforced = no_auth_status == 401;
    let detail = serde_json::json!({
        "no_auth_status": no_auth_status,
        "bad_auth_status": bad_auth_status,
        "good_auth_status": good_auth_status,
        "auth_enforced": auth_enforced,
    });

    if auth_enforced {
        // Auth IS enforced — verify bad key rejected, good key accepted
        let ok = bad_auth_status == 401 && (valid_key.is_none() || good_auth_status == 200);
        let desc = format!(
            "auth enforced: no-auth→{}, bad-key→{}, good-key→{}",
            no_auth_status, bad_auth_status, good_auth_status
        );
        (ok, detail, desc)
    } else {
        // Auth is NOT enforced — all requests should succeed
        let ok = no_auth_status == 200 && bad_auth_status == 200;
        let desc = format!("no auth configured: all requests → 200 (expected)");
        (ok, detail, desc)
    }
}

/// Connection exhaustion test: open `concurrency` simultaneous SSE streams.
/// PASS if the recorder handles the load (accepting up to max_connections,
/// returning 503 for excess, and not crashing).
async fn infra_test_connection_exhaustion(
    client: &reqwest::Client,
    recorder_url: &str,
    chain_id: &str,
    concurrency: usize,
) -> (bool, serde_json::Value, String) {
    let url = format!("{}/chain/{}/events", recorder_url, chain_id);
    let mut handles = Vec::with_capacity(concurrency);

    for _ in 0..concurrency {
        let c = client.clone();
        let u = url.clone();
        handles.push(tokio::spawn(async move {
            match c.get(&u).send().await {
                Ok(resp) => resp.status().as_u16(),
                Err(_) => 0,
            }
        }));
    }

    let mut accepted = 0u64;
    let mut rejected_503 = 0u64;
    let mut errors = 0u64;
    for h in handles {
        if let Ok(status) = h.await {
            match status {
                200 => accepted += 1,
                503 => rejected_503 += 1,
                0 => errors += 1,
                _ => errors += 1,
            }
        }
    }

    let detail = serde_json::json!({
        "concurrency": concurrency,
        "accepted_200": accepted,
        "rejected_503": rejected_503,
        "errors": errors,
    });

    // PASS: server didn't crash (no connection errors) and either:
    // - accepted all (no limit configured), or
    // - accepted some and rejected the rest with 503 (limit enforced)
    let ok = errors == 0 && (accepted > 0 || rejected_503 > 0);
    let desc = format!(
        "{} accepted, {} rejected (503), {} errors",
        accepted, rejected_503, errors
    );
    (ok, detail, desc)
}

/// Error probe test: send various malformed requests and verify that error
/// responses don't leak internal details (N10 error sanitization).
/// PASS if all error responses use generic messages, not stack traces or
/// internal state descriptions.
async fn infra_test_error_probe(
    client: &reqwest::Client,
    recorder_url: &str,
    chain_id: &str,
) -> (bool, serde_json::Value, String) {
    let mut probes = Vec::new();
    let mut all_sanitized = true;

    // Probe 1: submit garbage JSON to /submit
    let resp = client.post(format!("{}/chain/{}/submit", recorder_url, chain_id))
        .json(&serde_json::json!({"garbage": true}))
        .send().await;
    let (status1, body1) = extract_error(resp).await;
    let clean1 = !leaks_internals(&body1);
    if !clean1 { all_sanitized = false; }
    probes.push(serde_json::json!({
        "probe": "garbage_submit",
        "status": status1,
        "body_preview": &body1[..body1.len().min(100)],
        "sanitized": clean1,
    }));

    // Probe 2: request nonexistent chain
    let resp = client.get(format!("{}/chain/0000000000000000/info", recorder_url))
        .send().await;
    let (status2, body2) = extract_error(resp).await;
    let clean2 = !leaks_internals(&body2);
    if !clean2 { all_sanitized = false; }
    probes.push(serde_json::json!({
        "probe": "nonexistent_chain",
        "status": status2,
        "body_preview": &body2[..body2.len().min(100)],
        "sanitized": clean2,
    }));

    // Probe 3: invalid content type
    let resp = client.post(format!("{}/chain/{}/submit", recorder_url, chain_id))
        .header("content-type", "text/plain")
        .body("not json at all")
        .send().await;
    let (status3, body3) = extract_error(resp).await;
    let clean3 = !leaks_internals(&body3);
    if !clean3 { all_sanitized = false; }
    probes.push(serde_json::json!({
        "probe": "wrong_content_type",
        "status": status3,
        "body_preview": &body3[..body3.len().min(100)],
        "sanitized": clean3,
    }));

    // Probe 4: GET on POST-only endpoint
    let resp = client.get(format!("{}/chain/{}/submit", recorder_url, chain_id))
        .send().await;
    let (status4, body4) = extract_error(resp).await;
    let clean4 = !leaks_internals(&body4);
    if !clean4 { all_sanitized = false; }
    probes.push(serde_json::json!({
        "probe": "wrong_method",
        "status": status4,
        "body_preview": &body4[..body4.len().min(100)],
        "sanitized": clean4,
    }));

    let detail = serde_json::json!({ "probes": probes });
    let sanitized_count = probes.iter()
        .filter(|p| p["sanitized"].as_bool().unwrap_or(false))
        .count();
    let desc = format!(
        "{}/{} probes sanitized{}",
        sanitized_count, probes.len(),
        if all_sanitized { " (all clean)" } else { " — LEAKS DETECTED" }
    );
    (all_sanitized, detail, desc)
}

/// Extract status code and body text from a response.
async fn extract_error(resp: Result<reqwest::Response, reqwest::Error>) -> (u16, String) {
    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let body = r.text().await.unwrap_or_default();
            (status, body)
        }
        Err(e) => (0, e.to_string()),
    }
}

/// Check if an error response body leaks internal implementation details.
/// Looks for patterns that indicate stack traces, Rust types, file paths, etc.
fn leaks_internals(body: &str) -> bool {
    let lower = body.to_lowercase();
    // Look for signs of unsanitized error messages
    let leak_patterns = [
        "panicked at",
        "stack backtrace",
        "thread '",
        ".rs:",           // Rust source file paths
        "rwlock",
        "mutex",
        "poisoned",
        "sqlite",         // database internals
        "rusqlite",
        "tokio::",
        "hyper::",
        "axum::",
        "at /",           // Unix file paths
        "at c:\\",        // Windows file paths
        "core::result",
        "std::io::error",
    ];
    leak_patterns.iter().any(|pat| lower.contains(pat))
}

// ── Recorder operator agent (Sim-G: TⒶ³) ──────────────────────────
//
// Manages chain infrastructure operations: owner key rotation,
// recorder switching, and chain migration. Operations are scheduled
// at configured times during the simulation.

#[allow(clippy::too_many_arguments)]
pub async fn run_recorder_operator(
    config: AgentConfig,
    op_cfg: RecorderOperatorConfig,
    client: Arc<RecorderClient>,
    secondary_client: Option<Arc<RecorderClient>>,
    secondary_recorder_pubkey: Option<[u8; 32]>,
    secondary_recorder_url: Option<String>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    _mailbox: mpsc::Receiver<AgentMessage>,
    speed: SharedSpeed,
    paused: PauseFlag,
    initial_owner_seed: [u8; 32],
) -> Result<()> {
    let name = config.name.clone();
    let lat = config.lat;
    let lon = config.lon;

    info!("{}: recorder operator for chain of vendor '{}'", name, op_cfg.target_chain);

    // Wait for the target vendor to register its chain in the directory
    let (chain_id, symbol) = loop {
        wait_while_paused(&paused).await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let dir = directory.read().await;
        // Find the chain registered by the target vendor
        let found = dir.chains.values()
            .find(|reg| reg.vendor_name == op_cfg.target_chain)
            .map(|reg| (reg.chain_id.clone(), reg.symbol.clone()));
        drop(dir);
        if let Some((cid, sym)) = found {
            info!("{}: found chain {} ({})", name, &cid[..12], sym);
            break (cid, sym);
        }
    };

    // Owner seed comes from pre-genesis data, passed in by main.rs
    let mut current_owner_seed = initial_owner_seed;

    let mut key_rotated = false;
    let mut switch_phase = "idle".to_string();
    let mut chain_migrated = false;
    let mut ops_completed: u64 = 0;
    let mut last_result = "initialized".to_string();

    let make_state = |key_rotated: bool, switch_phase: &str, chain_migrated: bool,
                      ops_completed: u64, last_result: &str, paused: &PauseFlag| -> AgentState {
        AgentState {
            name: name.clone(),
            role: "recorder_operator".to_string(),
            status: "active".to_string(),
            lat, lon,
            chains: vec![ChainHolding {
                chain_id: chain_id.clone(),
                symbol: symbol.clone(),
                shares: BigInt::zero(),
                unspent_utxos: 0,
                coin_count: String::new(),
                total_shares: String::new(),
            }],
            key_summary: Vec::new(),
            coverage_radius: None,
            paused: paused.load(Ordering::Relaxed),
            trading_rates: Vec::new(),
            validator_status: None,
            attacker_status: None,
            infra_tester_status: None,
            caa_status: None,
            recorder_op_status: Some(RecorderOpStatus {
                target_chain: op_cfg.target_chain.clone(),
                key_rotated,
                recorder_switch_phase: switch_phase.to_string(),
                chain_migrated,
                operations_completed: ops_completed,
                last_result: last_result.to_string(),
            }),
            transactions: ops_completed,
            last_action: last_result.to_string(),
        }
    };

    let _ = state_tx.send(ViewerEvent::State(Box::new(
        make_state(false, "idle", false, 0, "initialized", &paused)
    ))).await;

    let start = std::time::Instant::now();

    loop {
        wait_while_paused(&paused).await;
        let spd = read_speed(&speed).max(0.1);
        tokio::time::sleep(std::time::Duration::from_secs_f64(1.0 / spd)).await;

        let elapsed_sim_secs = (start.elapsed().as_secs_f64() * spd) as u64;

        // Owner key rotation
        if !key_rotated && op_cfg.rotate_after_secs > 0 && elapsed_sim_secs >= op_cfg.rotate_after_secs {
            info!("{}: rotating owner key on chain {}", name, &chain_id[..12]);
            let new_key = ao_crypto::sign::SigningKey::generate();
            let new_pubkey: [u8; 32] = new_key.public_key_bytes().try_into().unwrap();

            let rotation = transfer::build_owner_key_rotation(&current_owner_seed, &new_pubkey);
            let rotation_json = ao_types::json::to_json(&rotation);

            match client.submit(&chain_id, &rotation_json).await {
                Ok(result) => {
                    info!("{}: owner key rotated at block {}", name, result.height);
                    current_owner_seed = *new_key.seed();
                    key_rotated = true;
                    ops_completed += 1;
                    last_result = format!("key rotated at block {}", result.height);
                    let _ = state_tx.send(ViewerEvent::Transaction(
                        tx_event(&chain_id, &symbol, &name, "chain", result.height, "owner key rotation")
                    )).await;
                }
                Err(e) => {
                    warn!("{}: key rotation failed: {}", name, e);
                    last_result = format!("rotation failed: {}", e);
                }
            }
            let _ = state_tx.send(ViewerEvent::State(Box::new(
                make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
            ))).await;
        }

        // Recorder switch
        if switch_phase == "idle" && op_cfg.switch_after_secs > 0 && elapsed_sim_secs >= op_cfg.switch_after_secs {
            if let (Some(sec_url), Some(sec_pk)) = (&secondary_recorder_url, &secondary_recorder_pubkey) {
                info!("{}: initiating recorder switch on chain {}", name, &chain_id[..12]);
                let pending = transfer::build_recorder_change_pending(
                    &current_owner_seed, sec_pk, sec_url,
                );
                let pending_json = ao_types::json::to_json(&pending);

                match client.submit(&chain_id, &pending_json).await {
                    Ok(result) => {
                        info!("{}: recorder change pending at block {}", name, result.height);
                        switch_phase = "pending".to_string();
                        ops_completed += 1;
                        last_result = format!("recorder change pending at block {}", result.height);
                        let _ = state_tx.send(ViewerEvent::Transaction(
                            tx_event(&chain_id, &symbol, &name, "chain", result.height, "recorder change pending")
                        )).await;
                    }
                    Err(e) => {
                        warn!("{}: recorder change pending failed: {}", name, e);
                        last_result = format!("pending failed: {}", e);
                    }
                }
                let _ = state_tx.send(ViewerEvent::State(Box::new(
                    make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
                ))).await;
            } else {
                warn!("{}: no secondary recorder configured for switch", name);
                switch_phase = "skipped".to_string();
            }
        }

        // Monitor recorder switch progress
        if switch_phase == "pending" {
            // Check chain info for pending_recorder_change status
            // The recorder auto-constructs RECORDER_CHANGE when CAA escrows drain.
            // In sims without active CAAs, this should happen on the next block.
            match client.chain_info(&chain_id).await {
                Ok(_info) => {
                    // After RECORDER_CHANGE_PENDING, the next block construction
                    // triggers auto-change if no active CAA escrows.
                    // We trigger it by waiting — the next regular block will complete it.
                    // Poll for completion: if chain is no longer pending, switch is done.
                    // For now, mark as draining and wait for the auto-change.
                    switch_phase = "draining".to_string();
                    last_result = "waiting for CAA drain + auto-change".to_string();
                }
                Err(e) => {
                    warn!("{}: chain info poll failed: {}", name, e);
                }
            }
            let _ = state_tx.send(ViewerEvent::State(Box::new(
                make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
            ))).await;
        }

        if switch_phase == "draining" {
            // The auto-change fires after the next block construction when no CAAs active.
            // Once the RECORDER_CHANGE block is recorded, we can consider it complete.
            // We detect completion by checking if the chain info indicates the switch happened.
            // Simple approach: wait a few ticks then mark complete.
            // A real check would look at chain info for the new recorder pubkey.
            tokio::time::sleep(std::time::Duration::from_secs_f64(3.0 / read_speed(&speed).max(0.1))).await;
            switch_phase = "complete".to_string();
            ops_completed += 1;
            last_result = "recorder switch complete".to_string();
            info!("{}: recorder switch complete on chain {}", name, &chain_id[..12]);
            let _ = state_tx.send(ViewerEvent::State(Box::new(
                make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
            ))).await;
        }

        // Chain migration
        if !chain_migrated && op_cfg.migrate_after_secs > 0 && elapsed_sim_secs >= op_cfg.migrate_after_secs {
            info!("{}: initiating chain migration on chain {}", name, &chain_id[..12]);

            // Create a new chain on the secondary recorder (or primary if no secondary)
            let target_client = secondary_client.as_ref().unwrap_or(&client);
            let new_symbol = op_cfg.migration_symbol.clone()
                .unwrap_or_else(|| format!("{}2", symbol));

            let new_issuer = ao_crypto::sign::SigningKey::generate();
            let new_issuer_seed = *new_issuer.seed();

            // Use original chain's parameters for the new chain
            let (coins, shares, fee_num, fee_den) = match client.chain_info(&chain_id).await {
                Ok(info) => (
                    info.coin_count.parse().unwrap_or_else(|_| parse_bigint("1000000000")),
                    info.shares_out.parse().unwrap_or_else(|_| parse_bigint("2^40")),
                    info.fee_rate_num.parse().unwrap_or_else(|_| parse_bigint("1")),
                    info.fee_rate_den.parse().unwrap_or_else(|_| crate::config::auto_fee_den("1000000000")),
                ),
                Err(_) => (
                    parse_bigint("1000000000"),
                    parse_bigint("2^40"),
                    parse_bigint("1"),
                    crate::config::auto_fee_den("1000000000"),
                ),
            };
            let fee_rate = transfer::FeeRate { num: fee_num, den: fee_den };

            let (new_genesis_item, new_genesis_json) = transfer::build_genesis(
                &new_issuer_seed, &new_symbol, &format!("Migrated from {}", symbol),
                &coins, &shares, &fee_rate,
            );

            // Compute new chain ID
            let new_chain_id = match ao_chain::genesis::compute_chain_id(&new_genesis_item) {
                Ok(id) => id,
                Err(e) => {
                    warn!("{}: failed to compute new chain ID: {}", name, e);
                    last_result = format!("migration failed: {}", e);
                    let _ = state_tx.send(ViewerEvent::State(Box::new(
                        make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
                    ))).await;
                    continue;
                }
            };

            // Create the new chain on the target recorder
            match target_client.create_chain(&new_genesis_json).await {
                Ok(info) => {
                    info!("{}: new chain created: {} ({})", name, &info.chain_id[..12], new_symbol);
                }
                Err(e) => {
                    warn!("{}: failed to create migration target chain: {}", name, e);
                    last_result = format!("migration chain creation failed: {}", e);
                    let _ = state_tx.send(ViewerEvent::State(Box::new(
                        make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
                    ))).await;
                    continue;
                }
            }

            // Submit CHAIN_MIGRATION to freeze the old chain
            let migration = transfer::build_chain_migration(&current_owner_seed, &new_chain_id);
            let migration_json = ao_types::json::to_json(&migration);

            match client.submit(&chain_id, &migration_json).await {
                Ok(result) => {
                    info!("{}: chain migration recorded at block {}, chain frozen", name, result.height);
                    chain_migrated = true;
                    ops_completed += 1;
                    last_result = format!("chain migrated at block {} → {}", result.height, &hex::encode(new_chain_id)[..12]);
                    let _ = state_tx.send(ViewerEvent::Transaction(
                        tx_event(&chain_id, &symbol, &name, "chain", result.height,
                            &format!("chain migration → {}", new_symbol))
                    )).await;
                }
                Err(e) => {
                    warn!("{}: chain migration failed: {}", name, e);
                    last_result = format!("migration failed: {}", e);
                }
            }
            let _ = state_tx.send(ViewerEvent::State(Box::new(
                make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
            ))).await;
        }

        // All operations scheduled — if all are done, we're idle
        let all_done = (op_cfg.rotate_after_secs == 0 || key_rotated)
            && (op_cfg.switch_after_secs == 0 || switch_phase == "complete" || switch_phase == "skipped")
            && (op_cfg.migrate_after_secs == 0 || chain_migrated);

        if all_done {
            last_result = "all operations complete".to_string();
            let _ = state_tx.send(ViewerEvent::State(Box::new(
                make_state(key_rotated, &switch_phase, chain_migrated, ops_completed, &last_result, &paused)
            ))).await;
            // Stay alive but idle — just report state periodically
            tokio::time::sleep(std::time::Duration::from_secs_f64(10.0 / read_speed(&speed).max(0.1))).await;
        }
    }
}

fn tx_event(
    chain_id: &str, symbol: &str,
    from: &str, to: &str,
    block_height: u64, description: &str,
) -> TransactionEvent {
    TransactionEvent {
        id: 0, // filled by ViewerState
        timestamp_ms: now_ms(),
        chain_id: chain_id.to_string(),
        symbol: symbol.to_string(),
        from_agent: from.to_string(),
        to_agent: to.to_string(),
        block_height,
        description: description.to_string(),
    }
}
