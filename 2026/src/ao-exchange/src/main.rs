use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tower_http::limit::RequestBodyLimitLayer;
use tracing::info;

use ao_exchange::config;
use ao_exchange::db::{TradeStore, TradeRecord, TradeQuery};
use ao_exchange::engine::ExchangeEngine;
use ao_exchange::client::RecorderClient;
use ao_exchange::client::parse_sse_events;

/// Maximum request body size (64 KB — trade requests are small JSON).
const MAX_BODY_SIZE: usize = 64 * 1024;

#[derive(Parser)]
#[command(name = "ao-exchange", about = "Assign Onward exchange agent daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the exchange agent daemon.
    Run {
        /// Path to config TOML file.
        config: String,
        /// HTTP API listen address (default: 127.0.0.1:3100).
        #[arg(long, default_value = "127.0.0.1:3100")]
        listen: String,
    },
    /// Show current exchange agent status (positions, pairs).
    Status {
        /// Path to config TOML file.
        config: String,
    },
}

type SharedEngine = Arc<Mutex<ExchangeEngine>>;
type SharedStore = Arc<std::sync::Mutex<TradeStore>>;

#[derive(Clone)]
struct AppState {
    engine: SharedEngine,
    store: SharedStore,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Run { config: config_path, listen } => run_daemon(&config_path, &listen).await,
        Command::Status { config: config_path } => show_status(&config_path).await,
    }
}

async fn run_daemon(config_path: &str, listen: &str) -> Result<()> {
    let cfg = config::load_config(config_path)?;
    let poll_interval = std::time::Duration::from_secs(cfg.poll_interval_secs);
    let use_sse = cfg.deposit_detection == "sse";

    let engine = ExchangeEngine::from_config(&cfg).await
        .context("failed to initialize exchange engine")?;

    info!(
        pairs = engine.pairs.len(),
        chains = engine.chains.len(),
        detection = %cfg.deposit_detection,
        poll_secs = cfg.poll_interval_secs,
        trade_ttl = cfg.trade_ttl_secs,
        "Exchange agent started"
    );

    for (symbol, balance) in engine.positions() {
        info!(symbol = %symbol, balance = %balance, "Position");
    }

    let shared = Arc::new(Mutex::new(engine));
    let trade_store = TradeStore::open(&cfg.db_path)
        .context("failed to open trade history database")?;
    let shared_store = Arc::new(std::sync::Mutex::new(trade_store));
    info!(db_path = %cfg.db_path, "Trade history database opened");

    let app_state = AppState {
        engine: shared.clone(),
        store: shared_store.clone(),
    };

    // HTTP API with body size limit
    let app = Router::new()
        .route("/trade", post(handle_trade_request))
        .route("/status", get(handle_status))
        .route("/trades", get(handle_trades_query))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind(listen).await
        .context("failed to bind HTTP listener")?;
    info!(listen = %listen, "HTTP API listening");

    // Spawn HTTP server
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    if use_sse {
        run_sse_loop(shared, shared_store, poll_interval).await
    } else {
        run_polling_loop(shared, shared_store, poll_interval).await
    }
}

/// Record trade outcomes to the persistent store.
fn record_trade_outcomes(
    store: &SharedStore,
    outcomes: &[ao_exchange::engine::TradeOutcome],
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let store = match store.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("trade store lock poisoned: {}", e);
            return;
        }
    };

    for outcome in outcomes {
        let (status, sell_amount, error_msg) = match &outcome.result {
            Ok((_height, amount)) => ("completed", amount.to_string(), None),
            Err(e) => ("failed", "0".to_string(), Some(e.to_string())),
        };

        let record = TradeRecord {
            trade_id: outcome.trade_id.clone(),
            buy_symbol: outcome.buy_symbol.clone(),
            sell_symbol: outcome.sell_symbol.clone(),
            buy_chain_id: outcome.buy_chain_id.clone(),
            sell_chain_id: outcome.sell_chain_id.clone(),
            buy_amount: outcome.buy_amount.to_string(),
            sell_amount,
            rate: outcome.rate,
            spread: outcome.spread,
            status: status.to_string(),
            requested_at: outcome.requested_at as i64,
            completed_at: now,
            error_message: error_msg,
        };

        if let Err(e) = store.insert_trade(&record) {
            tracing::error!(trade_id = %outcome.trade_id, "failed to record trade: {}", e);
        }
    }
}

/// Maximum SSE buffer size per chain (64 KB). Protects against unbounded
/// buffer growth from malformed or malicious SSE streams.
const MAX_SSE_BUF: usize = 64 * 1024;

