use clap::Args;
use serde::Deserialize;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::json as ao_json;
use ao_types::timestamp::Timestamp;

#[derive(Args)]
pub struct AssignArgs {
    /// Recorder URL (e.g. http://localhost:3000)
    #[arg(short, long)]
    recorder: String,

    /// Chain ID (hex)
    #[arg(short, long)]
    chain: String,

    /// Giver UTXO sequence IDs (comma-separated)
    #[arg(short, long, value_delimiter = ',')]
    givers: Vec<u64>,

    /// Receiver pubkeys (hex, comma-separated)
    #[arg(long, value_delimiter = ',')]
    receivers: Vec<String>,

    /// Receiver amounts in shares (comma-separated, must match receiver count)
    /// If omitted, splits evenly after fee deduction
    #[arg(long, value_delimiter = ',')]
    amounts: Option<Vec<String>>,

    /// Deadline as Unix timestamp (optional)
    #[arg(long)]
    deadline: Option<i64>,

    /// Output file for the ASSIGNMENT JSON (default: stdout)
    #[arg(short, long)]
    output: Option<String>,
}

#[derive(Deserialize)]
struct ChainInfo {
    shares_out: String,
    fee_rate_num: String,
    fee_rate_den: String,
}

#[derive(Deserialize)]
struct UtxoInfo {
    amount: String,
}

pub fn run(args: AssignArgs) {
    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    rt.block_on(async { run_async(args).await });
}

