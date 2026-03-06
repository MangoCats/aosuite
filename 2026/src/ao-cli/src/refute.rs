use clap::Args;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::json as ao_json;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign::SigningKey;
use ao_crypto::sign;
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

    /// Signing seed (hex, 32 bytes) — the refuting participant's key
    #[arg(short, long)]
    seed: String,
}

pub fn run(args: RefuteArgs) {
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

    // Parse signing key
    let seed_bytes = hex::decode(args.seed.trim()).unwrap_or_else(|e| {
        eprintln!("Invalid seed hex: {}", e);
        std::process::exit(1);
    });
    let seed: [u8; 32] = seed_bytes.try_into().unwrap_or_else(|_| {
        eprintln!("Seed must be 32 bytes");
        std::process::exit(1);
    });
    let key = SigningKey::from_seed(&seed);

    // Current timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let ts = Timestamp::from_unix_seconds(now);

    // Build REFUTATION DataItem
    let refutation = DataItem::container(REFUTATION, vec![
        DataItem::bytes(SHA256, agreement_hash.to_vec()),
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sign::sign_dataitem(&key, &assignment, ts).to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
        ]),
    ]);

    let json = ao_json::to_json(&refutation);
    let json_str = serde_json::to_string_pretty(&json).unwrap();

    println!("{}", json_str);
    eprintln!("Agreement hash: {}", hex::encode(agreement_hash));
    eprintln!("Refutation built. Submit to recorder when refutation endpoint is available.");
}