/// SSE-based deposit detection: subscribe to block events on each chain.
/// Falls back to polling if SSE connection drops, reconnects after poll_interval.
async fn run_sse_loop(
    shared: SharedEngine,
    trade_store: SharedStore,
    poll_interval: std::time::Duration,
) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(32);

    // Spawn per-chain SSE listeners
    let chain_configs: Vec<(String, String, String)> = {
        let engine = shared.lock().await;
        engine.chains.iter().map(|(chain_id, state)| {
            (chain_id.clone(), state.symbol.clone(), state.client.base_url().to_string())
        }).collect()
    };

    for (chain_id, symbol, recorder_url) in chain_configs {
        let tx = tx.clone();
        let poll_interval = poll_interval;
        tokio::spawn(async move {
            loop {
                let client = RecorderClient::new(&recorder_url);
                match client.subscribe_blocks(&chain_id).await {
                    Ok(mut resp) => {
                        info!(chain = %symbol, "SSE connected");
                        let mut buf = String::new();
                        loop {
                            match resp.chunk().await {
                                Ok(Some(chunk)) => {
                                    if let Ok(text) = std::str::from_utf8(&chunk) {
                                        buf.push_str(text);

                                        // Guard against unbounded buffer growth
                                        if buf.len() > MAX_SSE_BUF {
                                            tracing::warn!(
                                                chain = %symbol,
                                                buf_len = buf.len(),
                                                "SSE buffer exceeded limit, clearing"
                                            );
                                            buf.clear();
                                            continue;
                                        }

                                        let result = parse_sse_events(&buf);
                                        if !result.events.is_empty() {
                                            // Retain any partial trailing data
                                            buf = buf[result.consumed..].to_string();
                                            for event in &result.events {
                                                tracing::debug!(
                                                    chain = %symbol,
                                                    height = event.height,
                                                    seq_count = event.seq_count,
                                                    "SSE block event"
                                                );
                                            }
                                            // Notify main loop to check deposits
                                            let _ = tx.send(chain_id.clone()).await;
                                        }
                                    }
                                }
                                Ok(None) => {
                                    tracing::warn!(chain = %symbol, "SSE stream ended");
                                    break;
                                }
                                Err(e) => {
                                    tracing::warn!(chain = %symbol, "SSE error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(chain = %symbol, "SSE connect failed: {}", e);
                    }
                }
                // Reconnect after poll_interval (fallback to polling behavior)
                tracing::info!(chain = %symbol, "Falling back to poll, reconnecting SSE in {:?}", poll_interval);
                // Do a poll-based check while SSE is down
                let _ = tx.send(chain_id.clone()).await;
                tokio::time::sleep(poll_interval).await;
            }
        });
    }
    drop(tx); // Drop sender so rx ends when all spawned tasks end

    // Main loop: check deposits whenever an SSE event arrives
    while let Some(_chain_id) = rx.recv().await {
        // Drain any queued notifications to batch process
        while rx.try_recv().is_ok() {}

        let mut engine = shared.lock().await;
        let outcomes = engine.check_deposits().await;
        if !outcomes.is_empty() {
            info!(trades = outcomes.len(), "SSE-triggered deposit check completed trades");
            record_trade_outcomes(&trade_store, &outcomes);
        }
        for (symbol, balance) in engine.positions() {
            tracing::debug!(symbol = %symbol, balance = %balance, "Position");
        }
    }

    tracing::error!("All SSE listeners exited — exchange agent stopping");
    anyhow::bail!("all SSE listeners exited unexpectedly")
}

/// Legacy polling-based deposit detection.
async fn run_polling_loop(
    shared: SharedEngine,
    trade_store: SharedStore,
    poll_interval: std::time::Duration,
) -> Result<()> {
    loop {
        tokio::time::sleep(poll_interval).await;

        let mut engine = shared.lock().await;
        let outcomes = engine.check_deposits().await;

        if !outcomes.is_empty() {
            info!(trades = outcomes.len(), "Poll cycle completed trades");
            record_trade_outcomes(&trade_store, &outcomes);
        }

        for (symbol, balance) in engine.positions() {
            tracing::debug!(symbol = %symbol, balance = %balance, "Position");
        }
    }
}

// ── HTTP Handlers ────────────────────────────────────────────────────

/// Structured JSON error matching ao-recorder's error format.
#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn json_error(status: StatusCode, msg: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.into() }))
}

#[derive(Deserialize)]
struct TradeRequest {
    /// Symbol of chain consumer wants to receive (agent sells).
    sell_symbol: String,
    /// Symbol of chain consumer will pay on (agent buys).
    buy_symbol: String,
    /// Amount consumer will deposit on buy chain.
    amount: String,
}

#[derive(Serialize)]
struct TradeResponse {
    trade_id: String,
    /// Buy chain: consumer deposits here.
    deposit_chain_id: String,
    deposit_pubkey: String,
    deposit_seed: String,
    /// Sell chain: consumer receives here.
    receive_chain_id: String,
    receive_pubkey: String,
    receive_seed: String,
    /// Estimated amount consumer will receive (before fees).
    estimated_receive_amount: String,
    /// Unix timestamp when this trade request expires.
    expires_at: u64,
}

async fn handle_trade_request(
    State(state): State<AppState>,
    Json(req): Json<TradeRequest>,
) -> Result<Json<TradeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let amount: num_bigint::BigInt = req.amount.parse()
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, format!("invalid amount: {}", e)))?;

    let mut engine = state.engine.lock().await;
    let trade = engine.request_trade(&req.sell_symbol, &req.buy_symbol, &amount)
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, e.to_string()))?;

    let resp = TradeResponse {
        trade_id: trade.trade_id.clone(),
        deposit_chain_id: trade.buy_chain_id.clone(),
        deposit_pubkey: hex::encode(trade.deposit_pubkey),
        deposit_seed: hex::encode(trade.deposit_seed),
        receive_chain_id: trade.sell_chain_id.clone(),
        receive_pubkey: hex::encode(trade.receive_pubkey),
        receive_seed: hex::encode(trade.receive_seed),
        estimated_receive_amount: trade.estimated_receive_amount.to_string(),
        expires_at: trade.expires_at,
    };

    Ok(Json(resp))
}

