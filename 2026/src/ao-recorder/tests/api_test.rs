/// Integration tests for the ao-recorder HTTP API.
/// Spins up an in-process Axum server and tests all endpoints.
use std::sync::Arc;

use num_bigint::BigInt;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_types::fees;
use ao_types::json as ao_json;
use ao_crypto::hash;
use ao_crypto::sign::{self, SigningKey};

use ao_chain::store::ChainStore;
use ao_chain::genesis;

use ao_recorder::{AppState, build_router};

/// Build a test genesis block.
fn build_genesis(issuer_key: &SigningKey) -> DataItem {
    build_genesis_with_symbol(issuer_key, "TST")
}

fn build_genesis_with_symbol(issuer_key: &SigningKey, symbol: &str) -> DataItem {
    let pubkey = issuer_key.public_key_bytes().to_vec();

    let shares_out = BigInt::from(1u64 << 40);
    let mut shares_bytes = Vec::new();
    bigint::encode_bigint(&shares_out, &mut shares_bytes);

    let coin_count = BigInt::from(1_000_000_000u64);
    let mut coin_bytes = Vec::new();
    bigint::encode_bigint(&coin_count, &mut coin_bytes);

    let fee_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
    let mut fee_bytes = Vec::new();
    bigint::encode_rational(&fee_rate, &mut fee_bytes);

    let expiry_period = Timestamp::from_unix_seconds(31_536_000);
    let ts = Timestamp::from_unix_seconds(1_772_611_200);

    let signable_children = vec![
        DataItem::vbc_value(PROTOCOL_VER, 1),
        DataItem::bytes(CHAIN_SYMBOL, symbol.as_bytes().to_vec()),
        DataItem::bytes(DESCRIPTION, b"API test chain".to_vec()),
        DataItem::bytes(COIN_COUNT, coin_bytes),
        DataItem::bytes(SHARES_OUT, shares_bytes.clone()),
        DataItem::bytes(FEE_RATE, fee_bytes),
        DataItem::bytes(EXPIRY_PERIOD, expiry_period.to_bytes().to_vec()),
        DataItem::vbc_value(EXPIRY_MODE, 1),
        DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, pubkey),
            DataItem::bytes(AMOUNT, shares_bytes),
        ]),
    ];
    let signable = DataItem::container(GENESIS, signable_children.clone());
    let sig = sign::sign_dataitem(issuer_key, &signable, ts);

    let mut all_children = signable_children;
    all_children.push(DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
    ]));

    let mut content_bytes = Vec::new();
    for child in &all_children {
        child.encode(&mut content_bytes);
    }
    let chain_hash = hash::sha256(&content_bytes);
    all_children.push(DataItem::bytes(SHA256, chain_hash.to_vec()));

    DataItem::container(GENESIS, all_children)
}

/// Start a test server, return (base_url, chain_id_hex).
async fn start_test_server(
    issuer_key: &SigningKey,
    blockmaker_key: &SigningKey,
) -> (String, String) {
    let store = ChainStore::open_memory().unwrap();
    let genesis_item = build_genesis(issuer_key);
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();
    let chain_id = hex::encode(meta.chain_id);

    let state = Arc::new(AppState::new(store, SigningKey::from_seed(blockmaker_key.seed())));

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, chain_id)
}

/// Start a test server with no pre-loaded chains (multi-chain mode).
async fn start_empty_server(blockmaker_key: &SigningKey) -> String {
    let state = Arc::new(AppState::new_multi(
        None,
        SigningKey::from_seed(blockmaker_key.seed()),
    ));

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    base_url
}

/// Build a signed AUTHORIZATION with iterative fee calculation.
fn build_authorization(
    giver_key: &SigningKey,
    giver_seq_id: u64,
    giver_amount: &BigInt,
    receiver_key: &SigningKey,
    fee_rate_num: &BigInt,
    fee_rate_den: &BigInt,
    shares_out: &BigInt,
    giver_sign_ts: Timestamp,
    receiver_sign_ts: Timestamp,
) -> DataItem {
    let bid = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
    let mut bid_bytes = Vec::new();
    bigint::encode_rational(&bid, &mut bid_bytes);

    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 86400);

    let mut receiver_amount = giver_amount.clone();
    for _ in 0..3 {
        let auth = build_auth_inner(
            giver_key, giver_seq_id, giver_amount,
            receiver_key, &receiver_amount,
            &bid_bytes, deadline, giver_sign_ts, receiver_sign_ts,
        );
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            auth,
        ]);
        let page_bytes = page.to_bytes().len() as u64;
        let fee = fees::recording_fee(page_bytes, fee_rate_num, fee_rate_den, shares_out);
        receiver_amount = giver_amount - &fee;
    }

    build_auth_inner(
        giver_key, giver_seq_id, giver_amount,
        receiver_key, &receiver_amount,
        &bid_bytes, deadline, giver_sign_ts, receiver_sign_ts,
    )
}

