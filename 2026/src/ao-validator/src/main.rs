use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use axum::{Router, extract::{State, Path}, http::StatusCode, response::Json, routing::get};
use clap::{Parser, Subcommand};
use serde::Serialize;
use tracing::info;

use ao_validator::alert::{Alert, AlertDispatcher, AlertType};
use ao_validator::client::RecorderClient;
use ao_validator::config;
use ao_validator::metrics;
use ao_validator::store::ValidatorStore;
use ao_validator::verifier;

#[derive(Parser)]
#[command(name = "ao-validator", about = "Assign Onward chain validator daemon")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the validator daemon.
    Run {
        /// Path to config TOML file.
        config: String,
    },
    /// Show current validation status.
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

/// Shared state for the validator daemon.
/// Uses std::sync::Mutex for SQLite (not Send/Sync-safe for tokio::sync).
struct ValidatorState {
    store: Mutex<ValidatorStore>,
    alerts: AlertDispatcher,
}

async fn run_daemon(config_path: &str) -> Result<()> {
    let cfg = config::load_config(config_path)?;
    let store = ValidatorStore::open(&cfg.db_path)
        .context("failed to open validator database")?;

    // Initialize chain states for any not yet tracked
    for chain_cfg in &cfg.chains {
        if store.get_chain_state(&chain_cfg.chain_id)?.is_none() {
            store.update_chain_state(&chain_cfg.chain_id, 0, &[0u8; 32], "ok", None)?;
            info!(chain = %chain_cfg.chain_id, "Initialized tracking");
        }
    }

    let state = Arc::new(ValidatorState {
        store: Mutex::new(store),
        alerts: AlertDispatcher::new(cfg.webhook_url.clone()),
    });

    info!(chains = cfg.chains.len(), "Validator started");

    // Start the validation API server in background
    let api_state = Arc::clone(&state);
    let api_host = cfg.host.clone();
    let api_port = cfg.port;
    tokio::spawn(async move {
        if let Err(e) = run_api_server(api_state, &api_host, api_port).await {
            tracing::error!("API server failed: {}", e);
        }
    });

    // Pre-build HTTP clients per recorder URL (avoid re-allocation each poll cycle)
    let clients: std::collections::HashMap<String, RecorderClient> = cfg.chains.iter()
        .map(|c| (c.recorder_url.clone(), RecorderClient::new(&c.recorder_url)))
        .collect();

    // Main polling loop
    let poll_interval = std::time::Duration::from_secs(cfg.poll_interval_secs);

    loop {
        for chain_cfg in &cfg.chains {
            let label = chain_cfg.label.as_deref().unwrap_or(&chain_cfg.chain_id);
            let client = &clients[&chain_cfg.recorder_url];

            // Get current recorder height
            let recorder_height = match client.chain_info(&chain_cfg.chain_id).await {
                Ok(info) => info.block_height,
                Err(e) => {
                    let (was_ok, prev) = {
                        let store = state.store.lock().map_err(|e| anyhow::anyhow!("store lock poisoned: {}", e))?;
                        let prev = store.get_chain_state(&chain_cfg.chain_id)?;
                        let was_ok = prev.as_ref().is_none_or(|s| s.status == "ok");
                        (was_ok, prev)
                    };

                    if was_ok {
                        let alert = Alert {
                            chain_id: chain_cfg.chain_id.clone(),
                            alert_type: AlertType::Unreachable,
                            height: 0,
                            message: e.to_string(),
                            timestamp: unix_now(),
                        };
                        state.alerts.dispatch(&alert).await;
                        metrics::record_alert("unreachable");
                    }
                    metrics::record_run(label, "unreachable");

                    if let Some(prev) = prev {
                        let store = state.store.lock().map_err(|e| anyhow::anyhow!("store lock poisoned: {}", e))?;
                        store.update_chain_state(
                            &chain_cfg.chain_id, prev.validated_height,
                            &prev.rolled_hash, "unreachable",
                            Some(&e.to_string()),
                        )?;
                    }
                    continue;
                }
            };

            // Load our state
            let chain_state = {
                let store = state.store.lock().map_err(|e| anyhow::anyhow!("store lock poisoned: {}", e))?;
                store.get_chain_state(&chain_cfg.chain_id)?
                    .unwrap_or_else(|| ao_validator::store::ChainValidationState {
                        chain_id: chain_cfg.chain_id.clone(),
                        validated_height: 0,
                        rolled_hash: [0u8; 32],
                        last_poll_timestamp: 0,
                        status: "ok".to_string(),
                        alert_message: None,
                    })
            };

            // If previously unreachable, send recovery alert
            if chain_state.status == "unreachable" {
                let alert = Alert {
                    chain_id: chain_cfg.chain_id.clone(),
                    alert_type: AlertType::Recovered,
                    height: recorder_height,
                    message: "recorder reachable again".to_string(),
                    timestamp: unix_now(),
                };
                state.alerts.dispatch(&alert).await;
                metrics::record_alert("recovered");
            }

            let validated = chain_state.validated_height;

            if recorder_height <= validated {
                tracing::debug!(chain = %label, height = validated, "up to date");
                let store = state.store.lock().map_err(|e| anyhow::anyhow!("store lock poisoned: {}", e))?;
                store.update_chain_state(
                    &chain_cfg.chain_id, validated,
                    &chain_state.rolled_hash, "ok", None,
                )?;
                continue;
            }

            // Fetch and verify new blocks in batches of 1000
            let mut current_height = validated + 1;
            let mut rolled = chain_state.rolled_hash;
            let mut verification_ok = true;
            let verify_start = std::time::Instant::now();

            while current_height <= recorder_height {
                let batch_end = (current_height + 999).min(recorder_height);

                let blocks = match client.get_blocks(
                    &chain_cfg.chain_id, current_height, batch_end,
                ).await {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::warn!(
                            chain = %label,
                            "failed to fetch blocks {}-{}: {}",
                            current_height, batch_end, e
                        );
                        verification_ok = false;
                        break;
                    }
                };

                if blocks.is_empty() {
                    break;
                }

                match verifier::verify_block_batch(&blocks, current_height, &rolled) {
                    Ok(result) => {
                        rolled = result.rolled_hash;
                        current_height = result.last_height + 1;
                        metrics::record_blocks_verified(label, result.count);
                        info!(
                            chain = %label,
                            verified = result.count,
                            through = result.last_height,
                            "Blocks verified"
                        );
                    }
                    Err(e) => {
                        let alert = Alert {
                            chain_id: chain_cfg.chain_id.clone(),
                            alert_type: AlertType::Alteration,
                            height: current_height,
                            message: e.to_string(),
                            timestamp: unix_now(),
                        };
                        state.alerts.dispatch(&alert).await;
                        metrics::record_alert("alteration");
                        metrics::record_run(label, "alteration");

                        let store = state.store.lock().map_err(|e| anyhow::anyhow!("store lock poisoned: {}", e))?;
                        store.update_chain_state(
                            &chain_cfg.chain_id,
                            current_height.saturating_sub(1),
                            &rolled, "alert",
                            Some(&e.to_string()),
                        )?;
                        verification_ok = false;
                        break;
                    }
                }
            }

            if verification_ok {
                let final_height = current_height.saturating_sub(1);
                metrics::record_run(label, "ok");
                metrics::record_verify_duration(label, verify_start.elapsed().as_secs_f64());
                metrics::set_validated_height(label, final_height);
                let store = state.store.lock().map_err(|e| anyhow::anyhow!("store lock poisoned: {}", e))?;
                store.update_chain_state(
                    &chain_cfg.chain_id,
                    final_height,
                    &rolled, "ok", None,
                )?;
            }
        }

        tokio::time::sleep(poll_interval).await;
    }
}

