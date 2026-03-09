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
use ao_recorder::{AppState, build_router, build_router_with_config};

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

/// Pre-generated genesis data for a vendor, computed before recorder startup
/// so that chain_ids are known for known_recorders CAA configuration.
struct PreGenesis {
    vendor_name: String,
    genesis_json: serde_json::Value,
    issuer_seed: [u8; 32],
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

    // Pre-generate genesis for all vendors so we can compute chain_ids
    // before creating AppState (needed for known_recorders CAA config).
    let blockmaker_key = SigningKey::generate();
    let blockmaker_pubkey: [u8; 32] = blockmaker_key.public_key_bytes().try_into()
        .expect("blockmaker pubkey must be 32 bytes");

    let mut pre_genesis_list: Vec<PreGenesis> = Vec::new();
    let mut known_recorders: HashMap<[u8; 32], [u8; 32]> = HashMap::new();

    for agent_cfg in &scenario.agents {
        if agent_cfg.role != "vendor" {
            continue;
        }
        let Some(vendor_cfg) = &agent_cfg.vendor else {
            continue;
        };

        let issuer_key = SigningKey::generate();
        let issuer_seed = *issuer_key.seed();

        let coins = config::parse_bigint(&vendor_cfg.coins);
        let shares = config::parse_bigint(&vendor_cfg.shares);
        let fee_num = config::parse_bigint(&vendor_cfg.fee_num);
        let fee_den = vendor_cfg.fee_den.as_ref()
            .map(|s| config::parse_bigint(s))
            .unwrap_or_else(|| config::auto_fee_den(&vendor_cfg.coins));
        let fee_rate = transfer::FeeRate { num: fee_num, den: fee_den };

        let (genesis_item, genesis_json) = transfer::build_genesis(
            &issuer_seed, &vendor_cfg.symbol, &vendor_cfg.description,
            &coins, &shares, &fee_rate,
        );

        // Compute chain_id from genesis DataItem
        let chain_id = ao_chain::genesis::compute_chain_id(&genesis_item)
            .context(format!("failed to compute chain_id for vendor {}", agent_cfg.name))?;

        info!("{}: Pre-generated genesis for {} (chain {})",
            agent_cfg.name, vendor_cfg.symbol, &hex::encode(chain_id)[..12]);

        // Register this chain_id → blockmaker pubkey for CAA proof verification
        known_recorders.insert(chain_id, blockmaker_pubkey);

        pre_genesis_list.push(PreGenesis {
            vendor_name: agent_cfg.name.clone(),
            genesis_json,
            issuer_seed,
        });
    }

    // Start embedded recorder with known_recorders for CAA support
    let (recorder_url, recorder_state) = start_recorder(
        scenario.simulation.recorder_port,
        blockmaker_key,
        known_recorders,
        scenario.simulation.recorder_security.as_ref(),
    ).await;
    info!("Recorder running at {}", recorder_url);

    // Optionally start a secondary recorder for dual-recorder scenarios (Sim-G)
    let secondary_recorder_port = scenario.simulation.secondary_recorder_port;
    let (secondary_recorder_url, secondary_recorder_pubkey) = if secondary_recorder_port > 0 {
        let sec_key = SigningKey::generate();
        let sec_pubkey: [u8; 32] = sec_key.public_key_bytes().try_into()
            .expect("secondary blockmaker pubkey must be 32 bytes");
        let (sec_url, _sec_state) = start_recorder(
            secondary_recorder_port,
            sec_key,
            known_recorders.clone(),
            scenario.simulation.recorder_security.as_ref(),
        ).await;
        info!("Secondary recorder running at {}", sec_url);
        (Some(sec_url), Some(sec_pubkey))
    } else {
        (None, None)
    };

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

    // Build scenario metadata for viewer onboarding
    let scenario_meta = viewer::ScenarioMeta {
        name: scenario.simulation.name.clone(),
        title: scenario.simulation.title.clone(),
        description: scenario.simulation.description.clone(),
        what_to_watch: scenario.simulation.what_to_watch.clone(),
        agents: scenario.agents.iter().map(|a| viewer::AgentMeta {
            name: a.name.clone(),
            role: a.role.clone(),
            blurb: a.blurb.clone(),
        }).collect(),
    };

    // Start viewer state + API server
    let viewer_state = Arc::new(ViewerState::new());
    let viewer_url = start_viewer(cli.viewer_port, viewer_state.clone(), Arc::clone(&speed), Arc::clone(&pause_flags), scenario_meta).await;
    info!("Viewer API running at {}", viewer_url);