fn build_auth_inner(
    giver_key: &SigningKey,
    giver_seq_id: u64,
    giver_amount: &BigInt,
    receiver_key: &SigningKey,
    receiver_amount: &BigInt,
    bid_bytes: &[u8],
    deadline: Timestamp,
    giver_sign_ts: Timestamp,
    receiver_sign_ts: Timestamp,
) -> DataItem {
    let mut giver_amount_bytes = Vec::new();
    bigint::encode_bigint(giver_amount, &mut giver_amount_bytes);
    let mut receiver_amount_bytes = Vec::new();
    bigint::encode_bigint(receiver_amount, &mut receiver_amount_bytes);

    let assignment = DataItem::container(ASSIGNMENT, vec![
        DataItem::vbc_value(LIST_SIZE, 2),
        DataItem::container(PARTICIPANT, vec![
            DataItem::vbc_value(SEQ_ID, giver_seq_id),
            DataItem::bytes(AMOUNT, giver_amount_bytes),
        ]),
        DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, receiver_key.public_key_bytes().to_vec()),
            DataItem::bytes(AMOUNT, receiver_amount_bytes),
        ]),
        DataItem::bytes(RECORDING_BID, bid_bytes.to_vec()),
        DataItem::bytes(DEADLINE, deadline.to_bytes().to_vec()),
    ]);

    let giver_sig = sign::sign_dataitem(giver_key, &assignment, giver_sign_ts);
    let receiver_sig = sign::sign_dataitem(receiver_key, &assignment, receiver_sign_ts);

    DataItem::container(AUTHORIZATION, vec![
        assignment,
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, giver_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, giver_sign_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 0),
        ]),
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, receiver_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, receiver_sign_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 1),
        ]),
    ])
}

#[tokio::test]
async fn test_chain_info() {
    let issuer = SigningKey::from_seed(&[0x10; 32]);
    let blockmaker = SigningKey::from_seed(&[0x20; 32]);
    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    assert_eq!(resp["symbol"], "TST");
    assert_eq!(resp["block_height"], 0);
    assert_eq!(resp["chain_id"], chain_id);
    assert_eq!(resp["expiry_mode"], 1);
    assert!(resp["next_seq_id"].as_u64().unwrap() >= 2);
}