// ── Validation API ──────────────────────────────────────────────────

#[derive(Serialize)]
struct ValidationStatus {
    chain_id: String,
    validated_height: u64,
    rolled_hash: String,
    last_poll: i64,
    status: String,
    alert_message: Option<String>,
    latest_anchor: Option<AnchorInfo>,
}

#[derive(Serialize)]
struct AnchorInfo {
    height: u64,
    rolled_hash: String,
    anchor_ref: String,
    anchor_timestamp: i64,
}

async fn get_validation_status(
    State(state): State<Arc<ValidatorState>>,
    Path(chain_id): Path<String>,
) -> Result<Json<ValidationStatus>, StatusCode> {
    let store = state.store.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let chain_state = store.get_chain_state(&chain_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let anchor = store.get_latest_anchor(&chain_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(ValidationStatus {
        chain_id: chain_state.chain_id,
        validated_height: chain_state.validated_height,
        rolled_hash: hex::encode(chain_state.rolled_hash),
        last_poll: chain_state.last_poll_timestamp,
        status: chain_state.status,
        alert_message: chain_state.alert_message,
        latest_anchor: anchor.map(|a| AnchorInfo {
            height: a.height,
            rolled_hash: hex::encode(a.rolled_hash),
            anchor_ref: a.anchor_ref,
            anchor_timestamp: a.anchor_timestamp,
        }),
    }))
}

async fn list_all_status(
    State(state): State<Arc<ValidatorState>>,
) -> Result<Json<Vec<ValidationStatus>>, StatusCode> {
    let store = state.store.lock().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let ids = store.all_chain_ids()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut results = Vec::new();
    for id in ids {
        if let Some(cs) = store.get_chain_state(&id)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            let anchor = store.get_latest_anchor(&id)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            results.push(ValidationStatus {
                chain_id: cs.chain_id,
                validated_height: cs.validated_height,
                rolled_hash: hex::encode(cs.rolled_hash),
                last_poll: cs.last_poll_timestamp,
                status: cs.status,
                alert_message: cs.alert_message,
                latest_anchor: anchor.map(|a| AnchorInfo {
                    height: a.height,
                    rolled_hash: hex::encode(a.rolled_hash),
                    anchor_ref: a.anchor_ref,
                    anchor_timestamp: a.anchor_timestamp,
                }),
            });
        }
    }
    Ok(Json(results))
}

