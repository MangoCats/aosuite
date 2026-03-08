use clap::Args;
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::One;

use ao_types::dataitem::DataItem;
use ao_types::typecode;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_crypto::hash;
use ao_crypto::sign::SigningKey;

#[derive(Args)]
pub struct GenesisArgs {
    /// Chain symbol (e.g., "BCG")
    #[arg(short, long)]
    symbol: String,

    /// Chain description
    #[arg(short, long, default_value = "")]
    description: String,

    /// Display coin count (e.g., 10000000000)
    #[arg(short, long, default_value = "10000000000")]
    coins: String,

    /// Initial shares outstanding (e.g., 2^86). Accepts decimal or "2^N" notation.
    #[arg(long, default_value = "2^86")]
    shares: String,

    /// Fee rate numerator
    #[arg(long, default_value = "1")]
    fee_num: String,

    /// Fee rate denominator
    #[arg(long, default_value = "1000000")]
    fee_den: String,

    /// Expiry period in seconds (default: 1 year = 31557600)
    #[arg(long, default_value = "31557600")]
    expiry_seconds: i64,

    /// Expiry mode: 1 = hard cutoff, 2 = age tax
    #[arg(long, default_value = "1")]
    expiry_mode: u64,

    /// Issuer seed file (32 bytes raw) or hex string
    #[arg(long)]
    seed: Option<String>,

    /// Output file for genesis block binary
    #[arg(short, long)]
    output: Option<String>,

    /// Also output JSON representation
    #[arg(long)]
    json: bool,
}

fn parse_bigint(s: &str) -> BigInt {
    if let Some(exp) = s.strip_prefix("2^") {
        let n: u32 = exp.parse().unwrap_or_else(|_| {
            eprintln!("Invalid exponent: {}", exp);
            std::process::exit(1);
        });
        BigInt::one() << n
    } else {
        s.parse().unwrap_or_else(|_| {
            eprintln!("Invalid big integer: {}", s);
            std::process::exit(1);
        })
    }
}

pub fn run(args: GenesisArgs) {
    let coin_count = parse_bigint(&args.coins);
    let shares_out = parse_bigint(&args.shares);
    let fee_num = parse_bigint(&args.fee_num);
    let fee_den = parse_bigint(&args.fee_den);
    let fee_rate = BigRational::new(fee_num, fee_den);
    let expiry_ts = Timestamp::from_unix_seconds(args.expiry_seconds);

    // Get or generate issuer key
    let issuer_key = match &args.seed {
        Some(seed_arg) => load_seed(seed_arg),
        None => {
            let key = SigningKey::generate();
            let hex: String = key.seed().iter().map(|b| format!("{:02x}", b)).collect();
            eprintln!("Generated issuer seed: {}", hex);
            key
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;
    let signing_ts = Timestamp::from_unix_seconds(now);

    // Build genesis block structure per WireFormat.md §6.1
    let mut children: Vec<DataItem> = Vec::new();

    // PROTOCOL_VER
    children.push(DataItem::vbc_value(typecode::PROTOCOL_VER, 1));

    // CHAIN_SYMBOL
    children.push(DataItem::bytes(typecode::CHAIN_SYMBOL, args.symbol.as_bytes().to_vec()));

    // DESCRIPTION (separable)
    if !args.description.is_empty() {
        children.push(DataItem::bytes(typecode::DESCRIPTION, args.description.as_bytes().to_vec()));
    }

    // COIN_COUNT
    let mut coin_buf = Vec::new();
    bigint::encode_bigint(&coin_count, &mut coin_buf);
    children.push(DataItem::bytes(typecode::COIN_COUNT, coin_buf));

    // SHARES_OUT
    let mut shares_buf = Vec::new();
    bigint::encode_bigint(&shares_out, &mut shares_buf);
    children.push(DataItem::bytes(typecode::SHARES_OUT, shares_buf.clone()));

    // FEE_RATE
    let mut fee_buf = Vec::new();
    bigint::encode_rational(&fee_rate, &mut fee_buf);
    children.push(DataItem::bytes(typecode::FEE_RATE, fee_buf));

    // EXPIRY_PERIOD
    children.push(DataItem::bytes(typecode::EXPIRY_PERIOD, expiry_ts.to_bytes().to_vec()));

    // EXPIRY_MODE
    children.push(DataItem::vbc_value(typecode::EXPIRY_MODE, args.expiry_mode));

    // PARTICIPANT (issuer — receives all initial shares)
    children.push(DataItem::container(typecode::PARTICIPANT, vec![
        DataItem::bytes(typecode::ED25519_PUB, issuer_key.public_key_bytes().to_vec()),
        DataItem::bytes(typecode::AMOUNT, shares_buf),
    ]));

    // AUTH_SIG (issuer's signature over the genesis content so far)
    let genesis_so_far = DataItem::container(typecode::GENESIS, children.clone());
    let sig_bytes = ao_crypto::sign::sign_dataitem(&issuer_key, &genesis_so_far, signing_ts);
    children.push(DataItem::container(typecode::AUTH_SIG, vec![
        DataItem::bytes(typecode::ED25519_SIG, sig_bytes.to_vec()),
        DataItem::bytes(typecode::TIMESTAMP, signing_ts.to_bytes().to_vec()),
    ]));

    // Compute SHA256 hash of concatenated child encodings (= chain ID)
    // Must match ao-chain's genesis verification: hash only child encodings,
    // not the outer GENESIS type code + VBC size wrapper.
    let mut content_bytes = Vec::new();
    for child in &children {
        child.encode(&mut content_bytes);
    }
    let chain_id = hash::sha256(&content_bytes);

    children.push(DataItem::bytes(typecode::SHA256, chain_id.to_vec()));

    let genesis_block = DataItem::container(typecode::GENESIS, children);
    let binary = genesis_block.to_bytes();

    // Output
    let chain_id_hex: String = chain_id.iter().map(|b| format!("{:02x}", b)).collect();
    eprintln!("Chain ID: {}", chain_id_hex);
    eprintln!("Block size: {} bytes", binary.len());
    eprintln!("Issuer pubkey: {}", issuer_key.public_key_bytes().iter().map(|b| format!("{:02x}", b)).collect::<String>());

    if let Some(path) = &args.output {
        std::fs::write(path, &binary).unwrap_or_else(|e| {
            eprintln!("Error writing to {}: {}", path, e);
            std::process::exit(1);
        });
        eprintln!("Genesis block written to: {}", path);
    } else if !args.json {
        // Write binary to stdout
        use std::io::Write;
        std::io::stdout().write_all(&binary).unwrap();
    }

    if args.json {
        let json = ao_types::json::to_json(&genesis_block);
        println!("{}", serde_json::to_string_pretty(&json).expect("JSON serialization failed"));
    }
}

fn load_seed(seed_arg: &str) -> SigningKey {
    // Try as file first
    if let Ok(bytes) = std::fs::read(seed_arg)
        && bytes.len() == 32
    {
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        return SigningKey::from_seed(&seed);
    }
    // Try as hex string
    let hex_str = seed_arg.trim();
    if hex_str.len() == 64
        && let Ok(bytes) = hex::decode(hex_str)
    {
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&bytes);
        return SigningKey::from_seed(&seed);
    }
    eprintln!("Invalid seed: must be a 32-byte file or 64-char hex string");
    std::process::exit(1);
}