#[tokio::test]
async fn test_chain_info_wrong_id() {
    let issuer = SigningKey::from_seed(&[0x11; 32]);
    let blockmaker = SigningKey::from_seed(&[0x21; 32]);
    let (base, _chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/chain/{}/info", base, "0000"))
        .send().await.unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_get_utxo() {
    let issuer = SigningKey::from_seed(&[0x12; 32]);
    let blockmaker = SigningKey::from_seed(&[0x22; 32]);
    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();
    let resp: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    assert_eq!(resp["seq_id"], 1);
    assert_eq!(resp["status"], "Unspent");
    assert_eq!(resp["pubkey"], hex::encode(issuer.public_key_bytes()));
}

#[tokio::test]
async fn test_utxo_not_found() {
    let issuer = SigningKey::from_seed(&[0x13; 32]);
    let blockmaker = SigningKey::from_seed(&[0x23; 32]);
    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/chain/{}/utxo/999", base, chain_id))
        .send().await.unwrap();

    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_submit_assignment() {
    let issuer = SigningKey::from_seed(&[0x14; 32]);
    let blockmaker = SigningKey::from_seed(&[0x24; 32]);
    let receiver = SigningKey::generate();

    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;
    let client = reqwest::Client::new();

    // Get chain info for fee calculation
    let info: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    let shares_out: BigInt = info["shares_out"].as_str().unwrap().parse().unwrap();
    let fee_num: BigInt = info["fee_rate_num"].as_str().unwrap().parse().unwrap();
    let fee_den: BigInt = info["fee_rate_den"].as_str().unwrap().parse().unwrap();

    // Get giver amount
    let utxo: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    let giver_amount: BigInt = utxo["amount"].as_str().unwrap().parse().unwrap();

    // Build authorization
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let giver_ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let recv_ts = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);

    let auth = build_authorization(
        &issuer, 1, &giver_amount, &receiver,
        &fee_num, &fee_den, &shares_out,
        giver_ts, recv_ts,
    );

    let auth_json = ao_json::to_json(&auth);

    // Submit
    let resp = client
        .post(format!("{}/chain/{}/submit", base, chain_id))
        .json(&auth_json)
        .send().await.unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["height"], 1);
    assert_eq!(body["first_seq"], 2);
    assert_eq!(body["seq_count"], 1);
    assert!(body["hash"].as_str().unwrap().len() == 64); // 32 bytes hex

    // Verify UTXO 1 is now spent
    let utxo1: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(utxo1["status"], "Spent");

    // Verify new UTXO exists
    let utxo2: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/2", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(utxo2["status"], "Unspent");
    assert_eq!(utxo2["pubkey"], hex::encode(receiver.public_key_bytes()));

    // Verify chain info updated
    let info2: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(info2["block_height"], 1);
    assert_eq!(info2["next_seq_id"], 3);
}

#[tokio::test]
async fn test_get_blocks() {
    let issuer = SigningKey::from_seed(&[0x15; 32]);
    let blockmaker = SigningKey::from_seed(&[0x25; 32]);
    let receiver = SigningKey::generate();

    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;
    let client = reqwest::Client::new();

    // Submit an assignment first
    let info: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    let shares_out: BigInt = info["shares_out"].as_str().unwrap().parse().unwrap();
    let fee_num: BigInt = info["fee_rate_num"].as_str().unwrap().parse().unwrap();
    let fee_den: BigInt = info["fee_rate_den"].as_str().unwrap().parse().unwrap();

    let utxo: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    let giver_amount: BigInt = utxo["amount"].as_str().unwrap().parse().unwrap();

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let giver_ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let recv_ts = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);

    let auth = build_authorization(
        &issuer, 1, &giver_amount, &receiver,
        &fee_num, &fee_den, &shares_out,
        giver_ts, recv_ts,
    );
    let auth_json = ao_json::to_json(&auth);

    client.post(format!("{}/chain/{}/submit", base, chain_id))
        .json(&auth_json)
        .send().await.unwrap();

    // Get blocks
    let blocks: Vec<serde_json::Value> = client
        .get(format!("{}/chain/{}/blocks?from=1&to=1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["type"], "BLOCK");

    // Empty range returns empty
    let empty: Vec<serde_json::Value> = client
        .get(format!("{}/chain/{}/blocks?from=5&to=10", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    assert!(empty.is_empty());
}

#[tokio::test]
async fn test_submit_invalid_json() {
    let issuer = SigningKey::from_seed(&[0x16; 32]);
    let blockmaker = SigningKey::from_seed(&[0x26; 32]);
    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/chain/{}/submit", base, chain_id))
        .header("content-type", "application/json")
        .body("not json")
        .send().await.unwrap();

    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_double_spend_via_api() {
    let issuer = SigningKey::from_seed(&[0x17; 32]);
    let blockmaker = SigningKey::from_seed(&[0x27; 32]);
    let receiver1 = SigningKey::generate();
    let receiver2 = SigningKey::generate();

    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;
    let client = reqwest::Client::new();

    let info: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    let shares_out: BigInt = info["shares_out"].as_str().unwrap().parse().unwrap();
    let fee_num: BigInt = info["fee_rate_num"].as_str().unwrap().parse().unwrap();
    let fee_den: BigInt = info["fee_rate_den"].as_str().unwrap().parse().unwrap();

    let utxo: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    let giver_amount: BigInt = utxo["amount"].as_str().unwrap().parse().unwrap();

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    // First submission — should succeed
    let auth1 = build_authorization(
        &issuer, 1, &giver_amount, &receiver1,
        &fee_num, &fee_den, &shares_out,
        Timestamp::from_raw(genesis_ts.raw() + 1_000_000),
        Timestamp::from_raw(genesis_ts.raw() + 2_000_000),
    );
    let resp1 = client
        .post(format!("{}/chain/{}/submit", base, chain_id))
        .json(&ao_json::to_json(&auth1))
        .send().await.unwrap();
    assert_eq!(resp1.status(), 200);

    // Second submission with same UTXO — should fail
    let auth2 = build_authorization(
        &issuer, 1, &giver_amount, &receiver2,
        &fee_num, &fee_den, &shares_out,
        Timestamp::from_raw(genesis_ts.raw() + 3_000_000),
        Timestamp::from_raw(genesis_ts.raw() + 4_000_000),
    );
    let resp2 = client
        .post(format!("{}/chain/{}/submit", base, chain_id))
        .json(&ao_json::to_json(&auth2))
        .send().await.unwrap();
    assert_eq!(resp2.status(), 400);

    let body: serde_json::Value = resp2.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("already spent"));
}

#[tokio::test]
async fn test_sse_block_notification() {
    let issuer = SigningKey::from_seed(&[0x18; 32]);
    let blockmaker = SigningKey::from_seed(&[0x28; 32]);
    let receiver = SigningKey::generate();

    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;
    let client = reqwest::Client::new();

    // Connect SSE before submitting
    let mut sse_resp = client
        .get(format!("{}/chain/{}/events", base, chain_id))
        .send().await.unwrap();
    assert_eq!(sse_resp.status(), 200);

    // Get chain info for the submission
    let info: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    let shares_out: BigInt = info["shares_out"].as_str().unwrap().parse().unwrap();
    let fee_num: BigInt = info["fee_rate_num"].as_str().unwrap().parse().unwrap();
    let fee_den: BigInt = info["fee_rate_den"].as_str().unwrap().parse().unwrap();

    let utxo: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    let giver_amount: BigInt = utxo["amount"].as_str().unwrap().parse().unwrap();

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let auth = build_authorization(
        &issuer, 1, &giver_amount, &receiver,
        &fee_num, &fee_den, &shares_out,
        Timestamp::from_raw(genesis_ts.raw() + 1_000_000),
        Timestamp::from_raw(genesis_ts.raw() + 2_000_000),
    );

    // Read SSE stream in a background task
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();

    let sse_task = tokio::spawn(async move {
        let mut buffer = String::new();
        // Read chunks from SSE response body
        while let Some(chunk) = sse_resp.chunk().await.unwrap() {
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            // Look for "data:" line in SSE
            if let Some(data_start) = buffer.find("data:") {
                let rest = &buffer[data_start + 5..];
                if let Some(end) = rest.find('\n') {
                    let data = rest[..end].trim().to_string();
                    let _ = tx.send(data);
                    return;
                }
            }
        }
    });

    // Small delay for SSE connection to be ready
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Submit assignment
    let resp = client
        .post(format!("{}/chain/{}/submit", base, chain_id))
        .json(&ao_json::to_json(&auth))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Wait for SSE notification
    let sse_data = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        rx,
    ).await.expect("SSE timeout").expect("SSE channel closed");

    let block_info: serde_json::Value = serde_json::from_str(&sse_data).unwrap();
    assert_eq!(block_info["height"], 1);
    assert!(block_info["hash"].as_str().unwrap().len() == 64);

    sse_task.abort();
}

#[tokio::test]
async fn test_websocket_block_notification() {
    let issuer = SigningKey::from_seed(&[0x19; 32]);
    let blockmaker = SigningKey::from_seed(&[0x29; 32]);
    let receiver = SigningKey::generate();

    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;
    let client = reqwest::Client::new();

    // Connect WebSocket
    let ws_url = format!("{}/chain/{}/ws", base.replace("http://", "ws://"), chain_id);
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(&ws_url)
        .await.expect("WebSocket connect failed");

    // Get chain info
    let info: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();

    let shares_out: BigInt = info["shares_out"].as_str().unwrap().parse().unwrap();
    let fee_num: BigInt = info["fee_rate_num"].as_str().unwrap().parse().unwrap();
    let fee_den: BigInt = info["fee_rate_den"].as_str().unwrap().parse().unwrap();

    let utxo: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    let giver_amount: BigInt = utxo["amount"].as_str().unwrap().parse().unwrap();

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let auth = build_authorization(
        &issuer, 1, &giver_amount, &receiver,
        &fee_num, &fee_den, &shares_out,
        Timestamp::from_raw(genesis_ts.raw() + 1_000_000),
        Timestamp::from_raw(genesis_ts.raw() + 2_000_000),
    );

    // Submit assignment
    let resp = client
        .post(format!("{}/chain/{}/submit", base, chain_id))
        .json(&ao_json::to_json(&auth))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Read WebSocket message
    use futures_util::StreamExt;
    let msg = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        ws_stream.next(),
    ).await.expect("WS timeout").expect("WS stream ended").expect("WS error");

    let text = msg.into_text().expect("expected text message");
    let block_info: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(block_info["height"], 1);
    assert!(block_info["hash"].as_str().unwrap().len() == 64);
}

// ============ Multi-chain tests ============

#[tokio::test]
async fn test_list_chains() {
    let issuer = SigningKey::from_seed(&[0x30; 32]);
    let blockmaker = SigningKey::from_seed(&[0x31; 32]);
    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();
    let resp: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();

    assert_eq!(resp.len(), 1);
    assert_eq!(resp[0]["chain_id"], chain_id);
    assert_eq!(resp[0]["symbol"], "TST");
    assert_eq!(resp[0]["block_height"], 0);
}

#[tokio::test]
async fn test_create_chain_via_api() {
    let blockmaker = SigningKey::from_seed(&[0x32; 32]);
    let base = start_empty_server(&blockmaker).await;

    let client = reqwest::Client::new();

    // List should be empty
    let chains: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();
    assert!(chains.is_empty());

    // Create a chain via POST /chains
    let issuer = SigningKey::from_seed(&[0x33; 32]);
    let genesis = build_genesis(&issuer);
    let genesis_json = ao_json::to_json(&genesis);

    let resp = client
        .post(format!("{}/chains", base))
        .json(&serde_json::json!({ "genesis": genesis_json }))
        .send().await.unwrap();

    assert_eq!(resp.status(), 201);
    let info: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(info["symbol"], "TST");
    assert_eq!(info["block_height"], 0);
    let chain_id = info["chain_id"].as_str().unwrap().to_string();

    // List should now have one chain
    let chains: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(chains.len(), 1);
    assert_eq!(chains[0]["chain_id"], chain_id);

    // Chain info endpoint should work
    let chain_info: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_id))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(chain_info["symbol"], "TST");
}