async fn run_async(args: AssignArgs) {
    if args.givers.is_empty() {
        eprintln!("Error: at least one giver required");
        std::process::exit(1);
    }
    if args.receivers.is_empty() {
        eprintln!("Error: at least one receiver required");
        std::process::exit(1);
    }

    let client = reqwest::Client::new();

    // Get chain info for fee calculation
    let info_url = format!("{}/chain/{}/info", args.recorder, args.chain);
    let info: ChainInfo = client.get(&info_url).send().await
        .unwrap_or_else(|e| { eprintln!("Failed to connect: {}", e); std::process::exit(1); })
        .json().await
        .unwrap_or_else(|e| { eprintln!("Invalid response: {}", e); std::process::exit(1); });

    // Look up giver UTXO amounts
    let mut giver_amounts: Vec<(u64, num_bigint::BigInt)> = Vec::new();
    let mut giver_total = num_bigint::BigInt::from(0);
    for seq_id in &args.givers {
        let url = format!("{}/chain/{}/utxo/{}", args.recorder, args.chain, seq_id);
        let utxo: UtxoInfo = client.get(&url).send().await
            .unwrap_or_else(|e| { eprintln!("Failed to query UTXO {}: {}", seq_id, e); std::process::exit(1); })
            .json().await
            .unwrap_or_else(|e| { eprintln!("UTXO {} not found: {}", seq_id, e); std::process::exit(1); });
        let amount: num_bigint::BigInt = utxo.amount.parse().expect("recorder returned invalid amount");
        giver_total += &amount;
        giver_amounts.push((*seq_id, amount));
    }

    // Parse receiver pubkeys
    let receiver_pks: Vec<[u8; 32]> = args.receivers.iter().map(|hex_str| {
        let bytes = hex::decode(hex_str).unwrap_or_else(|e| {
            eprintln!("Invalid receiver pubkey hex '{}': {}", hex_str, e);
            std::process::exit(1);
        });
        let pk: [u8; 32] = bytes.try_into().unwrap_or_else(|_| {
            eprintln!("Receiver pubkey must be 32 bytes: {}", hex_str);
            std::process::exit(1);
        });
        pk
    }).collect();

    // Build assignment with iterative fee convergence
    let shares_out: num_bigint::BigInt = info.shares_out.parse().expect("recorder returned invalid shares_out");
    let fee_num: num_bigint::BigInt = info.fee_rate_num.parse().expect("recorder returned invalid fee_rate_num");
    let fee_den: num_bigint::BigInt = info.fee_rate_den.parse().expect("recorder returned invalid fee_rate_den");

    let receiver_amounts = if let Some(amounts_str) = &args.amounts {
        if amounts_str.len() != receiver_pks.len() {
            eprintln!("Error: amounts count ({}) must match receivers count ({})",
                amounts_str.len(), receiver_pks.len());
            std::process::exit(1);
        }
        amounts_str.iter().map(|s| {
            s.parse::<num_bigint::BigInt>().unwrap_or_else(|e| {
                eprintln!("Invalid amount '{}': {}", s, e);
                std::process::exit(1);
            })
        }).collect::<Vec<_>>()
    } else {
        // Auto-compute: iterate to find fee, split remainder evenly
        let assignment_estimate = build_assignment(
            &giver_amounts, &receiver_pks,
            &vec![num_bigint::BigInt::from(1); receiver_pks.len()],
            args.deadline,
        );
        let auth_estimate = DataItem::container(AUTHORIZATION, vec![assignment_estimate]);
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            auth_estimate,
        ]);
        let page_bytes = page.to_bytes().len() as u64;
        let fee = ao_types::fees::recording_fee(page_bytes, &fee_num, &fee_den, &shares_out);

        let remainder = &giver_total - &fee;
        if remainder <= num_bigint::BigInt::from(0) {
            eprintln!("Error: giver total ({}) does not cover fee ({})", giver_total, fee);
            std::process::exit(1);
        }

        // Split evenly, last receiver gets remainder
        let n = receiver_pks.len() as u64;
        let per_receiver = &remainder / num_bigint::BigInt::from(n);
        let mut amounts = vec![per_receiver.clone(); receiver_pks.len()];
        let distributed: num_bigint::BigInt = &per_receiver * num_bigint::BigInt::from(n);
        let last = amounts.last_mut().expect("receiver list is non-empty");
        *last += &remainder - &distributed;
        amounts
    };

    let assignment = build_assignment(&giver_amounts, &receiver_pks, &receiver_amounts, args.deadline);
    let json = ao_json::to_json(&assignment);
    let json_str = serde_json::to_string_pretty(&json).expect("JSON serialization failed");

    if let Some(path) = &args.output {
        std::fs::write(path, &json_str).unwrap_or_else(|e| {
            eprintln!("Failed to write {}: {}", path, e);
            std::process::exit(1);
        });
        eprintln!("Assignment written to {}", path);
    } else {
        println!("{}", json_str);
    }

    // Print summary
    eprintln!("Givers:    {} (total {} shares)", giver_amounts.len(), giver_total);
    let recv_total: num_bigint::BigInt = receiver_amounts.iter().sum();
    let fee = &giver_total - &recv_total;
    eprintln!("Receivers: {} (total {} shares)", receiver_pks.len(), recv_total);
    eprintln!("Fee:       {} shares", fee);
}

fn build_assignment(
    givers: &[(u64, num_bigint::BigInt)],
    receivers: &[[u8; 32]],
    receiver_amounts: &[num_bigint::BigInt],
    deadline: Option<i64>,
) -> DataItem {
    let participant_count = givers.len() + receivers.len();
    let mut children = vec![DataItem::vbc_value(LIST_SIZE, participant_count as u64)];

    for (seq_id, amount) in givers {
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(amount, &mut amount_bytes);
        children.push(DataItem::container(PARTICIPANT, vec![
            DataItem::vbc_value(SEQ_ID, *seq_id),
            DataItem::bytes(AMOUNT, amount_bytes),
        ]));
    }

    for (pk, amount) in receivers.iter().zip(receiver_amounts) {
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(amount, &mut amount_bytes);
        children.push(DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, pk.to_vec()),
            DataItem::bytes(AMOUNT, amount_bytes),
        ]));
    }

    if let Some(dl) = deadline {
        // --deadline is Unix seconds; DEADLINE field must be AO timestamp
        let ao_ts = Timestamp::from_unix_seconds(dl);
        children.push(DataItem::bytes(DEADLINE, ao_ts.to_bytes().to_vec()));
    }

    DataItem::container(ASSIGNMENT, children)
}
