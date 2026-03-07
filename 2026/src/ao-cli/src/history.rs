use clap::Args;

#[derive(Args)]
pub struct HistoryArgs {
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
}

pub fn run(args: HistoryArgs) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { run_async(args).await });
}

async fn run_async(args: HistoryArgs) {
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

    if blocks.is_empty() {
        println!("No blocks in range.");
        return;
    }

    println!("{:<8} {:<16} {:<8} Type", "Height", "Hash", "Pages");
    println!("{}", "-".repeat(60));

    for (i, block) in blocks.iter().enumerate() {
        let height = args.from + i as u64;
        let block_type = block["type"].as_str().unwrap_or("?");

        // Extract hash if BLOCK container
        let hash = extract_field(block, "SHA256");
        let hash_short = hash.get(..16).unwrap_or(&hash);

        // Count pages in BLOCK_CONTENTS
        let page_count = count_pages(block);

        println!("{:<8} {:<16} {:<8} {}", height, hash_short, page_count, block_type);
    }

    println!("\n{} block(s) displayed.", blocks.len());
}

fn extract_field(item: &serde_json::Value, type_name: &str) -> String {
    if let Some(items) = item.get("items").and_then(|v| v.as_array()) {
        for child in items {
            if child.get("type").and_then(|t| t.as_str()) == Some(type_name) {
                return child.get("value").and_then(|v| v.as_str()).unwrap_or("").to_string();
            }
            let found = extract_field(child, type_name);
            if !found.is_empty() {
                return found;
            }
        }
    }
    String::new()
}

fn count_pages(block: &serde_json::Value) -> usize {
    if let Some(items) = block.get("items").and_then(|v| v.as_array()) {
        for child in items {
            if child.get("type").and_then(|t| t.as_str()) == Some("BLOCK_SIGNED")
                && let Some(signed_items) = child.get("items").and_then(|v| v.as_array())
            {
                for sc in signed_items {
                    if sc.get("type").and_then(|t| t.as_str()) == Some("BLOCK_CONTENTS")
                        && let Some(bc_items) = sc.get("items").and_then(|v| v.as_array())
                    {
                        return bc_items.iter()
                            .filter(|c| c.get("type").and_then(|t| t.as_str()) == Some("PAGE"))
                            .count();
                    }
                }
            }
        }
    }
    0
}