#[tokio::test]
async fn test_create_duplicate_chain_rejected() {
    let blockmaker = SigningKey::from_seed(&[0x34; 32]);
    let base = start_empty_server(&blockmaker).await;

    let client = reqwest::Client::new();
    let issuer = SigningKey::from_seed(&[0x35; 32]);
    let genesis = build_genesis(&issuer);
    let genesis_json = ao_json::to_json(&genesis);

    // First create should succeed
    let resp = client
        .post(format!("{}/chains", base))
        .json(&serde_json::json!({ "genesis": genesis_json }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 201);

    // Second create with same genesis should fail
    let resp2 = client
        .post(format!("{}/chains", base))
        .json(&serde_json::json!({ "genesis": genesis_json }))
        .send().await.unwrap();
    assert_eq!(resp2.status(), 409);

    let body: serde_json::Value = resp2.json().await.unwrap();
    assert!(body["error"].as_str().unwrap().contains("already hosted"));
}

#[tokio::test]
async fn test_multi_chain_independent_operations() {
    let blockmaker = SigningKey::from_seed(&[0x36; 32]);
    let base = start_empty_server(&blockmaker).await;
    let client = reqwest::Client::new();

    // Create two chains with different issuers
    let issuer_a = SigningKey::from_seed(&[0x37; 32]);
    let issuer_b = SigningKey::from_seed(&[0x38; 32]);
    let genesis_a = build_genesis_with_symbol(&issuer_a, "AAA");
    let genesis_b = build_genesis_with_symbol(&issuer_b, "BBB");

    let resp_a = client
        .post(format!("{}/chains", base))
        .json(&serde_json::json!({ "genesis": ao_json::to_json(&genesis_a) }))
        .send().await.unwrap();
    assert_eq!(resp_a.status(), 201);
    let info_a: serde_json::Value = resp_a.json().await.unwrap();
    let chain_a = info_a["chain_id"].as_str().unwrap().to_string();

    let resp_b = client
        .post(format!("{}/chains", base))
        .json(&serde_json::json!({ "genesis": ao_json::to_json(&genesis_b) }))
        .send().await.unwrap();
    assert_eq!(resp_b.status(), 201);
    let info_b: serde_json::Value = resp_b.json().await.unwrap();
    let chain_b = info_b["chain_id"].as_str().unwrap().to_string();

    // Both chains should be listed
    let chains: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(chains.len(), 2);

    // Each chain should have independent state
    let info_a2: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_a))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(info_a2["symbol"], "AAA");

    let info_b2: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_b))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(info_b2["symbol"], "BBB");

    // Submit an assignment on chain A
    let shares_out: BigInt = info_a2["shares_out"].as_str().unwrap().parse().unwrap();
    let fee_num: BigInt = info_a2["fee_rate_num"].as_str().unwrap().parse().unwrap();
    let fee_den: BigInt = info_a2["fee_rate_den"].as_str().unwrap().parse().unwrap();

    let utxo_a: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base, chain_a))
        .send().await.unwrap()
        .json().await.unwrap();
    let giver_amount: BigInt = utxo_a["amount"].as_str().unwrap().parse().unwrap();

    let receiver = SigningKey::generate();
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let auth = build_authorization(
        &issuer_a, 1, &giver_amount, &receiver,
        &fee_num, &fee_den, &shares_out,
        Timestamp::from_raw(genesis_ts.raw() + 1_000_000),
        Timestamp::from_raw(genesis_ts.raw() + 2_000_000),
    );

    let resp = client
        .post(format!("{}/chain/{}/submit", base, chain_a))
        .json(&ao_json::to_json(&auth))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Chain A should be at height 1, chain B still at 0
    let info_a3: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_a))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(info_a3["block_height"], 1);

    let info_b3: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base, chain_b))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(info_b3["block_height"], 0);
}