async fn run_api_server(
    state: Arc<ValidatorState>,
    host: &str,
    port: u16,
) -> Result<()> {
    let app = Router::new()
        .route("/validate", get(list_all_status))
        .route("/validate/{chain_id}", get(get_validation_status))
        .route("/metrics", get(ao_validator::metrics::metrics_handler))
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    info!(addr = %addr, "Validation API listening");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// ── Status command ──────────────────────────────────────────────────

async fn show_status(config_path: &str) -> Result<()> {
    let cfg = config::load_config(config_path)?;
    let store = ValidatorStore::open(&cfg.db_path)
        .context("failed to open validator database")?;

    println!("Validator Status");
    println!("================");
    println!();

    for chain_cfg in &cfg.chains {
        let label = chain_cfg.label.as_deref().unwrap_or(&chain_cfg.chain_id);
        match store.get_chain_state(&chain_cfg.chain_id)? {
            Some(state) => {
                println!("  {} ({})", label, chain_cfg.chain_id);
                println!("    Status:          {}", state.status);
                println!("    Validated height: {}", state.validated_height);
                println!("    Rolled hash:      {}", hex::encode(state.rolled_hash));
                if let Some(msg) = &state.alert_message {
                    println!("    Alert:           {}", msg);
                }
                if let Some(anchor) = store.get_latest_anchor(&chain_cfg.chain_id)? {
                    println!("    Latest anchor:   height {} ({})", anchor.height, anchor.anchor_ref);
                }
            }
            None => {
                println!("  {} — not yet tracked", label);
            }
        }
        println!();
    }

    Ok(())
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
