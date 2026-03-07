use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use num_bigint::BigInt;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{info, warn};

use crate::client::RecorderClient;
use crate::config::{AgentConfig, VendorConfig, ExchangeConfig, ConsumerConfig, parse_bigint, auto_fee_den};
use crate::transfer::{self, Giver, Receiver};
use crate::wallet::Wallet;

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

// ── Agent state (reported to observer) ──────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AgentState {
    pub name: String,
    pub role: String,
    pub status: String,
    pub chains: Vec<ChainHolding>,
    pub transactions: u64,
    pub last_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChainHolding {
    pub chain_id: String,
    pub symbol: String,
    #[serde(serialize_with = "serialize_bigint")]
    pub shares: BigInt,
    pub unspent_utxos: usize,
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
    State(AgentState),
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
}

pub struct AgentDirectory {
    agents: HashMap<String, AgentSender>,
    /// chain_id → chain registration
    pub chains: HashMap<String, ChainRegistration>,
    /// symbol → chain_id for quick lookup
    pub symbol_to_chain: HashMap<String, String>,
}

impl AgentDirectory {
    pub fn new() -> Self {
        AgentDirectory {
            agents: HashMap::new(),
            chains: HashMap::new(),
            symbol_to_chain: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, sender: AgentSender) {
        self.agents.insert(name.to_string(), sender);
    }

    pub fn register_chain(&mut self, chain_id: &str, symbol: &str, vendor_name: &str, plate_price: u64) {
        self.symbol_to_chain.insert(symbol.to_string(), chain_id.to_string());
        self.chains.insert(chain_id.to_string(), ChainRegistration {
            chain_id: chain_id.to_string(),
            symbol: symbol.to_string(),
            vendor_name: vendor_name.to_string(),
            plate_price,
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

pub async fn run_vendor(
    config: AgentConfig,
    vendor_cfg: VendorConfig,
    client: Arc<RecorderClient>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    mut mailbox: mpsc::Receiver<AgentMessage>,
) -> Result<()> {
    let name = config.name.clone();
    let mut wallet = Wallet::new(&name);
    let mut tx_count: u64 = 0;

    // Create genesis
    let issuer_key = ao_crypto::sign::SigningKey::generate();
    let seed = *issuer_key.seed();
    let coins = parse_bigint(&vendor_cfg.coins);
    let shares = parse_bigint(&vendor_cfg.shares);

    let fee_num = parse_bigint(&vendor_cfg.fee_num);
    let fee_den = vendor_cfg.fee_den.as_ref()
        .map(|s| parse_bigint(s))
        .unwrap_or_else(|| auto_fee_den(&vendor_cfg.coins));
    let fee_rate = transfer::FeeRate { num: fee_num, den: fee_den };

    let (_genesis, genesis_json) = transfer::build_genesis(
        &seed, &vendor_cfg.symbol, &vendor_cfg.description, &coins, &shares, &fee_rate,
    );

    let chain_info = client.create_chain(&genesis_json).await?;
    let chain_id = chain_info.chain_id.clone();
    info!("{}: Created chain {} ({})", name, vendor_cfg.symbol, &chain_id[..12]);

    // Register chain + issuer UTXO
    {
        let mut dir = directory.write().await;
        dir.register_chain(&chain_id, &vendor_cfg.symbol, &name, vendor_cfg.plate_price);
    }
    let issuer_entry = wallet.import_key(seed, &chain_id);
    wallet.register_utxo(&issuer_entry.pubkey, 1, shares.clone());
    let _ = state_tx.send(ViewerEvent::State(build_state(&name,"vendor", "ready", &wallet, &chain_id, &vendor_cfg.symbol, tx_count, "genesis created"))).await;

    // Handle messages
    while let Some(msg) = mailbox.recv().await {
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
                    let _ = state_tx.send(ViewerEvent::State(build_state(&name,"vendor", "ready", &wallet, &chain_id, &vendor_cfg.symbol, tx_count, &format!("block {}", r.block_height)))).await;
                }
                let _ = reply.send(result);
            }
            AgentMessage::NotifyUtxo { pubkey, seq_id, amount } => {
                wallet.register_utxo(&pubkey, seq_id, amount);
                let _ = state_tx.send(ViewerEvent::State(build_state(&name,"vendor", "ready", &wallet, &chain_id, &vendor_cfg.symbol, tx_count, "received redemption"))).await;
            }
            AgentMessage::CrossChainBuy { reply, .. } => {
                let _ = reply.send(Err(anyhow::anyhow!("{}: vendors do not handle cross-chain buys", name)));
            }
        }
    }
    Ok(())
}

// ── Exchange agent ──────────────────────────────────────────────────

pub async fn run_exchange(
    config: AgentConfig,
    exchange_cfg: ExchangeConfig,
    client: Arc<RecorderClient>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    mut mailbox: mpsc::Receiver<AgentMessage>,
    _speed: f64,
) -> Result<()> {
    let name = config.name.clone();
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

    let _ = state_tx.send(ViewerEvent::State(build_multi_state(
        &name, "exchange", "ready", &wallet, &my_chains, tx_count, "inventory acquired",
    ))).await;

    // Handle messages
    while let Some(msg) = mailbox.recv().await {
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
                let _ = state_tx.send(ViewerEvent::State(build_multi_state(
                    &name, "exchange", "ready", &wallet, &my_chains, tx_count,
                    &format!("sold {}", sym),
                ))).await;
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
                    &pair_rates,
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
                let _ = state_tx.send(ViewerEvent::State(build_multi_state(
                    &name, "exchange", "ready", &wallet, &my_chains, tx_count, "cross-chain trade",
                ))).await;
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
) -> Result<CrossChainResult> {
    // Look up exchange rate
    let rate = pair_rates.get(&(sell_chain_id.to_string(), pay_chain_id.to_string()))
        .ok_or_else(|| anyhow::anyhow!("{}: no trading pair for this chain combination", name))?;

    // Leg 1: Receive payment on pay_chain (consumer sends to us)
    // Consumer already built the payment transfer — we just need to confirm
    // In this sim, the consumer already executed leg 1 before sending CrossChainBuy.
    // So we just need to do leg 2.

    // Calculate sell amount from pay amount and rate.
    // rate = how many pay units per 1 sell unit (as f64, converted to rational).
    // sell_amount = pay_amount / rate, using integer arithmetic to avoid f64 precision loss.
    // Express rate as integer ratio: rate = rate_num / rate_den (multiply by 1_000_000 for precision).
    let rate_scale = 1_000_000u64;
    let rate_num = BigInt::from((*rate * rate_scale as f64) as u64);
    let rate_den = BigInt::from(rate_scale);
    let sell_amount = pay_amount * &rate_den / &rate_num;

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

// ── Consumer agent ──────────────────────────────────────────────────

pub async fn run_consumer(
    config: AgentConfig,
    consumer_cfg: ConsumerConfig,
    client: Arc<RecorderClient>,
    directory: Arc<RwLock<AgentDirectory>>,
    state_tx: StateCollector,
    _mailbox: mpsc::Receiver<AgentMessage>,
    speed: f64,
) -> Result<()> {
    let name = config.name.clone();
    let mut wallet = Wallet::new(&name);
    let mut tx_count: u64 = 0;

    let is_cross_chain = consumer_cfg.want_symbol.is_some() && consumer_cfg.pay_symbol.is_some();

    if is_cross_chain {
        run_cross_chain_consumer(
            &name, &consumer_cfg, &client, &directory, &state_tx,
            &mut wallet, &mut tx_count, speed,
        ).await
    } else {
        run_single_chain_consumer(
            &name, &consumer_cfg, &client, &directory, &state_tx,
            &mut wallet, &mut tx_count, speed,
        ).await
    }
}

async fn run_single_chain_consumer(
    name: &str,
    consumer_cfg: &ConsumerConfig,
    client: &RecorderClient,
    directory: &Arc<RwLock<AgentDirectory>>,
    state_tx: &StateCollector,
    wallet: &mut Wallet,
    tx_count: &mut u64,
    speed: f64,
) -> Result<()> {
    let redeem_at_name = consumer_cfg.redeem_at.as_deref()
        .ok_or_else(|| anyhow::anyhow!("{}: single-chain consumer requires redeem_at", name))?;

    let (chain_id, symbol, plate_price) = wait_for_vendor_chain(directory, redeem_at_name).await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let interval = std::time::Duration::from_secs_f64(
        consumer_cfg.interval_secs as f64 / speed.max(0.1)
    );
    info!("{}: Starting single-chain purchase loop (every {:?})", name, interval);

    loop {
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

        let _ = state_tx.send(ViewerEvent::State(build_state(name, "consumer", "active", wallet, &chain_id, &symbol, *tx_count, "purchased + redeemed"))).await;
    }
}

async fn run_cross_chain_consumer(
    name: &str,
    consumer_cfg: &ConsumerConfig,
    client: &RecorderClient,
    directory: &Arc<RwLock<AgentDirectory>>,
    state_tx: &StateCollector,
    wallet: &mut Wallet,
    tx_count: &mut u64,
    speed: f64,
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

    let interval = std::time::Duration::from_secs_f64(
        consumer_cfg.interval_secs as f64 / speed.max(0.1)
    );
    info!("{}: Starting cross-chain loop {} → {} (every {:?})", name, pay_sym, want_sym, interval);

    let mut my_chains = HashMap::new();
    my_chains.insert(want_chain_id.clone(), want_sym.to_string());
    my_chains.insert(pay_chain_id.clone(), pay_sym.to_string());

    loop {
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

        let _ = state_tx.send(ViewerEvent::State(build_multi_state(
            name, "consumer", "active", wallet, &my_chains, *tx_count, "cross-chain trade",
        ))).await;
    }
}

// ── Shared helpers ──────────────────────────────────────────────────

/// Handle a SellToMe request: find own unspent UTXO, build transfer, execute.
async fn handle_sell(
    name: &str,
    wallet: &mut Wallet,
    client: &RecorderClient,
    chain_id: &str,
    receivers: &mut Vec<Receiver>,
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

/// Build an AgentState snapshot for reporting.
fn build_state(
    name: &str, role: &str, status: &str,
    wallet: &Wallet, chain_id: &str, symbol: &str,
    transactions: u64, last_action: &str,
) -> AgentState {
    AgentState {
        name: name.to_string(),
        role: role.to_string(),
        status: status.to_string(),
        chains: vec![ChainHolding {
            chain_id: chain_id.to_string(),
            symbol: symbol.to_string(),
            shares: wallet.balance(chain_id),
            unspent_utxos: wallet.find_all_unspent(chain_id).len(),
        }],
        transactions,
        last_action: last_action.to_string(),
    }
}

/// Build an AgentState snapshot for multi-chain agents.
fn build_multi_state(
    name: &str, role: &str, status: &str,
    wallet: &Wallet, chains: &HashMap<String, String>,
    transactions: u64, last_action: &str,
) -> AgentState {
    let holdings: Vec<ChainHolding> = chains.iter().map(|(chain_id, symbol)| {
        ChainHolding {
            chain_id: chain_id.clone(),
            symbol: symbol.clone(),
            shares: wallet.balance(chain_id),
            unspent_utxos: wallet.find_all_unspent(chain_id).len(),
        }
    }).collect();

    AgentState {
        name: name.to_string(),
        role: role.to_string(),
        status: status.to_string(),
        chains: holdings,
        transactions,
        last_action: last_action.to_string(),
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
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