// ============ Exchange agent registration tests ============

#[tokio::test]
async fn test_exchange_agent_registration() {
    let issuer = SigningKey::from_seed(&[0x40; 32]);
    let blockmaker = SigningKey::from_seed(&[0x41; 32]);
    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();

    // Register an exchange agent
    let resp = client
        .post(format!("{}/chain/{}/exchange-agent", base, chain_id))
        .json(&serde_json::json!({
            "name": "Charlie",
            "pairs": [
                { "sell_symbol": "TST", "buy_symbol": "CCC", "rate": 3.0 }
            ]
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // List chains should include the exchange agent
    let chains: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(chains.len(), 1);
    let agents = chains[0]["exchange_agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["name"], "Charlie");
    assert_eq!(agents[0]["pairs"][0]["sell_symbol"], "TST");
    assert_eq!(agents[0]["pairs"][0]["rate"], 3.0);

    // Re-register same agent with updated rate — should replace
    let resp = client
        .post(format!("{}/chain/{}/exchange-agent", base, chain_id))
        .json(&serde_json::json!({
            "name": "Charlie",
            "pairs": [
                { "sell_symbol": "TST", "buy_symbol": "CCC", "rate": 4.0 }
            ]
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let chains: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();
    let agents = chains[0]["exchange_agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1); // still 1, not 2
    assert_eq!(agents[0]["pairs"][0]["rate"], 4.0);

    // Register a second agent
    let resp = client
        .post(format!("{}/chain/{}/exchange-agent", base, chain_id))
        .json(&serde_json::json!({
            "name": "Eve",
            "pairs": [
                { "sell_symbol": "TST", "buy_symbol": "MFF", "rate": 2.0 }
            ]
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let chains: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();
    let agents = chains[0]["exchange_agents"].as_array().unwrap();
    assert_eq!(agents.len(), 2);

    // Non-existent chain should fail
    let resp = client
        .post(format!("{}/chain/{}/exchange-agent", base, "deadbeef"))
        .json(&serde_json::json!({ "name": "X", "pairs": [] }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 404);
}

/// Test that exchange agent registration supports extended fields:
/// contact_url, spread, min_trade, max_trade, TTL, and registered_at.
#[tokio::test]
async fn test_exchange_agent_extended_fields() {
    let issuer = SigningKey::from_seed(&[0x50; 32]);
    let blockmaker = SigningKey::from_seed(&[0x51; 32]);
    let (base, chain_id) = start_test_server(&issuer, &blockmaker).await;

    let client = reqwest::Client::new();

    // Register with extended fields
    let resp = client
        .post(format!("{}/chain/{}/exchange-agent", base, chain_id))
        .json(&serde_json::json!({
            "name": "Mako",
            "pairs": [
                {
                    "sell_symbol": "ENRA",
                    "buy_symbol": "TGS",
                    "rate": 1.5,
                    "spread": 0.03,
                    "min_trade": 100,
                    "max_trade": 50000
                }
            ],
            "contact_url": "http://localhost:3100/trade",
            "ttl": 7200
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200);

    // Verify fields come back in listing
    let chains: Vec<serde_json::Value> = client
        .get(format!("{}/chains", base))
        .send().await.unwrap()
        .json().await.unwrap();
    let agents = chains[0]["exchange_agents"].as_array().unwrap();
    assert_eq!(agents.len(), 1);
    assert_eq!(agents[0]["name"], "Mako");
    assert_eq!(agents[0]["contact_url"], "http://localhost:3100/trade");
    assert_eq!(agents[0]["ttl"], 7200);
    assert!(agents[0]["registered_at"].as_u64().unwrap() > 0);

    let pair = &agents[0]["pairs"][0];
    assert_eq!(pair["spread"], 0.03);
    assert_eq!(pair["min_trade"], 100);
    assert_eq!(pair["max_trade"], 50000);
}

// ============ CAA escrow tests ============

/// Start a server with known_recorders configured.
/// Returns (base_url, chain_id_hex, chain_id_bytes).
async fn start_caa_server(
    issuer_key: &SigningKey,
    blockmaker_key: &SigningKey,
    known_recorders: std::collections::HashMap<[u8; 32], [u8; 32]>,
) -> (String, String, [u8; 32]) {
    let store = ChainStore::open_memory().unwrap();
    let genesis_item = build_genesis(issuer_key);
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();
    let chain_id_hex = hex::encode(meta.chain_id);
    let chain_id_bytes = meta.chain_id;

    let mut state = AppState::new(store, SigningKey::from_seed(blockmaker_key.seed()));
    state.known_recorders = known_recorders;
    let state = Arc::new(state);

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, chain_id_hex, chain_id_bytes)
}

/// Build a CAA DataItem for a two-chain atomic exchange.
fn build_caa_for_test(
    chain_a_id: &[u8; 32],
    chain_b_id: &[u8; 32],
    giver_a_key: &SigningKey,
    giver_a_seq: u64,
    giver_a_amount: &BigInt,
    receiver_a_key: &SigningKey,
    giver_b_key: &SigningKey,
    giver_b_seq: u64,
    giver_b_amount: &BigInt,
    receiver_b_key: &SigningKey,
    fee_rate_num: &BigInt,
    fee_rate_den: &BigInt,
    shares_out: &BigInt,
    escrow_deadline: Timestamp,
) -> (DataItem, BigInt, BigInt) {
    let bid = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
    let mut bid_bytes = Vec::new();
    bigint::encode_rational(&bid, &mut bid_bytes);

    let assignment_deadline = Timestamp::from_unix_seconds(1_772_611_200 + 86400);

    // Placeholder AUTH_SIGs for fee calculation (same size as real ones)
    let placeholder_sigs = vec![
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, vec![0u8; 64]),
            DataItem::bytes(TIMESTAMP, vec![0u8; 8]),
            DataItem::vbc_value(PAGE_INDEX, 0),
        ]),
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, vec![0u8; 64]),
            DataItem::bytes(TIMESTAMP, vec![0u8; 8]),
            DataItem::vbc_value(PAGE_INDEX, 1),
        ]),
    ];

    // Calculate receiver amounts including AUTH_SIG sizes in fee
    let mut recv_a_amount = giver_a_amount.clone();
    for _ in 0..3 {
        let assignment_a = build_caa_assignment(
            giver_a_seq, giver_a_amount, &recv_a_amount, receiver_a_key,
            &bid_bytes, assignment_deadline,
        );
        let mut auth_children = vec![assignment_a];
        auth_children.extend(placeholder_sigs.clone());
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            DataItem::container(AUTHORIZATION, auth_children),
        ]);
        let page_bytes = page.to_bytes().len() as u64;
        let fee = fees::recording_fee(page_bytes, fee_rate_num, fee_rate_den, shares_out);
        recv_a_amount = giver_a_amount - &fee;
    }

    let mut recv_b_amount = giver_b_amount.clone();
    for _ in 0..3 {
        let assignment_b = build_caa_assignment(
            giver_b_seq, giver_b_amount, &recv_b_amount, receiver_b_key,
            &bid_bytes, assignment_deadline,
        );
        let mut auth_children = vec![assignment_b];
        auth_children.extend(placeholder_sigs.clone());
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            DataItem::container(AUTHORIZATION, auth_children),
        ]);
        let page_bytes = page.to_bytes().len() as u64;
        let fee = fees::recording_fee(page_bytes, fee_rate_num, fee_rate_den, shares_out);
        recv_b_amount = giver_b_amount - &fee;
    }

    let assignment_a = build_caa_assignment(
        giver_a_seq, giver_a_amount, &recv_a_amount, receiver_a_key,
        &bid_bytes, assignment_deadline,
    );
    let assignment_b = build_caa_assignment(
        giver_b_seq, giver_b_amount, &recv_b_amount, receiver_b_key,
        &bid_bytes, assignment_deadline,
    );

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    // Sign component A: giver_a + receiver_a sign the assignment
    let giver_a_ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let recv_a_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);
    let giver_a_sig = sign::sign_dataitem(giver_a_key, &assignment_a, giver_a_ts);
    let recv_a_sig = sign::sign_dataitem(receiver_a_key, &assignment_a, recv_a_ts);

    // Sign component B: giver_b + receiver_b sign the assignment
    let giver_b_ts = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let recv_b_ts = Timestamp::from_raw(genesis_ts.raw() + 4_000_000);
    let giver_b_sig = sign::sign_dataitem(giver_b_key, &assignment_b, giver_b_ts);
    let recv_b_sig = sign::sign_dataitem(receiver_b_key, &assignment_b, recv_b_ts);

    // Build CAA components
    let comp_a = DataItem::container(CAA_COMPONENT, vec![
        DataItem::bytes(CHAIN_REF, chain_a_id.to_vec()),
        DataItem::vbc_value(CHAIN_ORDER, 0),
        assignment_a,
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, giver_a_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, giver_a_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 0),
        ]),
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, recv_a_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, recv_a_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 1),
        ]),
    ]);

    let comp_b = DataItem::container(CAA_COMPONENT, vec![
        DataItem::bytes(CHAIN_REF, chain_b_id.to_vec()),
        DataItem::vbc_value(CHAIN_ORDER, 1),
        assignment_b,
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, giver_b_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, giver_b_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 0),
        ]),
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, recv_b_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, recv_b_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 1),
        ]),
    ]);

    // Build canonical CAA (for overall signing)
    let canonical_children = vec![
        DataItem::bytes(ESCROW_DEADLINE, escrow_deadline.to_bytes().to_vec()),
        DataItem::vbc_value(LIST_SIZE, 2u64),
        comp_a.clone(),
        comp_b.clone(),
    ];
    let canonical = DataItem::container(CAA, canonical_children.clone());

    // Overall signatures: all 4 participants sign the canonical CAA
    let overall_ts_base = Timestamp::from_raw(genesis_ts.raw() + 10_000_000);
    let mut overall_sigs = Vec::new();
    for (i, key) in [giver_a_key, receiver_a_key, giver_b_key, receiver_b_key].iter().enumerate() {
        let ts = Timestamp::from_raw(overall_ts_base.raw() + i as i64);
        let sig = sign::sign_dataitem(key, &canonical, ts);
        overall_sigs.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
        ]));
    }

    let mut caa_children = canonical_children;
    caa_children.extend(overall_sigs);

    (DataItem::container(CAA, caa_children), recv_a_amount, recv_b_amount)
}

