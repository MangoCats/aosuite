use clap::Args;
use serde::Deserialize;

#[derive(Args)]
pub struct CaaStatusArgs {
    /// Recorder URL (e.g. http://localhost:3000)
    #[arg(short, long)]
    recorder: String,

    /// Chain ID (hex)
    #[arg(short, long)]
    chain: String,

    /// CAA hash (hex)
    #[arg(long)]
    caa_hash: String,
}

#[derive(Deserialize)]
struct CaaStatus {
    caa_hash: String,
    status: String,
    chain_order: u64,
    deadline: i64,
    block_height: u64,
    #[serde(default)]
    has_proof: bool,
}

pub fn run(args: CaaStatusArgs) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { run_async(args).await });
}

async fn run_async(args: CaaStatusArgs) {
    let client = reqwest::Client::new();
    let url = format!("{}/chain/{}/caa/{}", args.recorder, args.chain, args.caa_hash);

    let resp = client
        .get(&url)
        .send()
        .await
        .unwrap_or_else(|e| { eprintln!("Failed to connect to recorder: {}", e); std::process::exit(1); });

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        eprintln!("CAA query failed ({}): {}", status, body);
        std::process::exit(1);
    }

    let caa: CaaStatus = resp
        .json()
        .await
        .unwrap_or_else(|e| { eprintln!("Invalid CAA status response: {}", e); std::process::exit(1); });

    println!("CAA Hash:    {}", caa.caa_hash);
    println!("Status:      {}", caa.status);
    println!("Chain Order: {}", caa.chain_order);
    println!("Deadline:    {}", caa.deadline);
    println!("Block:       {}", caa.block_height);
    println!("Has Proof:   {}", caa.has_proof);
}
