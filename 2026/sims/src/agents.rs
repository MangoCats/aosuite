use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use num_bigint::BigInt;
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
    /// Empty `givers` = recipient fills in from its own wallet.
    /// Receivers include seeds so the executor can sign for all parties.
    SellToMe {
        chain_id: String,
        receivers: Vec<Receiver>,
        reply: oneshot::Sender<Result<TransferResult>>,
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
    pub seq_count: u64,
}

// ── Agent state (reported to observer) ──────────────────────────────

#[derive(Debug, Clone)]
pub struct AgentState {
    pub name: String,
    pub role: String,
    pub status: String,
    pub chains: Vec<ChainHolding>,
    pub transactions: u64,
    pub last_action: String,
}

#[derive(Debug, Clone)]
pub struct ChainHolding {
    pub chain_id: String,
    pub symbol: String,
    pub shares: BigInt,
    pub unspent_utxos: usize,
}

// ── Agent directory ─────────────────────────────────────────────────

pub type AgentSender = mpsc::Sender<AgentMessage>;
pub type StateCollector = mpsc::Sender<AgentState>;

pub struct AgentDirectory {
    agents: HashMap<String, AgentSender>,
    /// chain_id → (symbol, vendor_name, plate_price_coins)
    pub chains: HashMap<String, (String, String, u64)>,
}

impl AgentDirectory {
    pub fn new() -> Self {
        AgentDirectory { agents: HashMap::new(), chains: HashMap::new() }
    }

    pub fn register(&mut self, name: &str, sender: AgentSender) {
        self.agents.insert(name.to_string(), sender);
    }

    pub fn register_chain(&mut self, chain_id: &str, symbol: &str, vendor_name: &str, plate_price: u64) {
        self.chains.insert(chain_id.to_string(), (symbol.to_string(), vendor_name.to_string(), plate_price));
    }

