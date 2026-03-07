use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tracing::info;

use ao_exchange::config;
use ao_exchange::engine::ExchangeEngine;

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
    },
    /// Show current exchange agent status (positions, pairs).
    Status {
        /// Path to config TOML file.
        config: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    match cli.command {
        Command::Run { config: config_path } => run_daemon(&config_path).await,
        Command::Status { config: config_path } => show_status(&config_path).await,
    }
}

async fn run_daemon(config_path: &str) -> Result<()> {
    let cfg = config::load_config(config_path)?;
    let engine = ExchangeEngine::from_config(&cfg).await
        .context("failed to initialize exchange engine")?;

    info!(
        pairs = engine.pairs.len(),
        chains = engine.chains.len(),
        "Exchange agent started"
    );

    // Display initial positions
    for (symbol, balance) in engine.positions() {
        info!(symbol = %symbol, balance = %balance, "Position");
    }

    // Main loop: poll chains for incoming UTXOs, match against trading rules.
    // For now, uses a simple polling loop. Will be upgraded to SSE/MQTT in future.
    let poll_interval = std::time::Duration::from_secs(5);

    loop {
        tokio::time::sleep(poll_interval).await;

        // Check each chain for new UTXOs addressed to our keys
        for (chain_id, chain_state) in &engine.chains {
            if let Err(e) = chain_state.client.chain_info(chain_id).await {
                tracing::warn!(chain = %chain_state.symbol, "poll failed: {}", e);
                continue;
            }

            let unspent = engine.wallet.find_all_unspent(chain_id);
            let balance = engine.wallet.balance(chain_id);
            tracing::debug!(
                chain = %chain_state.symbol,
                balance = %balance,
                utxos = unspent.len(),
                "Position check"
            );
        }
    }
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
