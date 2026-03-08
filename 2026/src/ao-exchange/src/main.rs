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
use ao_exchange::engine::ExchangeEngine;

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

    let engine = ExchangeEngine::from_config(&cfg).await
        .context("failed to initialize exchange engine")?;

    info!(
        pairs = engine.pairs.len(),
        chains = engine.chains.len(),
        poll_secs = cfg.poll_interval_secs,
        trade_ttl = cfg.trade_ttl_secs,
        "Exchange agent started"
    );

    for (symbol, balance) in engine.positions() {
        info!(symbol = %symbol, balance = %balance, "Position");
    }

    let shared = Arc::new(Mutex::new(engine));

    // HTTP API with body size limit
    let app = Router::new()
        .route("/trade", post(handle_trade_request))
        .route("/status", get(handle_status))
        .layer(RequestBodyLimitLayer::new(MAX_BODY_SIZE))
        .with_state(shared.clone());

    let listener = tokio::net::TcpListener::bind(listen).await
        .context("failed to bind HTTP listener")?;
    info!(listen = %listen, "HTTP API listening");

    // Spawn HTTP server
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("HTTP server error: {}", e);
        }
    });

    // Main polling loop
    loop {
        tokio::time::sleep(poll_interval).await;

        let mut engine = shared.lock().await;
        let results = engine.check_deposits().await;

        if !results.is_empty() {
            info!(trades = results.len(), "Poll cycle completed trades");
        }

        // Log positions periodically
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
    State(engine): State<SharedEngine>,
    Json(req): Json<TradeRequest>,
) -> Result<Json<TradeResponse>, (StatusCode, Json<ErrorResponse>)> {
    let amount: num_bigint::BigInt = req.amount.parse()
        .map_err(|e| json_error(StatusCode::BAD_REQUEST, format!("invalid amount: {}", e)))?;

    let mut engine = engine.lock().await;
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
}

async fn handle_status(
    State(engine): State<SharedEngine>,
) -> Json<StatusResponse> {
    let engine = engine.lock().await;
    Json(StatusResponse {
        pairs: engine.pairs.iter().map(|p| PairStatus {
            sell: p.sell_symbol.clone(),
            buy: p.buy_symbol.clone(),
            rate: p.rate,
            spread: p.spread,
        }).collect(),
        positions: engine.positions().iter().map(|(s, b)| PositionStatus {
            symbol: s.clone(),
            balance: b.to_string(),
        }).collect(),
        pending_trades: engine.trades.pending_count(),
    })
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