    // Build pre-genesis lookup: vendor_name → (genesis_json, issuer_seed)
    let mut pre_genesis_map: HashMap<String, (serde_json::Value, [u8; 32])> = HashMap::new();
    for pg in pre_genesis_list {
        pre_genesis_map.insert(pg.vendor_name, (pg.genesis_json, pg.issuer_seed));
    }

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
    let recorder_url_shared = recorder_url.clone();

    for agent_cfg in scenario.agents {
        let mailbox = mailbox_receivers.remove(&agent_cfg.name).unwrap();
        let client = Arc::clone(&client);
        let directory = Arc::clone(&directory);
        let state_tx = state_tx.clone();
        let speed = Arc::clone(&speed);
        let name = agent_cfg.name.clone();
        let agent_paused: PauseFlag = Arc::new(AtomicBool::new(false));
        {
            let mut flags = pause_flags.write().await;
            flags.insert(name.clone(), Arc::clone(&agent_paused));
        }

        let handle = match agent_cfg.role.as_str() {
            "vendor" => {
                let Some(vendor_cfg) = agent_cfg.vendor.clone() else {
                    bail!("vendor {} missing [agent.vendor] config", name);
                };
                let Some(pre_genesis) = pre_genesis_map.remove(&name) else {
                    bail!("vendor {} missing pre-generated genesis", name);
                };
                tokio::spawn(async move {
                    if let Err(e) = agents::run_vendor(
                        agent_cfg, vendor_cfg, client, directory, state_tx, mailbox, agent_paused, pre_genesis,
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
                let recorder_url = recorder_url_shared.clone();
                tokio::spawn(async move {
                    if let Err(e) = agents::run_exchange(
                        agent_cfg, exchange_cfg, client, directory, state_tx, mailbox, speed, block_rx, agent_paused, recorder_url,
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
                let atk_recorder = if attacker_cfg.attack == "chain_tamper" {
                    Some(Arc::clone(&recorder_state))
                } else {
                    None
                };
                tokio::spawn(async move {
                    if let Err(e) = agents::run_attacker(
                        agent_cfg, attacker_cfg, client, directory, state_tx, mailbox, speed, agent_paused, atk_recorder,
                    ).await {
                        tracing::error!("{}: attacker error: {}", name, e);
                    }
                })
            }
            "infra_tester" => {
                let Some(infra_cfg) = agent_cfg.infra_tester.clone() else {
                    bail!("infra_tester {} missing [agent.infra_tester] config", name);
                };
                let recorder_url = recorder_url_shared.clone();
                tokio::spawn(async move {
                    if let Err(e) = agents::run_infra_tester(
                        agent_cfg, infra_cfg, recorder_url, directory, state_tx, mailbox, speed, agent_paused,
                    ).await {
                        tracing::error!("{}: infra_tester error: {}", name, e);
                    }
                })
            }
            "recorder_operator" => {
                let Some(op_cfg) = agent_cfg.recorder_operator.clone() else {
                    bail!("recorder_operator {} missing [agent.recorder_operator] config", name);
                };
                let sec_client = secondary_recorder_url.as_ref().map(|url| Arc::new(RecorderClient::new(url)));
                let sec_pk = secondary_recorder_pubkey;
                let sec_url = secondary_recorder_url.clone();
                tokio::spawn(async move {
                    if let Err(e) = agents::run_recorder_operator(
                        agent_cfg, op_cfg, client, sec_client, sec_pk, sec_url,
                        directory, state_tx, mailbox, speed, agent_paused,
                    ).await {
                        tracing::error!("{}: recorder_operator error: {}", name, e);
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
/// When `security` is provided, configures N10 features (API keys, rate limits, etc.).
async fn start_recorder(
    port: u16,
    blockmaker_key: SigningKey,
    known_recorders: HashMap<[u8; 32], [u8; 32]>,
    security: Option<&config::RecorderSecurityConfig>,
) -> (String, Arc<AppState>) {
    let mut state = AppState::new_multi(None, blockmaker_key);
    state.known_recorders = known_recorders.into();
    let state = Arc::new(state);

    let app = if let Some(sec) = security {
        let mut cfg = ao_recorder::config::Config::default();
        cfg.api_keys = sec.api_keys.clone();
        cfg.read_rate_limit = sec.read_rate_limit;
        cfg.write_rate_limit = sec.write_rate_limit;
        cfg.max_connections = sec.max_connections;
        info!("Recorder security: api_keys={}, read_rate={}, write_rate={}, max_conn={}",
            cfg.api_keys.len(), cfg.read_rate_limit, cfg.write_rate_limit, cfg.max_connections);
        build_router_with_config(state.clone(), &cfg)
    } else {
        build_router(state.clone())
    };

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
    scenario_meta: viewer::ScenarioMeta,
) -> String {
    let app_state = viewer::ViewerAppState {
        viewer: viewer_state,
        speed,
        pause_flags,
        scenario_meta,
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
