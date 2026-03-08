//! Health endpoint and operational monitoring for ao-recorder.
//!
//! Provides `GET /health` returning JSON system metrics, per-chain health,
//! and capacity estimates. Also provides periodic background alerts for
//! disk space, stale chains, and memory baseline logging.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::State;
use axum::response::{IntoResponse, Json};
use serde::Serialize;
use sysinfo::{Disks, System};

use crate::{AppState, RecorderError, lock_store};

/// Process start time, set once during server initialization.
static START_TIME: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Record the process start time. Call once from main before serving.
pub fn record_start_time() {
    let _ = START_TIME.set(Instant::now());
}

fn uptime_seconds() -> u64 {
    START_TIME.get().map(|t| t.elapsed().as_secs()).unwrap_or(0)
}

// ── Response types ──────────────────────────────────────────────────

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub uptime_seconds: u64,
    pub version: &'static str,
    pub chains: Vec<ChainHealth>,
    pub system: SystemHealth,
    pub capacity: CapacityEstimate,
}

#[derive(Serialize)]
pub struct ChainHealth {
    pub chain_id: String,
    pub symbol: String,
    pub block_height: u64,
    pub last_block_age_seconds: Option<i64>,
    pub utxo_count: u64,
    pub db_size_bytes: u64,
}

#[derive(Serialize)]
pub struct SystemHealth {
    pub ram_used_bytes: u64,
    pub ram_available_bytes: u64,
    pub cpu_load_percent: Option<f32>,
    pub disk_free_bytes: u64,
    pub disk_used_bytes: u64,
}

#[derive(Serialize)]
pub struct CapacityEstimate {
    pub estimated_assignments_per_second: Option<f64>,
    pub estimated_days_until_disk_full: Option<f64>,
}

// ── Handler ─────────────────────────────────────────────────────────

