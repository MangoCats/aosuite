/// Integration tests for the full chain flow:
/// genesis → assignment → block construction → validation.
use num_bigint::BigInt;
use num_traits::Zero;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_types::fees;
use ao_crypto::hash;
use ao_crypto::sign::{self, SigningKey};

use ao_chain::genesis;
use ao_chain::store::{ChainStore, UtxoStatus};
use ao_chain::validate;
use ao_chain::block;

/// Helper: build a genesis block with the given key.
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

    let expiry_period = Timestamp::from_unix_seconds(31_536_000); // 1 year
    let ts = Timestamp::from_unix_seconds(1_772_611_200); // 2026-03-06

    let signable_children = vec![
        DataItem::vbc_value(PROTOCOL_VER, 1),
        DataItem::bytes(CHAIN_SYMBOL, b"TST".to_vec()),
        DataItem::bytes(DESCRIPTION, b"Integration test chain".to_vec()),
        DataItem::bytes(COIN_COUNT, coin_bytes),
        DataItem::bytes(SHARES_OUT, shares_bytes.clone()),
        DataItem::bytes(FEE_RATE, fee_bytes),
        DataItem::bytes(EXPIRY_PERIOD, expiry_period.to_bytes().to_vec()),
        DataItem::vbc_value(EXPIRY_MODE, 1),
        DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, pubkey.to_vec()),
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

/// Build a signed AUTHORIZATION, computing receiver_amount as giver_amount - actual_fee.
/// Uses iterative approach: build the auth, compute page size, compute fee, rebuild with correct amounts.
fn build_authorization(
    giver_key: &SigningKey,
    giver_seq_id: u64,
    giver_amount: &BigInt,
    receiver_key: &SigningKey,
    fee_rate_num: &BigInt,
    fee_rate_den: &BigInt,
    shares_out: &BigInt,
    deadline: Timestamp,
    giver_sign_ts: Timestamp,
    receiver_sign_ts: Timestamp,
) -> (DataItem, BigInt) {
    // recording_bid = fee_rate (same as chain)
    let bid = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
    let mut bid_bytes = Vec::new();
    bigint::encode_rational(&bid, &mut bid_bytes);

    // Iterative: build with estimated receiver_amount, compute actual fee, rebuild.
    // Two iterations is always enough since amounts change by at most a few bytes.
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

    let auth = build_auth_inner(
        giver_key, giver_seq_id, giver_amount,
        receiver_key, &receiver_amount,
        &bid_bytes, deadline, giver_sign_ts, receiver_sign_ts,
    );
    (auth, receiver_amount)
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

#[test]
fn test_full_chain_flow() {
    // Create keys
    let issuer_key = SigningKey::from_seed(&[0x01; 32]);
    let receiver_key = SigningKey::generate();
    let blockmaker_key = SigningKey::from_seed(&[0x99; 32]);

    // Build and load genesis
    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    assert_eq!(meta.symbol, "TST");
    assert_eq!(meta.block_height, 0);
    assert_eq!(meta.next_seq_id, 2);

    let issuer_utxo = store.get_utxo(1).unwrap().unwrap();
    assert_eq!(issuer_utxo.status, UtxoStatus::Unspent);
    let total_shares = issuer_utxo.amount.clone();

    // Timestamps for signing
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let giver_ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let receiver_ts = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600); // 1 hour later
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    // Build the authorization (computes correct fee iteratively)
    let (auth, _receiver_amount) = build_authorization(
        &issuer_key, 1, &total_shares,
        &receiver_key,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, giver_ts, receiver_ts,
    );

    // Validate the assignment
    let validated = validate::validate_assignment(
        &store, &meta, &auth, block_ts.raw(),
    ).unwrap();

    assert_eq!(validated.givers.len(), 1);
    assert_eq!(validated.receivers.len(), 1);
    assert!(validated.fee_shares > BigInt::zero());

    // Now the actual fee might differ from our estimate because page size differs.
    // Re-build with correct amounts if needed. For this test, we accept the validation
    // result's fee_shares and check the balance equation.
    // The validate function already checked the balance equation, so if it passed, we're good.

    // Construct the block
    let constructed = block::construct_block(
        &store, &meta, &blockmaker_key,
        vec![validated],
        block_ts.raw(),
    ).unwrap();

    assert_eq!(constructed.height, 1);
    assert_eq!(constructed.first_seq, 2); // seq 2 for the receiver
    assert_eq!(constructed.seq_count, 1);
    assert!(constructed.new_shares_out < meta.shares_out); // fees retired

    // Verify the block round-trips
    let decoded = DataItem::from_bytes(&constructed.block_bytes).unwrap();
    assert_eq!(decoded, constructed.block);

    // Verify UTXO states
    let old_utxo = store.get_utxo(1).unwrap().unwrap();
    assert_eq!(old_utxo.status, UtxoStatus::Spent);

    let new_utxo = store.get_utxo(2).unwrap().unwrap();
    assert_eq!(new_utxo.status, UtxoStatus::Unspent);
    let expected_pk: [u8; 32] = receiver_key.public_key_bytes().try_into().unwrap();
    assert_eq!(new_utxo.pubkey, expected_pk);

    // Verify chain state was advanced
    let updated_meta = store.load_chain_meta().unwrap().unwrap();
    assert_eq!(updated_meta.block_height, 1);
    assert_eq!(updated_meta.next_seq_id, 3);
    assert_eq!(updated_meta.prev_hash, constructed.block_hash);
}