fn build_caa_assignment(
    giver_seq: u64,
    giver_amount: &BigInt,
    receiver_amount: &BigInt,
    receiver_key: &SigningKey,
    bid_bytes: &[u8],
    deadline: Timestamp,
) -> DataItem {
    let mut giver_amount_bytes = Vec::new();
    bigint::encode_bigint(giver_amount, &mut giver_amount_bytes);
    let mut recv_amount_bytes = Vec::new();
    bigint::encode_bigint(receiver_amount, &mut recv_amount_bytes);

    DataItem::container(ASSIGNMENT, vec![
        DataItem::vbc_value(LIST_SIZE, 2),
        DataItem::container(PARTICIPANT, vec![
            DataItem::vbc_value(SEQ_ID, giver_seq),
            DataItem::bytes(AMOUNT, giver_amount_bytes),
        ]),
        DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, receiver_key.public_key_bytes().to_vec()),
            DataItem::bytes(AMOUNT, recv_amount_bytes),
        ]),
        DataItem::bytes(RECORDING_BID, bid_bytes.to_vec()),
        DataItem::bytes(DEADLINE, deadline.to_bytes().to_vec()),
    ])
}

#[tokio::test]
async fn test_caa_submit_and_status() {
    // Two independent chains, same recorder
    let issuer_a = SigningKey::from_seed(&[0x50; 32]);
    let issuer_b = SigningKey::from_seed(&[0x51; 32]);
    let blockmaker = SigningKey::from_seed(&[0x52; 32]);
    let receiver_a = SigningKey::generate();
    let receiver_b = SigningKey::generate();

    // Start chain A
    let (base_a, chain_a_hex, chain_a_bytes) =
        start_caa_server(&issuer_a, &blockmaker, std::collections::HashMap::new()).await;

    // Get chain A info
    let client = reqwest::Client::new();
    let info_a: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base_a, chain_a_hex))
        .send().await.unwrap()
        .json().await.unwrap();
    let shares_out: BigInt = info_a["shares_out"].as_str().unwrap().parse().unwrap();
    let fee_num: BigInt = info_a["fee_rate_num"].as_str().unwrap().parse().unwrap();
    let fee_den: BigInt = info_a["fee_rate_den"].as_str().unwrap().parse().unwrap();
    let giver_a_amount: BigInt = {
        let utxo: serde_json::Value = client
            .get(format!("{}/chain/{}/utxo/1", base_a, chain_a_hex))
            .send().await.unwrap()
            .json().await.unwrap();
        utxo["amount"].as_str().unwrap().parse().unwrap()
    };

    // Start chain B — needs to know chain A's recorder pubkey
    let mut known_b = std::collections::HashMap::new();
    known_b.insert(chain_a_bytes, {
        let mut pk = [0u8; 32];
        pk.copy_from_slice(blockmaker.public_key_bytes());
        pk
    });
    let (base_b, chain_b_hex, chain_b_bytes) =
        start_caa_server(&issuer_b, &blockmaker, known_b).await;

    let _info_b: serde_json::Value = client
        .get(format!("{}/chain/{}/info", base_b, chain_b_hex))
        .send().await.unwrap()
        .json().await.unwrap();
    let giver_b_amount: BigInt = {
        let utxo: serde_json::Value = client
            .get(format!("{}/chain/{}/utxo/1", base_b, chain_b_hex))
            .send().await.unwrap()
            .json().await.unwrap();
        utxo["amount"].as_str().unwrap().parse().unwrap()
    };

    // Build CAA
    let escrow_deadline = Timestamp::from_unix_seconds(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap()
            .as_secs() as i64 + 300
    );
    let (caa, _recv_a_amount, _recv_b_amount) = build_caa_for_test(
        &chain_a_bytes, &chain_b_bytes,
        &issuer_a, 1, &giver_a_amount, &receiver_a,
        &issuer_b, 1, &giver_b_amount, &receiver_b,
        &fee_num, &fee_den, &shares_out,
        escrow_deadline,
    );
    let caa_json = ao_json::to_json(&caa);

    // Step 1: Submit to chain A (order 0, no prior proofs needed)
    let resp_a = client
        .post(format!("{}/chain/{}/caa/submit", base_a, chain_a_hex))
        .json(&caa_json)
        .send().await.unwrap();

    assert_eq!(resp_a.status(), 200, "caa_submit chain A failed: {}",
        resp_a.text().await.unwrap_or_default());

    // Re-read: need the actual response
    let resp_a = client
        .post(format!("{}/chain/{}/caa/submit", base_a, chain_a_hex))
        .json(&caa_json)
        .send().await.unwrap();
    assert_eq!(resp_a.status(), 200); // idempotent re-submit
    let proof_a: serde_json::Value = resp_a.json().await.unwrap();
    let caa_hash_hex = proof_a["caa_hash"].as_str().unwrap().to_string();

    // Check escrow status on chain A
    let status_a: serde_json::Value = client
        .get(format!("{}/chain/{}/caa/{}", base_a, chain_a_hex, caa_hash_hex))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(status_a["status"], "escrowed");
    assert_eq!(status_a["chain_order"], 0);

    // Verify giver UTXO on chain A is now escrowed
    let utxo_a: serde_json::Value = client
        .get(format!("{}/chain/{}/utxo/1", base_a, chain_a_hex))
        .send().await.unwrap()
        .json().await.unwrap();
    assert_eq!(utxo_a["status"], "Escrowed");
}

