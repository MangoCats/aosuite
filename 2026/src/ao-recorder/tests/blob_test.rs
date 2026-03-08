/// Integration tests for the blob storage HTTP API.
use std::sync::Arc;

use num_bigint::BigInt;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_crypto::hash;
use ao_crypto::sign::{self, SigningKey};

use ao_chain::store::ChainStore;
use ao_chain::genesis;

use ao_recorder::{AppState, blob, build_router};

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
        DataItem::bytes(DESCRIPTION, b"Blob test chain".to_vec()),
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

/// Start a test server with blob storage enabled.
async fn start_blob_server(issuer_key: &SigningKey, blockmaker_key: &SigningKey) -> (String, String) {
    let store = ChainStore::open_memory().unwrap();
    let genesis_item = build_genesis(issuer_key);
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();
    let chain_id = hex::encode(meta.chain_id);

    let tmp_dir = tempfile::tempdir().unwrap();
    let blob_dir = tmp_dir.path().join("blobs");
    let blob_store = blob::BlobStore::new(blob_dir, 5_242_880).unwrap();

    let mut state = AppState::new(store, SigningKey::from_seed(blockmaker_key.seed()));
    state.blob_store = Some(blob_store);
    let state = Arc::new(state);

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    // Leak the tempdir so it lives for the test duration.
    std::mem::forget(tmp_dir);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, chain_id)
}

/// Start a test server with a small blob limit for rejection tests.
async fn start_small_blob_server(issuer_key: &SigningKey, blockmaker_key: &SigningKey) -> (String, String) {
    let store = ChainStore::open_memory().unwrap();
    let genesis_item = build_genesis(issuer_key);
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();
    let chain_id = hex::encode(meta.chain_id);

    let tmp_dir = tempfile::tempdir().unwrap();
    let blob_dir = tmp_dir.path().join("blobs");
    let blob_store = blob::BlobStore::new(blob_dir, 100).unwrap();

    let mut state = AppState::new(store, SigningKey::from_seed(blockmaker_key.seed()));
    state.blob_store = Some(blob_store);
    let state = Arc::new(state);

    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{}", addr);

    std::mem::forget(tmp_dir);

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    (base_url, chain_id)
}

#[tokio::test]
async fn test_blob_upload_and_retrieve() {
    let issuer_key = SigningKey::generate();
    let blockmaker_key = SigningKey::generate();
    let (base_url, chain_id) = start_blob_server(&issuer_key, &blockmaker_key).await;

    let client = reqwest::Client::new();

    // Upload a blob with MIME prefix.
    let blob_data = b"image/png\0\x89PNG fake image content here";
    let resp = client
        .post(format!("{}/chain/{}/blob", base_url, chain_id))
        .body(blob_data.to_vec())
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let hash = body["hash"].as_str().unwrap();
    assert_eq!(hash.len(), 64);

    // Retrieve the blob.
    let resp = client
        .get(format!("{}/chain/{}/blob/{}", base_url, chain_id, hash))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(content_type, "image/png");

    let cache_control = resp.headers().get("cache-control").unwrap().to_str().unwrap();
    assert_eq!(cache_control, "public, max-age=31536000, immutable");

    let content = resp.bytes().await.unwrap();
    assert_eq!(&content[..], b"\x89PNG fake image content here");
}

#[tokio::test]
async fn test_blob_too_large_413() {
    let issuer_key = SigningKey::generate();
    let blockmaker_key = SigningKey::generate();
    let (base_url, chain_id) = start_small_blob_server(&issuer_key, &blockmaker_key).await;

    let client = reqwest::Client::new();

    // Build a blob that exceeds the 100-byte limit (after MIME prefix).
    let mut blob_data = b"image/png\0".to_vec();
    blob_data.extend_from_slice(&[0xAA; 200]);

    let resp = client
        .post(format!("{}/chain/{}/blob", base_url, chain_id))
        .body(blob_data)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 413);
}

#[tokio::test]
async fn test_blob_not_found_404() {
    let issuer_key = SigningKey::generate();
    let blockmaker_key = SigningKey::generate();
    let (base_url, chain_id) = start_blob_server(&issuer_key, &blockmaker_key).await;

    let client = reqwest::Client::new();

    let fake_hash = "a".repeat(64);
    let resp = client
        .get(format!("{}/chain/{}/blob/{}", base_url, chain_id, fake_hash))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 500); // IoError wrapping NotFound
}
