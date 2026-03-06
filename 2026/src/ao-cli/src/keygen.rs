use clap::Args;
use ao_crypto::sign::SigningKey;

#[derive(Args)]
pub struct KeygenArgs {
    /// Output file path for the seed (default: stdout as hex)
    #[arg(short, long)]
    output: Option<String>,
}

pub fn run(args: KeygenArgs) {
    let key = SigningKey::generate();
    let seed_hex: String = key.seed().iter().map(|b| format!("{:02x}", b)).collect();
    let pub_hex: String = key.public_key_bytes().iter().map(|b| format!("{:02x}", b)).collect();

    if let Some(path) = args.output {
        std::fs::write(&path, key.seed()).unwrap_or_else(|e| {
            eprintln!("Error writing seed to {}: {}", path, e);
            std::process::exit(1);
        });
        println!("Seed written to: {}", path);
    } else {
        println!("seed:   {}", seed_hex);
    }
    println!("pubkey: {}", pub_hex);
}
