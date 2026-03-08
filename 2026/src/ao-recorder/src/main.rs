use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use tracing::info;

use ao_types::dataitem::DataItem;
use ao_crypto::sign::SigningKey;
use ao_chain::store::ChainStore;

use ao_recorder::{AppState, ChainState, blob, build_router, config, health, mqtt, poll_validators};

fn load_blockmaker_key(seed_hex: &str) -> Result<SigningKey> {
    let seed_bytes: Vec<u8> = hex::decode(seed_hex.trim())
        .context("invalid blockmaker seed hex")?;
    let seed: [u8; 32] = seed_bytes.try_into()
        .map_err(|v: Vec<u8>| anyhow::anyhow!("blockmaker seed must be 32 bytes, got {}", v.len()))?;
    SigningKey::try_from_seed(&seed)
        .map_err(|e| anyhow::anyhow!("invalid Ed25519 seed: {}", e))
}

fn load_chain(db_path: &str, genesis_path: &str, blockmaker_key: &SigningKey) -> Result<(String, ChainStore)> {
    let store = ChainStore::open(db_path)
        .context("failed to open database")?;

    let meta = match store.load_chain_meta()
        .context("failed to query chain metadata")?
    {
        Some(m) => {
            info!(chain_id = hex::encode(m.chain_id), symbol = %m.symbol, "Chain loaded");
            m
        }
        None => {
            let genesis_data = std::fs::read(genesis_path)
                .context("failed to read genesis file")?;
            let genesis_item = DataItem::from_bytes(&genesis_data)
                .map_err(|e| anyhow::anyhow!("failed to decode genesis block: {:?}", e))?;
            let m = ao_chain::genesis::load_genesis(&store, &genesis_item)
                .context("failed to load genesis")?;
            info!(chain_id = hex::encode(m.chain_id), symbol = %m.symbol, "Genesis loaded");
            m
        }
    };

    let _ = blockmaker_key; // used by caller
    Ok((hex::encode(meta.chain_id), store))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Subcommand dispatch
    match args.get(1).map(|s| s.as_str()) {
        Some("--version") | Some("-V") => {
            println!("ao-recorder {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("doctor") => {
            let config_path = args.get(2).map(|s| s.as_str()).unwrap_or("recorder.toml");
            return run_doctor(config_path);
        }
        Some("init") => {
            let output_path = args.get(2).map(|s| s.as_str()).unwrap_or("recorder.toml");
            return run_init(output_path);
        }
        Some("bench") => {
            tracing_subscriber::fmt::init();
            let config_path = args.get(2).map(|s| s.as_str()).unwrap_or("recorder.toml");
            return run_bench(config_path).await;
        }
        _ => {}
    }

    tracing_subscriber::fmt::init();
    health::record_start_time();

    let config_path = args.get(1).map(|s| s.as_str()).unwrap_or("recorder.toml");
    let cfg = config::load_config(config_path)?;

    let default_key = load_blockmaker_key(&cfg.blockmaker_seed)?;

    let data_dir = cfg.data_dir.as_ref().map(PathBuf::from);
    if let Some(dir) = &data_dir {
        std::fs::create_dir_all(dir).context("failed to create data directory")?;
    }

    let blob_store = if let Some(ref data_dir) = data_dir {
        let blob_dir = data_dir.join("blobs");
        Some(blob::BlobStore::new(blob_dir, 5_242_880)
            .context("failed to create blob store")?)
    } else {
        None
    };

    if blob_store.is_none() {
        tracing::warn!("Blob storage disabled: no data_dir configured. Blob upload/download endpoints will return errors.");
    }

    let mut state_inner = AppState::new_multi(data_dir, SigningKey::from_seed(default_key.seed()));
    state_inner.blob_store = blob_store;
    let state = Arc::new(state_inner);

    // Load single-chain config (backward compatible)
    if let (Some(db_path), Some(genesis_path)) = (&cfg.db_path, &cfg.genesis_path) {
        let (chain_id, store) = load_chain(db_path, genesis_path, &default_key)?;
        let chain_state = Arc::new(ChainState::new(store, SigningKey::from_seed(default_key.seed())));
        info!(chain_id = %chain_id, "Registered chain");
        state.add_chain(chain_id, chain_state);
    }

    // Load additional chains from [[chains]] config
    for chain_cfg in &cfg.chains {
        let bm_key = if let Some(seed_hex) = &chain_cfg.blockmaker_seed {
            load_blockmaker_key(seed_hex)?
        } else {
            SigningKey::from_seed(default_key.seed())
        };
        let (chain_id, store) = load_chain(&chain_cfg.db_path, &chain_cfg.genesis_path, &bm_key)?;
        let chain_state = Arc::new(ChainState::new(store, bm_key));
        info!(chain_id = %chain_id, "Registered chain");
        state.add_chain(chain_id, chain_state);
    }

    // Initialize MQTT if configured
    if let Some(mqtt_cfg) = &cfg.mqtt {
        if let Some(publisher) = mqtt::MqttPublisher::connect(mqtt_cfg) {
            state.set_mqtt(publisher);
            info!("MQTT block publishing enabled");
        } else {
            tracing::warn!("MQTT configured but connection failed — continuing without MQTT");
        }
    }

    // Start validator polling background task if validators are configured
    if !cfg.validators.is_empty() {
        let validator_state = Arc::clone(&state);
        let validators = cfg.validators.clone();
        tokio::spawn(async move {
            poll_validators(validator_state, validators).await;
        });
        info!(count = cfg.validators.len(), "Validator polling enabled");
    }

    // Start operational alerts background task
    {
        let alert_config = health::AlertConfig::from_config(&cfg);
        let alert_state = Arc::clone(&state);
        tokio::spawn(async move {
            health::run_alerts(alert_state, alert_config).await;
        });
    }

    let chain_count = state.chains.read()
        .map_err(|e| anyhow::anyhow!("chains lock poisoned: {}", e))?
        .len();
    let app = build_router(state);

    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    info!(%bind_addr, chain_count, "Starting AO Recorder");

    let listener = tokio::net::TcpListener::bind(&bind_addr).await
        .context("failed to bind TCP listener")?;
    axum::serve(listener, app).await
        .context("server error")?;

    Ok(())
}

// ── Subcommands ─────────────────────────────────────────────────────

/// `ao-recorder doctor [config.toml]` — post-install diagnostic.
fn run_doctor(config_path: &str) -> Result<()> {
    println!("ao-recorder doctor v{}", env!("CARGO_PKG_VERSION"));
    println!();

    let mut ok = true;

    // 1. Binary runs (we're here, so yes)
    print_check(true, "Binary executes");

    // 2. Config file readable and parseable
    let cfg = match config::load_config(config_path) {
        Ok(cfg) => {
            print_check(true, &format!("Config file '{}' is valid", config_path));
            Some(cfg)
        }
        Err(e) => {
            print_check(false, &format!("Config file '{}': {}", config_path, e));
            ok = false;
            None
        }
    };

    if let Some(cfg) = &cfg {
        // 3. Blockmaker seed valid
        match load_blockmaker_key(&cfg.blockmaker_seed) {
            Ok(_) => print_check(true, "Blockmaker seed is valid Ed25519 key"),
            Err(e) => {
                print_check(false, &format!("Blockmaker seed: {}", e));
                ok = false;
            }
        }

        // 4. Data directory writable
        if let Some(dir) = &cfg.data_dir {
            let path = PathBuf::from(dir);
            if path.exists() {
                let test_file = path.join(".doctor_test");
                match std::fs::write(&test_file, b"test") {
                    Ok(_) => {
                        let _ = std::fs::remove_file(&test_file);
                        print_check(true, &format!("Data directory '{}' is writable", dir));
                    }
                    Err(e) => {
                        print_check(false, &format!("Data directory '{}' not writable: {}", dir, e));
                        ok = false;
                    }
                }
            } else {
                match std::fs::create_dir_all(&path) {
                    Ok(_) => print_check(true, &format!("Data directory '{}' created", dir)),
                    Err(e) => {
                        print_check(false, &format!("Cannot create data directory '{}': {}", dir, e));
                        ok = false;
                    }
                }
            }
        } else {
            print_check(false, "No data_dir configured (in-memory only — data lost on restart)");
            // Not a hard failure, just a warning
        }

        // 5. Port available
        let addr = format!("{}:{}", cfg.host, cfg.port);
        match std::net::TcpListener::bind(&addr) {
            Ok(_) => print_check(true, &format!("Port {} available on {}", cfg.port, cfg.host)),
            Err(e) => {
                print_check(false, &format!("Port {} on {}: {}", cfg.port, cfg.host, e));
                ok = false;
            }
        }

        // 6. SQLite functional
        match ChainStore::open_memory() {
            Ok(store) => {
                match store.init_schema() {
                    Ok(_) => print_check(true, "SQLite functional"),
                    Err(e) => {
                        print_check(false, &format!("SQLite schema init failed: {}", e));
                        ok = false;
                    }
                }
            }
            Err(e) => {
                print_check(false, &format!("SQLite open failed: {}", e));
                ok = false;
            }
        }

        // 7. Check chain DB files if configured
        if let (Some(db_path), _) = (&cfg.db_path, &cfg.genesis_path) {
            let p = std::path::Path::new(db_path);
            if p.exists() {
                match ChainStore::open(db_path) {
                    Ok(store) => {
                        match store.load_chain_meta() {
                            Ok(Some(m)) => print_check(true, &format!(
                                "Chain database '{}': {} at height {}", db_path, m.symbol, m.block_height
                            )),
                            Ok(None) => print_check(true, &format!(
                                "Chain database '{}' exists but uninitialized (genesis will load on start)", db_path
                            )),
                            Err(e) => {
                                print_check(false, &format!("Chain database '{}': {}", db_path, e));
                                ok = false;
                            }
                        }
                    }
                    Err(e) => {
                        print_check(false, &format!("Cannot open chain database '{}': {}", db_path, e));
                        ok = false;
                    }
                }
            } else {
                print_check(true, &format!("Chain database '{}' will be created on first run", db_path));
            }
        }

        // 8. Disk space check
        if let Some(dir) = &cfg.data_dir {
            let disks = sysinfo::Disks::new_with_refreshed_list();
            let path = PathBuf::from(dir);
            let mut found = false;
            for disk in disks.list() {
                if path.starts_with(disk.mount_point()) {
                    let free = disk.available_space();
                    let total = disk.total_space();
                    let free_pct = if total > 0 { (free as f64 / total as f64) * 100.0 } else { 0.0 };
                    let free_gb = free as f64 / 1_073_741_824.0;
                    if free_pct < 5.0 {
                        print_check(false, &format!("Disk space: {:.1} GB free ({:.1}%) — critically low", free_gb, free_pct));
                        ok = false;
                    } else if free_pct < 10.0 {
                        print_check(false, &format!("Disk space: {:.1} GB free ({:.1}%) — low", free_gb, free_pct));
                    } else {
                        print_check(true, &format!("Disk space: {:.1} GB free ({:.1}%)", free_gb, free_pct));
                    }
                    found = true;
                    break;
                }
            }
            if !found {
                print_check(false, "Could not determine disk space for data directory");
            }
        }
    }

    println!();
    if ok {
        println!("All checks passed. Ready to start.");
        Ok(())
    } else {
        anyhow::bail!("Some checks failed. See above for details.");
    }
}

fn print_check(pass: bool, msg: &str) {
    if pass {
        println!("  [OK]   {}", msg);
    } else {
        println!("  [FAIL] {}", msg);
    }
}

/// `ao-recorder init [output.toml]` — generate starter config.
fn run_init(output_path: &str) -> Result<()> {
    if std::path::Path::new(output_path).exists() {
        anyhow::bail!("Config file '{}' already exists. Remove it first or choose a different path.", output_path);
    }

    // Generate a random blockmaker seed
    let seed = ao_crypto::sign::SigningKey::generate();
    let seed_hex = hex::encode(seed.seed());
    let pubkey_hex = hex::encode(seed.public_key_bytes());

    // Prompt-free: generate sensible defaults
    let config_content = format!(r#"# AO Recorder configuration
# Generated by ao-recorder init

# Network binding
host = "0.0.0.0"
port = 3000

# Blockmaker identity (Ed25519 seed, hex-encoded)
# Public key: {}
blockmaker_seed = "{}"

# Persistent data directory (databases, blobs)
data_dir = "data"

# Single-chain configuration (uncomment and edit):
# db_path = "data/chain.db"
# genesis_path = "genesis.bin"

# Multiple chains (add [[chains]] sections):
# [[chains]]
# db_path = "data/mychain.db"
# genesis_path = "mychain_genesis.bin"
# blockmaker_seed = "optional-per-chain-seed-hex"

# MQTT block notifications (optional):
# [mqtt]
# host = "localhost"
# port = 1883

# Validator endpoints (optional):
# [[validators]]
# url = "http://localhost:4000"
# label = "local-validator"

# Operational alerts (optional):
# [alerts]
# disk_warn_percent = 10.0
# disk_error_percent = 5.0
# stale_chain_seconds = 86400
# memory_log_interval_seconds = 3600
# webhook_url = "https://example.com/webhook"
"#, pubkey_hex, seed_hex);

    std::fs::write(output_path, config_content)
        .context("failed to write config file")?;

    // Create data directory
    std::fs::create_dir_all("data")
        .context("failed to create data directory")?;

    println!("Created config: {}", output_path);
    println!("Created data directory: data/");
    println!();
    println!("Blockmaker public key: {}", pubkey_hex);
    println!();
    println!("Next steps:");
    println!("  1. Edit {} to configure your chain(s)", output_path);
    println!("  2. Run: ao-recorder doctor");
    println!("  3. Run: ao-recorder {}", output_path);
    println!("  4. Open: http://localhost:3000/health");

    Ok(())
}

/// `ao-recorder bench [config.toml]` — benchmark storage throughput on target hardware.
///
/// Measures SQLite block storage performance (the real bottleneck for capacity planning)
/// by inserting synthetic blocks into a temporary database. This gives a hardware-specific
/// throughput baseline without requiring the full authorization signing flow.
async fn run_bench(config_path: &str) -> Result<()> {
    use std::time::Instant;

    println!("ao-recorder bench v{}", env!("CARGO_PKG_VERSION"));
    println!();

    let _cfg = config::load_config(config_path)
        .unwrap_or_else(|_| config::Config::default());

    let n_blocks: u64 = 1000;
    println!("Inserting {} synthetic blocks (in-memory SQLite)...", n_blocks);

    let initial_rss = process_rss();

    let store = ChainStore::open_memory().context("open store")?;
    store.init_schema().context("init schema")?;

    // Minimal chain meta so block insertion works
    let meta = ao_chain::store::ChainMeta {
        chain_id: [0xAA; 32],
        symbol: "BENCH".to_string(),
        coin_count: num_bigint::BigInt::from(1_000_000),
        shares_out: num_bigint::BigInt::from(1_000_000),
        fee_rate_num: num_bigint::BigInt::from(1),
        fee_rate_den: num_bigint::BigInt::from(1_000_000),
        expiry_period: 86400 * 30,
        expiry_mode: 0,
        tax_start_age: None,
        tax_doubling_period: None,
        block_height: 0,
        next_seq_id: 1,
        last_block_timestamp: 0,
        prev_hash: [0u8; 32],
    };
    store.store_chain_meta(&meta).context("store meta")?;

    // Generate a synthetic block payload (~500 bytes, realistic)
    let block_data: Vec<u8> = (0..500).map(|i| (i % 256) as u8).collect();
    let mut hash = [0u8; 32];

    let start = Instant::now();
    for i in 0..n_blocks {
        // Vary the hash so each block is unique
        hash[0] = (i & 0xFF) as u8;
        hash[1] = ((i >> 8) & 0xFF) as u8;
        let timestamp = 1_772_611_200 + i as i64;
        store.store_block(i, timestamp, &hash, &block_data)
            .context("store block")?;
        store.advance_block(i, timestamp, &hash)
            .context("advance block")?;
    }
    let elapsed = start.elapsed();
    let final_rss = process_rss();

    let block_count = store.block_count().unwrap_or(0);

    println!();
    println!("Results:");
    println!("  Blocks inserted:  {}", block_count);
    println!("  Block size:       {} bytes each", block_data.len());
    println!("  Total time:       {:.2}s", elapsed.as_secs_f64());
    println!("  Throughput:       {:.1} blocks/sec", n_blocks as f64 / elapsed.as_secs_f64());
    println!("  Avg insert time:  {:.3}ms", elapsed.as_millis() as f64 / n_blocks as f64);
    if let (Some(initial), Some(final_)) = (initial_rss, final_rss) {
        println!("  RSS initial:      {:.1} MB", initial as f64 / 1_048_576.0);
        println!("  RSS final:        {:.1} MB", final_ as f64 / 1_048_576.0);
        println!("  RSS growth:       {:.1} MB", (final_ as f64 - initial as f64) / 1_048_576.0);
    }

    // Signature benchmark (Ed25519 — the other bottleneck)
    println!();
    println!("Ed25519 signature benchmark (1000 sign + verify)...");
    let sign_key = ao_crypto::sign::SigningKey::generate();
    let msg = [0u8; 256];
    let sign_start = Instant::now();
    for _ in 0..1000 {
        let sig = sign_key.sign_raw(&msg);
        let _ = ao_crypto::sign::verify_raw(sign_key.public_key_bytes(), &msg, &sig);
    }
    let sign_elapsed = sign_start.elapsed();
    println!("  Sign+verify:      {:.1} ops/sec", 1000.0 / sign_elapsed.as_secs_f64());
    println!("  Avg time:         {:.3}ms per sign+verify", sign_elapsed.as_millis() as f64 / 1000.0);

    Ok(())
}

fn process_rss() -> Option<u64> {
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::get_current_pid().ok()?;
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    sys.process(pid).map(|p| p.memory())
}
