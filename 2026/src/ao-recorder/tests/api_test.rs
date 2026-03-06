/// Integration tests for the ao-recorder HTTP API.
/// Spins up an in-process Axum server and tests all endpoints.
use std::sync::{Arc, Mutex};

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
        DataItem::bytes(CHAIN_SYMBOL, b"TST".to_vec()),
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

    let state = Arc::new(AppState {
        store: Mutex::new(store),
        blockmaker_key: SigningKey::from_seed(blockmaker_key.seed()),
    });

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, chain_id)
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
