mod agents;
mod client;
mod config;
mod observer;
mod transfer;
mod wallet;

use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::{mpsc, RwLock, watch};
use tracing::info;

use ao_crypto::sign::SigningKey;
use ao_recorder::{AppState, build_router};

use agents::{AgentDirectory, AgentMessage};
use client::RecorderClient;
use config::ScenarioConfig;

#[derive(Parser)]
#[command(name = "ao-sims", about = "Assign Onward community simulator")]
struct Cli {
    /// Path to scenario TOML file
    #[arg(default_value = "scenarios/minimal.toml")]
    scenario: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();
    let scenario_str = std::fs::read_to_string(&cli.scenario)
        .context(format!("failed to read scenario file: {}", cli.scenario))?;
    let scenario: ScenarioConfig = toml::from_str(&scenario_str)
        .context("failed to parse scenario TOML")?;

    info!("Loaded scenario: {} ({} agents)", scenario.simulation.name, scenario.agents.len());

    // Start embedded recorder
    let recorder_url = start_recorder(scenario.simulation.recorder_port).await;
    info!("Recorder running at {}", recorder_url);

    let client = Arc::new(RecorderClient::new(&recorder_url));
    let directory = Arc::new(RwLock::new(AgentDirectory::new()));
    let (state_tx, state_rx) = mpsc::channel(256);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    // Register agent mailboxes
    let mut mailbox_senders: std::collections::HashMap<String, mpsc::Sender<AgentMessage>> = std::collections::HashMap::new();
    let mut mailbox_receivers: std::collections::HashMap<String, mpsc::Receiver<AgentMessage>> = std::collections::HashMap::new();

    for agent_cfg in &scenario.agents {
        let (tx, rx) = mpsc::channel(64);
        {
            let mut dir = directory.write().await;
            dir.register(&agent_cfg.name, tx.clone());
        }
        mailbox_senders.insert(agent_cfg.name.clone(), tx);
        mailbox_receivers.insert(agent_cfg.name.clone(), rx);
    }

    // Spawn observer
    let observer_handle = tokio::spawn(observer::run_observer(state_rx, shutdown_rx));

    // Spawn agents
    let speed = scenario.simulation.speed;
    let mut agent_handles = Vec::new();

    for agent_cfg in scenario.agents {
        let mailbox = mailbox_receivers.remove(&agent_cfg.name).unwrap();
        let client = Arc::clone(&client);
        let directory = Arc::clone(&directory);
        let state_tx = state_tx.clone();
        let name = agent_cfg.name.clone();

        let handle = match agent_cfg.role.as_str() {
            "vendor" => {
                let vendor_cfg = agent_cfg.vendor.clone()
                    .unwrap_or_else(|| panic!("vendor {} missing [agent.vendor] config", name));
                tokio::spawn(async move {
                    if let Err(e) = agents::run_vendor(
                        agent_cfg, vendor_cfg, client, directory, state_tx, mailbox,
                    ).await {
                        tracing::error!("{}: vendor error: {}", name, e);
                    }
                })
            }
            "exchange" => {
                let exchange_cfg = agent_cfg.exchange.clone()
                    .unwrap_or_else(|| panic!("exchange {} missing [agent.exchange] config", name));
                tokio::spawn(async move {
                    if let Err(e) = agents::run_exchange(
                        agent_cfg, exchange_cfg, client, directory, state_tx, mailbox, speed,
                    ).await {
                        tracing::error!("{}: exchange error: {}", name, e);
                    }
                })
            }
            "consumer" => {
                let consumer_cfg = agent_cfg.consumer.clone()
                    .unwrap_or_else(|| panic!("consumer {} missing [agent.consumer] config", name));
                tokio::spawn(async move {
                    if let Err(e) = agents::run_consumer(
                        agent_cfg, consumer_cfg, client, directory, state_tx, mailbox, speed,
                    ).await {
                        tracing::error!("{}: consumer error: {}", name, e);
                    }
                })
            }
            "recorder" => {
                // Recorder is embedded — no agent loop needed.
                info!("{}: recorder (embedded, no agent loop)", name);
                continue;
            }
            other => {
                tracing::warn!("Unknown agent role: {} ({}), skipping", other, name);
                continue;
            }
        };

        agent_handles.push(handle);
    }

    // Run for the configured duration
    let duration = std::time::Duration::from_secs(scenario.simulation.duration_secs);
    info!("Simulation running for {:?} (speed {}x)...", duration, speed);
    info!("Press Ctrl+C to stop early.");

    tokio::select! {
        _ = tokio::time::sleep(duration) => {
            info!("Simulation duration reached.");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Ctrl+C received, shutting down...");
        }
    }

    // Shutdown
    let _ = shutdown_tx.send(true);
    drop(state_tx); // close observer channel

    // Give observer a moment to print final state
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Abort agent tasks
    for handle in agent_handles {
        handle.abort();
    }

    let _ = observer_handle.await;

    info!("Simulation complete.");
    Ok(())
}

/// Start an embedded ao-recorder server. Returns the base URL.
async fn start_recorder(port: u16) -> String {
    let blockmaker_key = SigningKey::generate();
    let state = Arc::new(AppState::new_multi(None, blockmaker_key));
    let app = build_router(state);

    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .expect("failed to bind recorder");
    let actual_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{}", actual_addr)
}