    pub fn get(&self, name: &str) -> Option<&AgentSender> {
        self.agents.get(name)
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
    let _ = state_tx.send(build_state(&name,"vendor", "ready", &wallet, &chain_id, &vendor_cfg.symbol, tx_count, "genesis created")).await;

    // Handle messages
    while let Some(msg) = mailbox.recv().await {
        match msg {
            AgentMessage::RequestPubkey { chain_id: cid, reply } => {
                let entry = wallet.generate_key(&cid);
                let _ = reply.send(PubkeyResponse { pubkey: entry.pubkey, seed: entry.seed });
            }
            AgentMessage::SellToMe { chain_id: cid, mut receivers, reply } => {
                let result = handle_sell(&name, &mut wallet, &client, &cid, &mut receivers).await;
                if let Ok(ref r) = result {
                    tx_count += 1;
                    info!("{}: Block {} (tx #{})", name, r.block_height, tx_count);
                    let _ = state_tx.send(build_state(&name,"vendor", "ready", &wallet, &chain_id, &vendor_cfg.symbol, tx_count, &format!("block {}", r.block_height))).await;
                }
                let _ = reply.send(result);
            }
            AgentMessage::NotifyUtxo { pubkey, seq_id, amount } => {
                wallet.register_utxo(&pubkey, seq_id, amount);
                let _ = state_tx.send(build_state(&name,"vendor", "ready", &wallet, &chain_id, &vendor_cfg.symbol, tx_count, "received redemption")).await;
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

    // Wait for vendor's chain
    let (chain_id, symbol, plate_price) = wait_for_vendor_chain(&directory, &exchange_cfg.buy_from).await?;
    info!("{}: Found chain {} ({})", name, symbol, &chain_id[..12]);

    // Buy initial inventory from vendor
    let info = client.chain_info(&chain_id).await?;
    let total_shares: BigInt = info.shares_out.parse()?;
    let total_coins: BigInt = info.coin_count.parse()?;
    let buy_coins = BigInt::from(plate_price) * BigInt::from(exchange_cfg.initial_buy);
    let buy_shares = &total_shares * &buy_coins / &total_coins;

    request_purchase(
        &name, &mut wallet, &directory,
        &exchange_cfg.buy_from, &chain_id, &buy_shares,
    ).await?;
    tx_count += 1;
    info!("{}: Bought {} plates from {}", name, exchange_cfg.initial_buy, exchange_cfg.buy_from);
    let _ = state_tx.send(build_state(&name,"exchange", "ready", &wallet, &chain_id, &symbol, tx_count, "inventory acquired")).await;

    // Handle buy requests from consumers
    while let Some(msg) = mailbox.recv().await {
        match msg {
            AgentMessage::RequestPubkey { chain_id: cid, reply } => {
                let entry = wallet.generate_key(&cid);
                let _ = reply.send(PubkeyResponse { pubkey: entry.pubkey, seed: entry.seed });
            }
            AgentMessage::SellToMe { chain_id: cid, mut receivers, reply } => {
                let result = handle_sell(&name, &mut wallet, &client, &cid, &mut receivers).await;
                if let Ok(ref r) = result {
                    tx_count += 1;
                    info!("{}: Block {} (tx #{})", name, r.block_height, tx_count);
                    let _ = state_tx.send(build_state(&name,"exchange", "ready", &wallet, &chain_id, &symbol, tx_count, &format!("sold plate, block {}", r.block_height))).await;
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

    // Wait for vendor's chain
    let (chain_id, symbol, plate_price) = wait_for_vendor_chain(&directory, &consumer_cfg.redeem_at).await?;
    // Give exchange time to acquire inventory
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let interval = std::time::Duration::from_secs_f64(
        consumer_cfg.interval_secs as f64 / speed.max(0.1)
    );
    info!("{}: Starting purchase loop (every {:?})", name, interval);

    loop {
        tokio::time::sleep(interval).await;

        // Step 1: Buy from exchange
        let info = match client.chain_info(&chain_id).await {
            Ok(i) => i,
            Err(e) => {
                warn!("{}: chain_info failed: {}", name, e);
                continue;
            }
        };
        let total_shares: BigInt = info.shares_out.parse()?;
        let total_coins: BigInt = info.coin_count.parse()?;
        let plate_shares = &total_shares * BigInt::from(plate_price) / &total_coins;

        match request_purchase(
            &name, &mut wallet, &directory,
            &consumer_cfg.buy_from, &chain_id, &plate_shares,
        ).await {
            Ok(()) => {
                tx_count += 1;
                info!("{}: Bought plate from {}", name, consumer_cfg.buy_from);
            }
            Err(e) => {
                warn!("{}: Buy failed: {}", name, e);
                continue;
            }
        }

        // Step 2: Redeem all unspent UTXOs at vendor
        while wallet.find_unspent(&chain_id).is_some() {
            match redeem_at(
                &name, &mut wallet, &client, &directory,
                &consumer_cfg.redeem_at, &chain_id,
            ).await {
                Ok(h) => {
                    tx_count += 1;
                    info!("{}: Redeemed at {} (block {})", name, consumer_cfg.redeem_at, h);
                }
                Err(e) => {
                    warn!("{}: Redeem failed: {}", name, e);
                    break;
                }
            }
        }

        let _ = state_tx.send(build_state(&name,"consumer", "active", &wallet, &chain_id, &symbol, tx_count, "purchased + redeemed")).await;
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
        seq_count: result.seq_count,
    })
}

/// Request a purchase from another agent (vendor or exchange).
/// The seller provides the giver UTXO; we provide our receiver key.
async fn request_purchase(
    _name: &str,
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
            for (chain_id, (symbol, name, plate_price)) in &dir.chains {
                if name == vendor_name {
                    return Ok((chain_id.clone(), symbol.clone(), *plate_price));
                }
            }
        }
        if std::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for vendor {} chain", vendor_name);
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
