use clap::Args;
use ao_types::dataitem::DataItem;
use ao_types::json::to_json;

#[derive(Args)]
pub struct InspectArgs {
    /// Input file (binary DataItem). Use "-" for stdin.
    #[arg()]
    file: String,

    /// Output format: "json" (default) or "hex"
    #[arg(short, long, default_value = "json")]
    format: String,
}

pub fn run(args: InspectArgs) {
    let data = if args.file == "-" {
        use std::io::Read;
        let mut buf = Vec::new();
        std::io::stdin().read_to_end(&mut buf).unwrap_or_else(|e| {
            eprintln!("Error reading stdin: {}", e);
            std::process::exit(1);
        });
        buf
    } else {
        std::fs::read(&args.file).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", args.file, e);
            std::process::exit(1);
        })
    };

    let item = match DataItem::from_bytes(&data) {
        Ok(item) => item,
        Err(e) => {
            eprintln!("Error decoding DataItem: {}", e);
            std::process::exit(1);
        }
    };

    match args.format.as_str() {
        "json" => {
            let json = to_json(&item);
            println!("{}", serde_json::to_string_pretty(&json).unwrap());
        }
        "hex" => {
            let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
            println!("{}", hex);
        }
        other => {
            eprintln!("Unknown format: {}. Use 'json' or 'hex'.", other);
            std::process::exit(1);
        }
    }
}
