use clap::Args;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::json as ao_json;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign::SigningKey;
use ao_crypto::sign;

#[derive(Args)]
pub struct AcceptArgs {
    /// Recorder URL (e.g. http://localhost:3000)
    #[arg(short, long)]
    recorder: String,

    /// Chain ID (hex)
    #[arg(short, long)]
    chain: String,

    /// Path to ASSIGNMENT JSON file
    #[arg(short, long)]
    assignment: String,

    /// Signing seeds (hex, comma-separated) — one per participant in order
    #[arg(short, long, value_delimiter = ',')]
    seeds: Vec<String>,

    /// Page index for each signer (comma-separated, matching seed order)
    /// Defaults to 0, 1, 2, ... matching participant order
    #[arg(long, value_delimiter = ',')]
    page_indices: Option<Vec<u64>>,
}

pub fn run(args: AcceptArgs) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { run_async(args).await });
}

async fn run_async(args: AcceptArgs) {
    // Read assignment JSON
    let json_str = std::fs::read_to_string(&args.assignment).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", args.assignment, e);
        std::process::exit(1);
    });
    let json_value: serde_json::Value = serde_json::from_str(&json_str).unwrap_or_else(|e| {
        eprintln!("Invalid JSON: {}", e);
        std::process::exit(1);
    });
    let assignment = ao_json::from_json(&json_value).unwrap_or_else(|e| {
        eprintln!("Invalid DataItem: {}", e);
        std::process::exit(1);
    });

    if assignment.type_code != ASSIGNMENT {
        eprintln!("Error: expected ASSIGNMENT, got type code {}", assignment.type_code);
        std::process::exit(1);
    }

    // Parse signing keys
    let keys: Vec<SigningKey> = args.seeds.iter().map(|s| {
        let seed_bytes = hex::decode(s.trim()).unwrap_or_else(|e| {
            eprintln!("Invalid seed hex: {}", e);
            std::process::exit(1);
        });
        let seed: [u8; 32] = seed_bytes.try_into().unwrap_or_else(|_| {
            eprintln!("Seed must be 32 bytes");
            std::process::exit(1);
        });
        SigningKey::from_seed(&seed)
    }).collect();

    let page_indices: Vec<u64> = args.page_indices.unwrap_or_else(|| {
        (0..keys.len() as u64).collect()
    });

    if keys.len() != page_indices.len() {
        eprintln!("Error: seeds count ({}) must match page_indices count ({})",
            keys.len(), page_indices.len());
        std::process::exit(1);
    }

    // Current timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let ts = Timestamp::from_unix_seconds(now);

    // Build AUTH_SIG items
    let mut auth_sigs = Vec::new();
    for (key, &page_idx) in keys.iter().zip(&page_indices) {
        let sig = sign::sign_dataitem(key, &assignment, ts);
        auth_sigs.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, page_idx),
        ]));
    }

    // Build AUTHORIZATION
    let mut auth_children = vec![assignment];
    auth_children.extend(auth_sigs);
    let authorization = DataItem::container(AUTHORIZATION, auth_children);

    // Submit to recorder
    let auth_json = ao_json::to_json(&authorization);
    let client = reqwest::Client::new();
    let submit_url = format!("{}/chain/{}/submit", args.recorder, args.chain);

    let response = client.post(&submit_url)
        .json(&auth_json)
        .send()
        .await
        .unwrap_or_else(|e| {
            eprintln!("Failed to submit: {}", e);
            std::process::exit(1);
        });

    let status = response.status();
    let body: serde_json::Value = response.json().await.unwrap_or_else(|e| {
        eprintln!("Invalid response: {}", e);
        std::process::exit(1);
    });

    if status.is_success() {
        println!("Block recorded!");
        println!("  Height:     {}", body["height"]);
        println!("  Hash:       {}", body["hash"].as_str().unwrap_or(""));
        println!("  Timestamp:  {}", body["timestamp"]);
        println!("  Shares out: {}", body["shares_out"].as_str().unwrap_or(""));
        println!("  First seq:  {}", body["first_seq"]);
        println!("  Seq count:  {}", body["seq_count"]);
    } else {
        eprintln!("Error ({}): {}", status, body["error"].as_str().unwrap_or("unknown"));
        std::process::exit(1);
    }
}
