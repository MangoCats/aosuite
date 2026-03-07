use clap::Args;
use serde::Deserialize;

#[derive(Args)]
pub struct BalanceArgs {
    /// Recorder URL (e.g. http://localhost:3000)
    #[arg(short, long)]
    recorder: String,

    /// Chain ID (hex)
    #[arg(short, long)]
    chain: String,

    /// UTXO sequence ID to query
    #[arg(short, long)]
    seq_id: u64,
}

#[derive(Deserialize)]
struct ChainInfo {
    symbol: String,
    shares_out: String,
    coin_count: String,
}

#[derive(Deserialize)]
struct UtxoInfo {
    seq_id: u64,
    pubkey: String,
    amount: String,
    block_height: u64,
    status: String,
}

pub fn run(args: BalanceArgs) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { run_async(args).await });
}

async fn run_async(args: BalanceArgs) {
    let client = reqwest::Client::new();

    // Get chain info for coin display calculation
    let info_url = format!("{}/chain/{}/info", args.recorder, args.chain);
    let info: ChainInfo = client
        .get(&info_url)
        .send()
        .await
        .unwrap_or_else(|e| { eprintln!("Failed to connect to recorder: {}", e); std::process::exit(1); })
        .json()
        .await
        .unwrap_or_else(|e| { eprintln!("Invalid chain info response: {}", e); std::process::exit(1); });

    // Get UTXO
    let utxo_url = format!("{}/chain/{}/utxo/{}", args.recorder, args.chain, args.seq_id);
    let utxo: UtxoInfo = client
        .get(&utxo_url)
        .send()
        .await
        .unwrap_or_else(|e| { eprintln!("Failed to query UTXO: {}", e); std::process::exit(1); })
        .json()
        .await
        .unwrap_or_else(|e| { eprintln!("UTXO not found or invalid response: {}", e); std::process::exit(1); });

    // Compute coin display value: user_coins = shares * total_coins / total_shares
    let shares: num_bigint::BigInt = utxo.amount.parse().expect("recorder returned invalid amount");
    let total_coins: num_bigint::BigInt = info.coin_count.parse().expect("recorder returned invalid coin_count");
    let total_shares: num_bigint::BigInt = info.shares_out.parse().expect("recorder returned invalid shares_out");

    let coin_value = &shares * &total_coins / &total_shares;

    println!("Chain:   {} ({})", info.symbol, args.chain);
    println!("Seq ID:  {}", utxo.seq_id);
    println!("Pubkey:  {}", utxo.pubkey);
    println!("Shares:  {}", utxo.amount);
    println!("Coins:   {} {}", coin_value, info.symbol);
    println!("Block:   {}", utxo.block_height);
    println!("Status:  {}", utxo.status);
}