#[tokio::test]
async fn test_refutation_endpoint() {
    let issuer_key = SigningKey::from_seed(&[0x42u8; 32]);
    let blockmaker_key = SigningKey::from_seed(&[0x99u8; 32]);
    let (base_url, chain_id) = start_test_server(&issuer_key, &blockmaker_key).await;

    // Build an assignment that we will refute (not submit — just hash)
    let receiver_key = SigningKey::from_seed(&[0xABu8; 32]);
    let giver_amount = BigInt::from(1u64 << 40);
    let giver_ts = Timestamp::from_unix_seconds(1_772_611_200 + 100);
    let recv_ts = Timestamp::from_unix_seconds(1_772_611_200 + 101);
    let auth = build_authorization(
        &issuer_key, 1, &giver_amount,
        &receiver_key,
        &BigInt::from(1), &BigInt::from(1_000_000),
        &giver_amount,
        giver_ts, recv_ts,
    );

    // Compute the agreement hash (hash of the ASSIGNMENT, not AUTHORIZATION)
    let assignment = auth.find_child(ASSIGNMENT).unwrap();
    let agreement_hash = hash::sha256(&assignment.to_bytes());
    let hash_hex = hex::encode(agreement_hash);

    let client = reqwest::Client::new();

    // Submit refutation
    let resp = client
        .post(format!("{}/chain/{}/refute", base_url, chain_id))
        .json(&serde_json::json!({ "agreement_hash": hash_hex }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 200, "refutation failed: {}", resp.text().await.unwrap_or_default());

    // Idempotent: re-submit should also succeed
    let resp2 = client
        .post(format!("{}/chain/{}/refute", base_url, chain_id))
        .json(&serde_json::json!({ "agreement_hash": hash_hex }))
        .send().await.unwrap();
    assert_eq!(resp2.status(), 200);

    // Bad hex should fail
    let resp3 = client
        .post(format!("{}/chain/{}/refute", base_url, chain_id))
        .json(&serde_json::json!({ "agreement_hash": "not-hex" }))
        .send().await.unwrap();
    assert_eq!(resp3.status(), 400);

    // Wrong length should fail
    let resp4 = client
        .post(format!("{}/chain/{}/refute", base_url, chain_id))
        .json(&serde_json::json!({ "agreement_hash": "aabb" }))
        .send().await.unwrap();
    assert_eq!(resp4.status(), 400);
}
