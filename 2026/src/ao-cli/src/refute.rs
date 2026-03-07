use clap::Args;

use ao_types::json as ao_json;
use ao_crypto::hash;

#[derive(Args)]
pub struct RefuteArgs {
    /// Recorder URL (e.g. http://localhost:3000)
    #[arg(short, long)]
    recorder: String,

    /// Chain ID (hex)
    #[arg(short, long)]
    chain: String,

    /// Path to the ASSIGNMENT JSON file being refuted
    #[arg(short, long)]
    assignment: String,
}

pub fn run(args: RefuteArgs) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { run_async(args).await });
}

async fn run_async(args: RefuteArgs) {
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

    // Compute agreement hash
    let assignment_bytes = assignment.to_bytes();
    let agreement_hash = hash::sha256(&assignment_bytes);
    let agreement_hash_hex = hex::encode(agreement_hash);

    eprintln!("Agreement hash: {}", agreement_hash_hex);

    // Submit refutation to recorder
    let url = format!("{}/chain/{}/refute", args.recorder.trim_end_matches('/'), args.chain);
    let client = reqwest::Client::new();
    let body = serde_json::json!({ "agreement_hash": agreement_hash_hex });

    let resp = client.post(&url)
        .json(&body)
        .send()
        .await
        .unwrap_or_else(|e| { eprintln!("Request failed: {}", e); std::process::exit(1); });

    if resp.status().is_success() {
        eprintln!("Refutation recorded successfully.");
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        eprintln!("Refutation failed ({}): {}", status, text);
        std::process::exit(1);
    }
}