/// GET /health — system and per-chain health metrics.
pub async fn health(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, RecorderError> {
    // Collect chain refs under read lock
    let chain_refs: Vec<(String, Arc<crate::ChainState>)> = {
        let chains = state.chains.read()
            .map_err(|e| RecorderError::LockPoisoned(format!("chains read: {}", e)))?;
        chains.iter().map(|(id, cs)| (id.clone(), Arc::clone(cs))).collect()
    };

    let data_dir = state.data_dir.clone();

    // Query chain health on blocking pool
    let chain_health = crate::blocking(move || {
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let mut chains = Vec::new();
        for (id, cs) in &chain_refs {
            let store = lock_store(cs)?;
            let meta = store.load_chain_meta()
                .map_err(|e| RecorderError::Internal(e.to_string()))?;
            if let Some(meta) = meta {
                // Get UTXO count
                let utxo_count = store.count_utxos()
                    .unwrap_or(0);

                // Get DB file size
                let db_size = store.db_file_size().unwrap_or(0);

                let last_block_age = if meta.block_height > 0 {
                    store.last_block_timestamp()
                        .ok()
                        .flatten()
                        .map(|ts| now_unix - ts)
                } else {
                    None
                };

                chains.push(ChainHealth {
                    chain_id: id.clone(),
                    symbol: meta.symbol,
                    block_height: meta.block_height,
                    last_block_age_seconds: last_block_age,
                    utxo_count,
                    db_size_bytes: db_size,
                });
            }
        }
        chains.sort_by(|a, b| a.chain_id.cmp(&b.chain_id));
        Ok(chains)
    }).await?;

    // System metrics (quick, no blocking needed for sysinfo)
    let system_health = collect_system_health(data_dir.as_ref());

    // Capacity estimate
    let total_db_bytes: u64 = chain_health.iter().map(|c| c.db_size_bytes).sum();
    let capacity = CapacityEstimate {
        estimated_assignments_per_second: None, // filled by bench, not runtime
        estimated_days_until_disk_full: if total_db_bytes > 0 && system_health.disk_free_bytes > 0 {
            // Very rough: extrapolate from current usage assuming linear growth
            // This is a placeholder — real estimation needs growth rate tracking
            None
        } else {
            None
        },
    };

    // Determine overall status
    let status = if chain_health.is_empty() {
        "degraded"
    } else if system_health.disk_free_bytes < 100_000_000 {
        // Less than 100 MB free
        "error"
    } else if system_health.disk_free_bytes < 1_000_000_000 {
        // Less than 1 GB free
        "degraded"
    } else {
        "ok"
    };

    Ok(Json(HealthResponse {
        status,
        uptime_seconds: uptime_seconds(),
        version: env!("CARGO_PKG_VERSION"),
        chains: chain_health,
        system: system_health,
        capacity,
    }))
}

fn collect_system_health(data_dir: Option<&std::path::PathBuf>) -> SystemHealth {
    let mut sys = System::new();
    sys.refresh_memory();

    let ram_used = sys.used_memory();
    let ram_available = sys.available_memory();

    // CPU load — requires two refreshes with a gap, skip for now (returns None)
    let cpu_load = None;

    // Disk usage for data directory
    let (disk_free, disk_used) = if let Some(dir) = data_dir {
        disk_usage_for_path(dir)
    } else {
        (0, 0)
    };

    SystemHealth {
        ram_used_bytes: ram_used,
        ram_available_bytes: ram_available,
        cpu_load_percent: cpu_load,
        disk_free_bytes: disk_free,
        disk_used_bytes: disk_used,
    }
}

fn disk_usage_for_path(path: &std::path::Path) -> (u64, u64) {
    let disks = Disks::new_with_refreshed_list();
    // Find the disk that contains our path
    let mut best_match: Option<&sysinfo::Disk> = None;
    let mut best_len = 0;

    for disk in disks.list() {
        let mount = disk.mount_point();
        if path.starts_with(mount) {
            let len = mount.as_os_str().len();
            if len > best_len {
                best_len = len;
                best_match = Some(disk);
            }
        }
    }

    if let Some(disk) = best_match {
        (disk.available_space(), disk.total_space() - disk.available_space())
    } else {
        (0, 0)
    }
}

// ── Background alerts ───────────────────────────────────────────────

/// Alert configuration from TOML.
#[derive(Clone)]
pub struct AlertConfig {
    pub disk_warn_percent: f64,
    pub disk_error_percent: f64,
    pub stale_chain_seconds: u64,
    pub memory_log_interval_seconds: u64,
    pub check_interval_seconds: u64,
    pub webhook_url: Option<String>,
}

impl AlertConfig {
    /// Build from parsed TOML config.
    pub fn from_config(cfg: &crate::config::Config) -> Self {
        if let Some(alerts) = &cfg.alerts {
            AlertConfig {
                disk_warn_percent: alerts.disk_warn_percent,
                disk_error_percent: alerts.disk_error_percent,
                stale_chain_seconds: alerts.stale_chain_seconds,
                memory_log_interval_seconds: alerts.memory_log_interval_seconds,
                check_interval_seconds: alerts.check_interval_seconds,
                webhook_url: alerts.webhook_url.clone(),
            }
        } else {
            AlertConfig::default()
        }
    }
}

impl Default for AlertConfig {
    fn default() -> Self {
        AlertConfig {
            disk_warn_percent: 10.0,
            disk_error_percent: 5.0,
            stale_chain_seconds: 86400, // 24 hours
            memory_log_interval_seconds: 3600, // 1 hour
            check_interval_seconds: 60,
            webhook_url: None,
        }
    }
}

/// Run periodic operational alerts. Call from a spawned task.
pub async fn run_alerts(state: Arc<AppState>, config: AlertConfig) {
    let interval = std::time::Duration::from_secs(config.check_interval_seconds);
    let mut memory_last_logged = Instant::now();

    // Log initial memory baseline
    log_memory_baseline();

    loop {
        tokio::time::sleep(interval).await;

        // Disk space check
        if let Some(ref dir) = state.data_dir {
            check_disk_space(dir, &config).await;
        }

        // Stale chain check
        check_stale_chains(&state, &config).await;

        // Memory baseline
        if memory_last_logged.elapsed().as_secs() >= config.memory_log_interval_seconds {
            log_memory_baseline();
            memory_last_logged = Instant::now();
        }
    }
}

fn log_memory_baseline() {
    let mut sys = System::new();
    sys.refresh_memory();
    let pid = sysinfo::get_current_pid().ok();
    let process_rss = pid.and_then(|p| {
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
        sys.process(p).map(|proc_| proc_.memory())
    });
    tracing::info!(
        rss_bytes = process_rss,
        system_used_bytes = sys.used_memory(),
        system_available_bytes = sys.available_memory(),
        "Memory baseline"
    );
}

async fn check_disk_space(dir: &std::path::Path, config: &AlertConfig) {
    let (free, used) = disk_usage_for_path(dir);
    let total = free + used;
    if total == 0 {
        return;
    }
    let free_percent = (free as f64 / total as f64) * 100.0;

    if free_percent < config.disk_error_percent {
        tracing::error!(
            free_percent = format!("{:.1}", free_percent),
            free_bytes = free,
            "Disk space critically low"
        );
        fire_webhook(&config.webhook_url, "disk_critical", &format!(
            "Disk space critically low: {:.1}% free ({} bytes)", free_percent, free
        )).await;
    } else if free_percent < config.disk_warn_percent {
        tracing::warn!(
            free_percent = format!("{:.1}", free_percent),
            free_bytes = free,
            "Disk space low"
        );
        fire_webhook(&config.webhook_url, "disk_warning", &format!(
            "Disk space low: {:.1}% free ({} bytes)", free_percent, free
        )).await;
    }
}

async fn check_stale_chains(state: &AppState, config: &AlertConfig) {
    let chain_refs: Vec<(String, Arc<crate::ChainState>)> = match state.chains.read() {
        Ok(chains) => chains.iter().map(|(id, cs)| (id.clone(), Arc::clone(cs))).collect(),
        Err(e) => {
            tracing::error!("chains read lock poisoned in stale check: {}", e);
            return;
        }
    };

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    for (id, cs) in &chain_refs {
        let age = {
            let store = match cs.store.lock() {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(chain_id = %id, "store lock poisoned in stale check: {}", e);
                    continue;
                }
            };
            store.last_block_timestamp()
                .ok()
                .flatten()
                .map(|ts| now_unix - ts)
        };

        if let Some(age) = age.filter(|&a| a > config.stale_chain_seconds as i64) {
            tracing::warn!(
                chain_id = %id,
                last_block_age_seconds = age,
                threshold_seconds = config.stale_chain_seconds,
                "Chain stale — no blocks recorded recently"
            );
            fire_webhook(&config.webhook_url, "chain_stale", &format!(
                "Chain {} stale: last block {} seconds ago (threshold: {})",
                id, age, config.stale_chain_seconds
            )).await;
        }
    }
}