#[derive(Serialize)]
struct StatusResponse {
    pairs: Vec<PairStatus>,
    positions: Vec<PositionStatus>,
    pending_trades: usize,
}

#[derive(Serialize)]
struct PairStatus {
    sell: String,
    buy: String,
    rate: f64,
    spread: f64,
}

#[derive(Serialize)]
struct PositionStatus {
    symbol: String,
    balance: String,
    low_stock: bool,
}

async fn handle_status(
    State(state): State<AppState>,
) -> Json<StatusResponse> {
    let engine = state.engine.lock().await;
    Json(StatusResponse {
        pairs: engine.pairs.iter().map(|p| PairStatus {
            sell: p.sell_symbol.clone(),
            buy: p.buy_symbol.clone(),
            rate: p.rate,
            spread: p.spread,
        }).collect(),
        positions: {
            let positions = engine.positions();
            positions.iter().map(|(symbol, balance)| {
                let low_stock = engine.chains.values()
                    .find(|cs| cs.symbol == *symbol)
                    .and_then(|cs| cs.low_stock_threshold.as_ref())
                    .map(|thresh| balance < thresh)
                    .unwrap_or(false);
                PositionStatus {
                    symbol: symbol.clone(),
                    balance: balance.to_string(),
                    low_stock,
                }
            }).collect()
        },
        pending_trades: engine.trades.pending_count(),
    })
}

// ── Trade History Query ───────────────────────────────────────────────

#[derive(Deserialize)]
struct TradesQueryParams {
    from: Option<i64>,
    to: Option<i64>,
    symbol: Option<String>,
    status: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Serialize)]
struct TradesResponse {
    trades: Vec<TradeRecord>,
    total: u64,
    pnl: Vec<ao_exchange::db::PairPnl>,
}

async fn handle_trades_query(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<TradesQueryParams>,
) -> Result<Json<TradesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let store = state.store.clone();
    let from = params.from;
    let to = params.to;
    let symbol = params.symbol.clone();
    let status = params.status.clone();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    tokio::task::spawn_blocking(move || {
        let store = store.lock()
            .map_err(|_| json_error(StatusCode::INTERNAL_SERVER_ERROR, "store lock poisoned"))?;

        let q = TradeQuery {
            from_secs: from,
            to_secs: to,
            symbol,
            status,
            limit,
            offset,
        };

        let trades = store.query_trades(&q)
            .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let total = store.count_trades(&q)
            .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let pnl = store.pair_pnl(from, to)
            .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        Ok(Json(TradesResponse { trades, total, pnl }))
    })
    .await
    .map_err(|e| json_error(StatusCode::INTERNAL_SERVER_ERROR, format!("task join: {}", e)))?
}

async fn show_status(config_path: &str) -> Result<()> {
    let cfg = config::load_config(config_path)?;
    let engine = ExchangeEngine::from_config(&cfg).await
        .context("failed to initialize exchange engine")?;

    println!("Exchange Agent Status");
    println!("=====================");
    println!();

    println!("Trading Pairs:");
    for pair in &engine.pairs {
        println!(
            "  {} → {} (rate: {:.4}, spread: {:.1}%)",
            pair.buy_symbol, pair.sell_symbol, pair.rate, pair.spread * 100.0
        );
    }
    println!();

    println!("Positions:");
    for (symbol, balance) in engine.positions() {
        println!("  {}: {} shares", symbol, balance);
    }

    Ok(())
}