#[test]
fn test_double_spend_rejected() {
    let issuer_key = SigningKey::from_seed(&[0x02; 32]);
    let receiver1 = SigningKey::generate();
    let receiver2 = SigningKey::generate();
    let blockmaker = SigningKey::from_seed(&[0xAA; 32]);

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    // First assignment: spend seq 1
    let (auth1, _) = build_authorization(
        &issuer_key, 1, &total, &receiver1,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );
    let va1 = validate::validate_assignment(&store, &meta, &auth1, block_ts.raw()).unwrap();
    block::construct_block(&store, &meta, &blockmaker, vec![va1], block_ts.raw()).unwrap();

    // Second assignment: try to spend seq 1 again
    let sign_ts3 = Timestamp::from_raw(genesis_ts.raw() + 4_000_000);
    let sign_ts4 = Timestamp::from_raw(genesis_ts.raw() + 5_000_000);
    let block_ts2 = Timestamp::from_raw(genesis_ts.raw() + 6_000_000);
    let (auth2, _) = build_authorization(
        &issuer_key, 1, &total, &receiver2,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts3, sign_ts4,
    );

    let updated_meta = store.load_chain_meta().unwrap().unwrap();
    let result = validate::validate_assignment(&store, &updated_meta, &auth2, block_ts2.raw());
    assert!(result.is_err());
    // Should be UtxoAlreadySpent
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("already spent"), "Expected 'already spent', got: {}", err_msg);
}

#[test]
fn test_key_reuse_rejected() {
    let issuer_key = SigningKey::from_seed(&[0x03; 32]);
    let receiver = SigningKey::generate();
    let blockmaker = SigningKey::from_seed(&[0xBB; 32]);

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    // Record first assignment
    let (auth1, _) = build_authorization(
        &issuer_key, 1, &total, &receiver,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );
    let va1 = validate::validate_assignment(&store, &meta, &auth1, block_ts.raw()).unwrap();
    block::construct_block(&store, &meta, &blockmaker, vec![va1], block_ts.raw()).unwrap();

    // Try to use the same receiver key again (self-assign from seq 2 to same key)
    let updated_meta = store.load_chain_meta().unwrap().unwrap();
    let new_amount = store.get_utxo(2).unwrap().unwrap().amount;

    let sign_ts3 = Timestamp::from_raw(genesis_ts.raw() + 4_000_000);
    let sign_ts4 = Timestamp::from_raw(genesis_ts.raw() + 5_000_000);
    let block_ts2 = Timestamp::from_raw(genesis_ts.raw() + 6_000_000);

    let (auth2, _) = build_authorization(
        &receiver, 2, &new_amount, &receiver,
        &updated_meta.fee_rate_num, &updated_meta.fee_rate_den, &updated_meta.shares_out,
        deadline, sign_ts3, sign_ts4,
    );

    let result = validate::validate_assignment(&store, &updated_meta, &auth2, block_ts2.raw());
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("key already used"), "Expected 'key already used', got: {}", err_msg);
}