async fn fire_webhook(url: &Option<String>, event: &str, message: &str) {
    let Some(url) = url else { return };
    let payload = serde_json::json!({
        "event": event,
        "message": message,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
    });
    // Fire and forget
    let url = url.clone();
    tokio::spawn(async move {
        let _ = reqwest::Client::new()
            .post(&url)
            .json(&payload)
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;
    });
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uptime_tracking() {
        record_start_time();
        let up = uptime_seconds();
        assert!(up < 2, "uptime should be near zero right after start");
    }

    #[test]
    fn test_system_health_collects() {
        let health = collect_system_health(None);
        // RAM values should be non-zero on any real system
        assert!(health.ram_used_bytes > 0 || health.ram_available_bytes > 0);
    }

    #[test]
    fn test_disk_usage_for_nonexistent_path() {
        let (free, used) = disk_usage_for_path(std::path::Path::new("/nonexistent/path/xyz"));
        // Should return zeros rather than panic
        assert_eq!(free, 0);
        assert_eq!(used, 0);
    }

    #[test]
    fn test_alert_config_defaults() {
        let cfg = AlertConfig::default();
        assert!((cfg.disk_warn_percent - 10.0).abs() < f64::EPSILON);
        assert!((cfg.disk_error_percent - 5.0).abs() < f64::EPSILON);
        assert_eq!(cfg.stale_chain_seconds, 86400);
        assert_eq!(cfg.memory_log_interval_seconds, 3600);
        assert_eq!(cfg.check_interval_seconds, 60);
        assert!(cfg.webhook_url.is_none());
    }

    #[test]
    fn test_health_response_serializes() {
        let resp = HealthResponse {
            status: "ok",
            uptime_seconds: 42,
            version: "0.1.0",
            chains: vec![ChainHealth {
                chain_id: "abc123".into(),
                symbol: "TST".into(),
                block_height: 5,
                last_block_age_seconds: Some(120),
                utxo_count: 10,
                db_size_bytes: 4096,
            }],
            system: SystemHealth {
                ram_used_bytes: 1024,
                ram_available_bytes: 2048,
                cpu_load_percent: None,
                disk_free_bytes: 1_000_000,
                disk_used_bytes: 500_000,
            },
            capacity: CapacityEstimate {
                estimated_assignments_per_second: None,
                estimated_days_until_disk_full: Some(365.0),
            },
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"uptime_seconds\":42"));
        assert!(json.contains("\"abc123\""));
    }
}
