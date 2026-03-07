mod agents;
mod client;
mod config;
mod mqtt;
mod observer;
mod transfer;
mod viewer;
mod wallet;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64};

use anyhow::{Context, Result, bail};
use clap::Parser;
use tokio::sync::{mpsc, RwLock, watch};
use tracing::info;

use ao_crypto::sign::SigningKey;
use ao_recorder::{AppState, build_router};

use agents::{AgentDirectory, AgentMessage, PauseFlag, SharedSpeed, ViewerState};
use client::RecorderClient;
use config::ScenarioConfig;

/// Shared map of agent name → pause flag, accessible from the viewer API.
pub type PauseFlags = Arc<RwLock<HashMap<String, PauseFlag>>>;

#[derive(Parser)]
#[command(name = "ao-sims", about = "Assign Onward community simulator")]
struct Cli {
    /// Path to scenario TOML file
    #[arg(default_value = "scenarios/minimal.toml")]
    scenario: String,

    /// Viewer API port (0 = auto-assign)
    #[arg(long, default_value = "4200")]
    viewer_port: u16,
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
    let (recorder_url, recorder_state) = start_recorder(scenario.simulation.recorder_port).await;
    info!("Recorder running at {}", recorder_url);

    // Optionally start MQTT broker for block notifications
    let mqtt_port = scenario.simulation.mqtt_port;
    if mqtt_port > 0 {
        mqtt::start_broker(mqtt_port);
        mqtt::connect_recorder_publisher(mqtt_port, &recorder_state);
    }

    let client = Arc::new(RecorderClient::new(&recorder_url));
    let directory = Arc::new(RwLock::new(AgentDirectory::new()));
    let (state_tx, state_rx) = mpsc::channel(256);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let speed: SharedSpeed = Arc::new(AtomicU64::new(scenario.simulation.speed.to_bits()));
    let pause_flags: PauseFlags = Arc::new(RwLock::new(HashMap::new()));

    // Start viewer state + API server
    let viewer_state = Arc::new(ViewerState::new());
    let viewer_url = start_viewer(cli.viewer_port, viewer_state.clone(), Arc::clone(&speed), Arc::clone(&pause_flags)).await;
    info!("Viewer API running at {}", viewer_url);

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
    let observer_handle = tokio::spawn(observer::run_observer(state_rx, viewer_state.clone(), shutdown_rx));

    // Spawn agents
    let mut agent_handles = Vec::new();

    for agent_cfg in scenario.agents {
        let mailbox = mailbox_receivers.remove(&agent_cfg.name).unwrap();
        let client = Arc::clone(&client);
        let directory = Arc::clone(&directory);
        let state_tx = state_tx.clone();
        let speed = Arc::clone(&speed);
        let name = agent_cfg.name.clone();
        let agent_paused: PauseFlag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = pause_flags.blocking_write();
            flags.insert(name.clone(), Arc::clone(&agent_paused));
        }

        let handle = match agent_cfg.role.as_str() {
            "vendor" => {
                let Some(vendor_cfg) = agent_cfg.vendor.clone() else {
                    bail!("vendor {} missing [agent.vendor] config", name);
                };
                tokio::spawn(async move {
                    if let Err(e) = agents::run_vendor(
                        agent_cfg, vendor_cfg, client, directory, state_tx, mailbox, agent_paused,
                    ).await {
                        tracing::error!("{}: vendor error: {}", name, e);
                    }
                })
            }
            "exchange" => {
                let Some(exchange_cfg) = agent_cfg.exchange.clone() else {
                    bail!("exchange {} missing [agent.exchange] config", name);
                };
                let block_rx = if mqtt_port > 0 {
                    Some(mqtt::subscribe_blocks(mqtt_port, &format!("exchange-{}", name)).await)
                } else {
                    None
                };
                tokio::spawn(async move {
                    if let Err(e) = agents::run_exchange(
                        agent_cfg, exchange_cfg, client, directory, state_tx, mailbox, speed, block_rx, agent_paused,
                    ).await {
                        tracing::error!("{}: exchange error: {}", name, e);
                    }
                })
            }
            "consumer" => {
                let Some(consumer_cfg) = agent_cfg.consumer.clone() else {
                    bail!("consumer {} missing [agent.consumer] config", name);
                };
                tokio::spawn(async move {
                    if let Err(e) = agents::run_consumer(
                        agent_cfg, consumer_cfg, client, directory, state_tx, mailbox, speed, agent_paused,
                    ).await {
                        tracing::error!("{}: consumer error: {}", name, e);
                    }
                })
            }
            "validator" => {
                let Some(validator_cfg) = agent_cfg.validator.clone() else {
                    bail!("validator {} missing [agent.validator] config", name);
                };
                tokio::spawn(async move {
                    if let Err(e) = agents::run_validator(
                        agent_cfg, validator_cfg, client, directory, state_tx, mailbox, speed, agent_paused,
                    ).await {
                        tracing::error!("{}: validator error: {}", name, e);
                    }
                })
            }
            "attacker" => {
                let Some(attacker_cfg) = agent_cfg.attacker.clone() else {
                    bail!("attacker {} missing [agent.attacker] config", name);
                };
                tokio::spawn(async move {
                    if let Err(e) = agents::run_attacker(
                        agent_cfg, attacker_cfg, client, directory, state_tx, mailbox, speed, agent_paused,
                    ).await {
                        tracing::error!("{}: attacker error: {}", name, e);
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
    info!("Simulation running for {:?} (speed {}x)...", duration, agents::read_speed(&speed));
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

/// Start an embedded ao-recorder server. Returns the base URL and shared state.
async fn start_recorder(port: u16) -> (String, Arc<AppState>) {
    let blockmaker_key = SigningKey::generate();
    let state = Arc::new(AppState::new_multi(None, blockmaker_key));
    let app = build_router(state.clone());

    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .expect("failed to bind recorder");
    let actual_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (format!("http://{}", actual_addr), state)
}

/// Start the viewer API server. Returns the base URL.
async fn start_viewer(
    port: u16,
    viewer_state: Arc<ViewerState>,
    speed: SharedSpeed,
    pause_flags: PauseFlags,
) -> String {
    let app_state = viewer::ViewerAppState {
        viewer: viewer_state,
        speed,
        pause_flags,
    };
    let app = viewer::build_viewer_router(app_state);

    let bind_addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .expect("failed to bind viewer");
    let actual_addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    format!("http://{}", actual_addr)
}
