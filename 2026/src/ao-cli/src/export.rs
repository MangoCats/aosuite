use clap::Args;

#[derive(Args)]
pub struct ExportArgs {
    /// Recorder URL (e.g. http://localhost:3000)
    #[arg(short, long)]
    recorder: String,

    /// Chain ID (hex)
    #[arg(short, long)]
    chain: String,

    /// Starting block height (default: 0)
    #[arg(long, default_value = "0")]
    from: u64,

    /// Ending block height (default: latest)
    #[arg(long)]
    to: Option<u64>,

    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<String>,
}

pub fn run(args: ExportArgs) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { run_async(args).await });
}

async fn run_async(args: ExportArgs) {
    let client = reqwest::Client::new();

    let mut url = format!("{}/chain/{}/blocks?from={}", args.recorder, args.chain, args.from);
    if let Some(to) = args.to {
        url.push_str(&format!("&to={}", to));
    }

    let response = client.get(&url).send().await
        .unwrap_or_else(|e| { eprintln!("Failed to connect: {}", e); std::process::exit(1); });

    let status = response.status();
    if !status.is_success() {
        let body: serde_json::Value = response.json().await.unwrap_or_default();
        eprintln!("Error ({}): {}", status, body["error"].as_str().unwrap_or("unknown"));
        std::process::exit(1);
    }

    let blocks: Vec<serde_json::Value> = response.json().await
        .unwrap_or_else(|e| { eprintln!("Invalid response: {}", e); std::process::exit(1); });

    let json_str = serde_json::to_string_pretty(&blocks).expect("JSON serialization failed");

    if let Some(path) = &args.output {
        std::fs::write(path, &json_str).unwrap_or_else(|e| {
            eprintln!("Failed to write {}: {}", path, e);
            std::process::exit(1);
        });
        eprintln!("Exported {} block(s) to {}", blocks.len(), path);
    } else {
        println!("{}", json_str);
    }
}
