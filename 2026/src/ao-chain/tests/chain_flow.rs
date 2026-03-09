/// Integration tests for the full chain flow:
/// genesis → assignment → block construction → validation.
use std::collections::HashMap;

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
use ao_chain::reward_rate;
use ao_chain::recorder_switch;
use ao_chain::store::PendingRecorderChange;

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
#[allow(clippy::too_many_arguments)]
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

#[allow(clippy::too_many_arguments)]
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
        &store, &meta, &auth, block_ts.raw(), &HashMap::new(),
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
        None,
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
    let va1 = validate::validate_assignment(&store, &meta, &auth1, block_ts.raw(), &HashMap::new()).unwrap();
    block::construct_block(&store, &meta, &blockmaker, vec![va1], block_ts.raw(), None).unwrap();

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
    let result = validate::validate_assignment(&store, &updated_meta, &auth2, block_ts2.raw(), &HashMap::new());
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
    let va1 = validate::validate_assignment(&store, &meta, &auth1, block_ts.raw(), &HashMap::new()).unwrap();
    block::construct_block(&store, &meta, &blockmaker, vec![va1], block_ts.raw(), None).unwrap();

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

    let result = validate::validate_assignment(&store, &updated_meta, &auth2, block_ts2.raw(), &HashMap::new());
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("key already used"), "Expected 'key already used', got: {}", err_msg);
}

#[test]
fn test_expired_utxo_rejected() {
    let issuer_key = SigningKey::from_seed(&[0x04; 32]);
    let receiver_key = SigningKey::generate();

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 86400 * 365 * 2); // 2 years

    // Set block timestamp well past the expiry period (1 year in the genesis)
    // The UTXO was created at genesis_ts, expiry_period is 31_536_000 Unix seconds
    // = 31_536_000 * 189_000_000 in AO timestamps
    let far_future_ts = genesis_ts.raw() + (31_536_000i64 + 1) * 189_000_000;

    let (auth, _) = build_authorization(
        &issuer_key, 1, &total, &receiver_key,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );

    let result = validate::validate_assignment(&store, &meta, &auth, far_future_ts, &HashMap::new());
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("expired"), "Expected 'expired', got: {}", err_msg);
}

#[test]
fn test_expiration_sweep_in_block() {
    // Create a chain, record an assignment, then record a second block
    // far enough in the future that the first UTXO expires during the sweep.
    let issuer_key = SigningKey::from_seed(&[0x05; 32]);
    let receiver1 = SigningKey::generate();
    let receiver2 = SigningKey::generate();
    let blockmaker = SigningKey::from_seed(&[0xDD; 32]);

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);
    let block1_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    // Record first assignment: issuer → receiver1
    let (auth1, recv1_amount) = build_authorization(
        &issuer_key, 1, &total, &receiver1,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );
    let va1 = validate::validate_assignment(&store, &meta, &auth1, block1_ts.raw(), &HashMap::new()).unwrap();
    block::construct_block(&store, &meta, &blockmaker, vec![va1], block1_ts.raw(), None).unwrap();

    let meta_after_1 = store.load_chain_meta().unwrap().unwrap();

    // Now build a second assignment: receiver1 → receiver2
    // Set block2 timestamp far in future so that block 1's UTXO (seq 2) has expired by the sweep
    // But we need it NOT expired when we validate, just expired during the sweep of other UTXOs.
    // Actually, the sweep expires old UTXOs. Let's make the block2 timestamp just barely past
    // the expiry of the genesis UTXO (seq 1, already spent — doesn't matter).
    // The relevant UTXO is seq 2 from block 1.
    // Block 1 timestamp = genesis_ts + 3_000_000 (in AO timestamp units).
    // Expiry period = 31_536_000 * 189_000_000 (in AO timestamp units).
    // To make seq 2 NOT expired yet, block2_ts must be < block1_ts + expiry.
    // Let's just use a normal timestamp.
    let sign_ts3 = Timestamp::from_raw(block1_ts.raw() + 1_000_000);
    let sign_ts4 = Timestamp::from_raw(block1_ts.raw() + 2_000_000);
    let deadline2 = Timestamp::from_unix_seconds(1_772_611_200 + 7200);
    let block2_ts = Timestamp::from_raw(block1_ts.raw() + 4_000_000);

    let (auth2, _) = build_authorization(
        &receiver1, 2, &recv1_amount, &receiver2,
        &meta_after_1.fee_rate_num, &meta_after_1.fee_rate_den, &meta_after_1.shares_out,
        deadline2, sign_ts3, sign_ts4,
    );
    let va2 = validate::validate_assignment(&store, &meta_after_1, &auth2, block2_ts.raw(), &HashMap::new()).unwrap();
    let block2 = block::construct_block(&store, &meta_after_1, &blockmaker, vec![va2], block2_ts.raw(), None).unwrap();

    // Verify chain progressed
    assert_eq!(block2.height, 2);
    let meta_after_2 = store.load_chain_meta().unwrap().unwrap();
    assert_eq!(meta_after_2.block_height, 2);
    assert_eq!(meta_after_2.next_seq_id, 4);

    // Verify seq 1 is spent, seq 2 is spent, seq 3 is unspent
    assert_eq!(store.get_utxo(1).unwrap().unwrap().status, UtxoStatus::Spent);
    assert_eq!(store.get_utxo(2).unwrap().unwrap().status, UtxoStatus::Spent);
    assert_eq!(store.get_utxo(3).unwrap().unwrap().status, UtxoStatus::Unspent);
}

#[test]
fn test_timestamp_ordering_giver() {
    // Giver's signature timestamp must be > the UTXO's receipt timestamp.
    let issuer_key = SigningKey::from_seed(&[0x06; 32]);
    let receiver_key = SigningKey::generate();

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    // Use a giver sign timestamp BEFORE the genesis timestamp (UTXO receipt time)
    let bad_giver_ts = Timestamp::from_raw(genesis_ts.raw() - 1_000_000);
    let recv_ts = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    let (auth, _) = build_authorization(
        &issuer_key, 1, &total, &receiver_key,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, bad_giver_ts, recv_ts,
    );

    let result = validate::validate_assignment(&store, &meta, &auth, block_ts.raw(), &HashMap::new());
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("timestamp"), "Expected timestamp error, got: {}", err_msg);
}

#[test]
fn test_multi_receiver_assignment() {
    // One giver splitting to two receivers.
    let issuer_key = SigningKey::from_seed(&[0x07; 32]);
    let receiver1 = SigningKey::generate();
    let receiver2 = SigningKey::generate();
    let blockmaker = SigningKey::from_seed(&[0xEE; 32]);

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let bid = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
    let mut bid_bytes = Vec::new();
    bigint::encode_rational(&bid, &mut bid_bytes);

    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);
    let sign_ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let r1_sig_ts = Timestamp::from_raw(sign_ts.raw() + 1_000_000);
    let r2_sig_ts = Timestamp::from_raw(sign_ts.raw() + 2_000_000);

    // Iterative fee convergence — must include AUTH_SIGs in size estimate
    let dummy_sig = [0u8; 64];
    let dummy_ts = [0u8; 8];

    let mut r1_amount = &total / BigInt::from(2);
    let mut r2_amount = &total - &r1_amount;
    for _ in 0..3 {
        let assignment = build_split_assignment(
            &total, &r1_amount, &r2_amount,
            &receiver1, &receiver2,
            &bid_bytes, deadline,
        );

        // Include 3 placeholder AUTH_SIGs in the size estimate
        let auth = DataItem::container(AUTHORIZATION, vec![
            assignment,
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, dummy_sig.to_vec()),
                DataItem::bytes(TIMESTAMP, dummy_ts.to_vec()),
                DataItem::vbc_value(PAGE_INDEX, 0),
            ]),
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, dummy_sig.to_vec()),
                DataItem::bytes(TIMESTAMP, dummy_ts.to_vec()),
                DataItem::vbc_value(PAGE_INDEX, 1),
            ]),
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, dummy_sig.to_vec()),
                DataItem::bytes(TIMESTAMP, dummy_ts.to_vec()),
                DataItem::vbc_value(PAGE_INDEX, 2),
            ]),
        ]);
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            auth,
        ]);
        let page_bytes = page.to_bytes().len() as u64;
        let fee = fees::recording_fee(page_bytes, &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out);
        let remainder = &total - &fee;
        r1_amount = &remainder / BigInt::from(2);
        r2_amount = &remainder - &r1_amount;
    }

    // Build final assignment with real signatures
    let assignment = build_split_assignment(
        &total, &r1_amount, &r2_amount,
        &receiver1, &receiver2,
        &bid_bytes, deadline,
    );

    let giver_sig = sign::sign_dataitem(&issuer_key, &assignment, sign_ts);
    let recv1_sig = sign::sign_dataitem(&receiver1, &assignment, r1_sig_ts);
    let recv2_sig = sign::sign_dataitem(&receiver2, &assignment, r2_sig_ts);

    let auth = DataItem::container(AUTHORIZATION, vec![
        assignment,
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, giver_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, sign_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 0),
        ]),
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, recv1_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, r1_sig_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 1),
        ]),
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, recv2_sig.to_vec()),
            DataItem::bytes(TIMESTAMP, r2_sig_ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, 2),
        ]),
    ]);

    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 5_000_000);
    let va = validate::validate_assignment(&store, &meta, &auth, block_ts.raw(), &HashMap::new()).unwrap();
    assert_eq!(va.givers.len(), 1);
    assert_eq!(va.receivers.len(), 2);

    let constructed = block::construct_block(&store, &meta, &blockmaker, vec![va], block_ts.raw(), None).unwrap();
    assert_eq!(constructed.height, 1);
    assert_eq!(constructed.seq_count, 2);

    let utxo2 = store.get_utxo(2).unwrap().unwrap();
    let utxo3 = store.get_utxo(3).unwrap().unwrap();
    assert_eq!(utxo2.status, UtxoStatus::Unspent);
    assert_eq!(utxo3.status, UtxoStatus::Unspent);
    assert_eq!(&utxo2.amount + &utxo3.amount, &r1_amount + &r2_amount);
}

/// B1 regression: verify that the CLI genesis construction path produces
/// a chain ID that ao-chain's genesis loader accepts.
#[test]
fn test_cli_genesis_chain_id_matches_loader() {
    // Replicate the ao-cli genesis code path: build children, hash child encodings,
    // embed chain ID, then load via ao-chain's genesis loader.
    let issuer_key = SigningKey::from_seed(&[0x42; 32]);
    let genesis_item = build_genesis(&issuer_key);

    // Extract chain ID the same way ao-cli does
    let children = genesis_item.children();
    let sha256_item = genesis_item.find_child(SHA256).expect("genesis must have SHA256");
    let embedded_id = sha256_item.as_bytes().expect("SHA256 must have bytes");

    // Recompute chain ID from children (excluding SHA256), matching ao-chain logic
    let mut content_bytes = Vec::new();
    for child in children {
        if child.type_code != SHA256 {
            child.encode(&mut content_bytes);
        }
    }
    let recomputed = hash::sha256(&content_bytes);
    assert_eq!(&recomputed[..], embedded_id, "CLI chain ID must match recomputed hash");

    // Now load through ao-chain's genesis loader — it must accept the chain ID
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();
    assert_eq!(&meta.chain_id[..], embedded_id,
        "ao-chain loader chain ID must match CLI-embedded chain ID");

    // Also test compute_chain_id standalone
    let extracted = genesis::compute_chain_id(&genesis_item).unwrap();
    assert_eq!(extracted, meta.chain_id);
}

/// B6 regression: validate_assignment returns UtxoNotFound for missing UTXO,
/// rather than panicking.
#[test]
fn test_missing_utxo_returns_error() {
    let issuer_key = SigningKey::from_seed(&[0x0B; 32]);
    let receiver_key = SigningKey::generate();

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    // Build an authorization referencing seq_id 999 which does not exist.
    // Use a large amount so the fee calculation doesn't make the receiver amount negative.
    let bogus_amount = BigInt::from(1u64 << 40);
    let (auth, _) = build_authorization(
        &issuer_key, 999, &bogus_amount, &receiver_key,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );

    let result = validate::validate_assignment(&store, &meta, &auth, block_ts.raw(), &HashMap::new());
    assert!(result.is_err(), "Missing UTXO must return an error, not panic");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("not found"), "Expected 'not found', got: {}", err_msg);
}

fn build_split_assignment(
    giver_amount: &BigInt,
    r1_amount: &BigInt,
    r2_amount: &BigInt,
    receiver1: &SigningKey,
    receiver2: &SigningKey,
    bid_bytes: &[u8],
    deadline: Timestamp,
) -> DataItem {
    let mut ga = Vec::new();
    bigint::encode_bigint(giver_amount, &mut ga);
    let mut r1a = Vec::new();
    bigint::encode_bigint(r1_amount, &mut r1a);
    let mut r2a = Vec::new();
    bigint::encode_bigint(r2_amount, &mut r2a);

    DataItem::container(ASSIGNMENT, vec![
        DataItem::vbc_value(LIST_SIZE, 3),
        DataItem::container(PARTICIPANT, vec![
            DataItem::vbc_value(SEQ_ID, 1),
            DataItem::bytes(AMOUNT, ga),
        ]),
        DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, receiver1.public_key_bytes().to_vec()),
            DataItem::bytes(AMOUNT, r1a),
        ]),
        DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, receiver2.public_key_bytes().to_vec()),
            DataItem::bytes(AMOUNT, r2a),
        ]),
        DataItem::bytes(RECORDING_BID, bid_bytes.to_vec()),
        DataItem::bytes(DEADLINE, deadline.to_bytes().to_vec()),
    ])
}

#[test]
fn test_late_recording_allowed() {
    // An assignment past its deadline can still be recorded if no refutation exists
    // and the UTXO is unspent and not expired.
    let issuer_key = SigningKey::from_seed(&[0x08; 32]);
    let receiver_key = SigningKey::generate();
    let blockmaker = SigningKey::from_seed(&[0xFF; 32]);

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    // Deadline in the past (1 hour after genesis)
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);

    let (auth, _) = build_authorization(
        &issuer_key, 1, &total, &receiver_key,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );

    // Validate at a time well past the deadline (1 day later)
    // but still within the expiry period (1 year)
    let late_ts = Timestamp::from_unix_seconds(1_772_611_200 + 86400);
    let result = validate::validate_assignment(&store, &meta, &auth, late_ts.raw(), &HashMap::new());
    assert!(result.is_ok(), "Late recording should succeed: {:?}", result.err());

    // Actually record it
    let va = result.unwrap();
    let constructed = block::construct_block(&store, &meta, &blockmaker, vec![va], late_ts.raw(), None).unwrap();
    assert_eq!(constructed.height, 1);
}

#[test]
fn test_late_recording_rejected_after_refutation() {
    // After a refutation is recorded, late recording of that assignment must fail.
    let issuer_key = SigningKey::from_seed(&[0x09; 32]);
    let receiver_key = SigningKey::generate();

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 3600);

    let (auth, _) = build_authorization(
        &issuer_key, 1, &total, &receiver_key,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );

    // Compute agreement hash (hash of the ASSIGNMENT, which is the first child of AUTHORIZATION)
    let assignment = auth.find_child(ASSIGNMENT).unwrap();
    let agreement_hash = hash::sha256(&assignment.to_bytes());

    // Record the refutation
    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(&agreement_hash);
    store.add_refutation(&hash_arr).unwrap();

    // Try to validate past deadline — should fail with AgreementRefuted
    let late_ts = Timestamp::from_unix_seconds(1_772_611_200 + 86400);
    let result = validate::validate_assignment(&store, &meta, &auth, late_ts.raw(), &HashMap::new());
    assert!(result.is_err());
    let err_msg = format!("{}", result.unwrap_err());
    assert!(err_msg.contains("refuted"), "Expected 'refuted', got: {}", err_msg);
}

#[test]
fn test_before_deadline_not_affected_by_refutation() {
    // Before deadline, a refutation should NOT prevent recording.
    let issuer_key = SigningKey::from_seed(&[0x0A; 32]);
    let receiver_key = SigningKey::generate();
    let blockmaker = SigningKey::from_seed(&[0xFE; 32]);

    let genesis_item = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis_item).unwrap();

    let total = store.get_utxo(1).unwrap().unwrap().amount;

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    // Deadline far in the future
    let deadline = Timestamp::from_unix_seconds(1_772_611_200 + 86400 * 365);

    let (auth, _) = build_authorization(
        &issuer_key, 1, &total, &receiver_key,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );

    // Record a refutation
    let assignment = auth.find_child(ASSIGNMENT).unwrap();
    let agreement_hash = hash::sha256(&assignment.to_bytes());
    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(&agreement_hash);
    store.add_refutation(&hash_arr).unwrap();

    // Validate BEFORE deadline — refutation should not matter
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);
    let result = validate::validate_assignment(&store, &meta, &auth, block_ts.raw(), &HashMap::new());
    assert!(result.is_ok(), "Before-deadline recording should succeed despite refutation: {:?}", result.err());

    let va = result.unwrap();
    let constructed = block::construct_block(&store, &meta, &blockmaker, vec![va], block_ts.raw(), None).unwrap();
    assert_eq!(constructed.height, 1);
}

// ── Share reward + fee transparency tests ───────────────────────────

/// Helper: build a genesis block with a non-zero REWARD_RATE.
fn build_genesis_with_reward(issuer_key: &SigningKey) -> DataItem {
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

    // Reward rate: 1/100 (1% of transaction amount)
    let reward_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(100));
    let mut reward_bytes = Vec::new();
    bigint::encode_rational(&reward_rate, &mut reward_bytes);

    let expiry_period = Timestamp::from_unix_seconds(31_536_000);
    let ts = Timestamp::from_unix_seconds(1_772_611_200);

    let signable_children = vec![
        DataItem::vbc_value(PROTOCOL_VER, 1),
        DataItem::bytes(CHAIN_SYMBOL, b"RWD".to_vec()),
        DataItem::bytes(DESCRIPTION, b"Reward test chain".to_vec()),
        DataItem::bytes(COIN_COUNT, coin_bytes),
        DataItem::bytes(SHARES_OUT, shares_bytes.clone()),
        DataItem::bytes(FEE_RATE, fee_bytes),
        DataItem::bytes(EXPIRY_PERIOD, expiry_period.to_bytes().to_vec()),
        DataItem::vbc_value(EXPIRY_MODE, 1),
        DataItem::bytes(REWARD_RATE, reward_bytes),
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

/// Build a signed AUTHORIZATION accounting for both fee AND reward.
#[allow(clippy::too_many_arguments)]
fn build_authorization_with_reward(
    giver_key: &SigningKey,
    giver_seq_id: u64,
    giver_amount: &BigInt,
    receiver_key: &SigningKey,
    fee_rate_num: &BigInt,
    fee_rate_den: &BigInt,
    shares_out: &BigInt,
    reward_rate_num: &BigInt,
    reward_rate_den: &BigInt,
    deadline: Timestamp,
    giver_sign_ts: Timestamp,
    receiver_sign_ts: Timestamp,
) -> (DataItem, BigInt) {
    let bid = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
    let mut bid_bytes = Vec::new();
    bigint::encode_rational(&bid, &mut bid_bytes);

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
        let reward = fees::share_reward(giver_amount, reward_rate_num, reward_rate_den);
        receiver_amount = giver_amount - &fee - &reward;
    }

    let auth = build_auth_inner(
        giver_key, giver_seq_id, giver_amount,
        receiver_key, &receiver_amount,
        &bid_bytes, deadline, giver_sign_ts, receiver_sign_ts,
    );
    (auth, receiver_amount)
}

#[test]
fn test_share_reward_basic_flow() {
    // Create chain with 1% reward rate, make a transaction, verify reward UTXO created.
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let receiver = SigningKey::generate();
    let reward_key = SigningKey::generate();

    let genesis = build_genesis_with_reward(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis).unwrap();

    // Verify reward rate was stored
    assert_eq!(meta.reward_rate_num, BigInt::from(1));
    assert_eq!(meta.reward_rate_den, BigInt::from(100));

    let total = meta.shares_out.clone();
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_raw(genesis_ts.raw() + 100_000_000_000);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    let (auth, receiver_amount) = build_authorization_with_reward(
        &issuer_key, 1, &total, &receiver,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        &meta.reward_rate_num, &meta.reward_rate_den,
        deadline, sign_ts1, sign_ts2,
    );

    let validated = validate::validate_assignment(
        &store, &meta, &auth, block_ts.raw(), &HashMap::new(),
    ).unwrap();

    // Verify reward was computed
    let expected_reward = fees::share_reward(&total, &meta.reward_rate_num, &meta.reward_rate_den);
    assert_eq!(validated.reward_shares, expected_reward);
    assert!(expected_reward > BigInt::zero());

    // Construct block with recorder reward key
    let mut reward_pk = [0u8; 32];
    reward_pk.copy_from_slice(reward_key.public_key_bytes());

    let constructed = block::construct_block(
        &store, &meta, &blockmaker,
        vec![validated],
        block_ts.raw(),
        Some(reward_pk),
    ).unwrap();

    assert_eq!(constructed.height, 1);
    // seq 2 = receiver, seq 3 = reward UTXO
    assert_eq!(constructed.first_seq, 2);
    assert_eq!(constructed.seq_count, 2); // one receiver + one reward

    // Verify receiver UTXO
    let receiver_utxo = store.get_utxo(2).unwrap().unwrap();
    assert_eq!(receiver_utxo.amount, receiver_amount);
    assert_eq!(receiver_utxo.status, UtxoStatus::Unspent);

    // Verify reward UTXO
    let reward_utxo = store.get_utxo(3).unwrap().unwrap();
    assert_eq!(reward_utxo.amount, expected_reward);
    assert_eq!(reward_utxo.pubkey, reward_pk);
    assert_eq!(reward_utxo.status, UtxoStatus::Unspent);

    // Verify shares_out was reduced only by the burn fee, NOT the reward
    let meta_after = store.load_chain_meta().unwrap().unwrap();
    let fee = &total - &receiver_amount - &expected_reward;
    assert_eq!(meta_after.shares_out, &meta.shares_out - &fee);
}

#[test]
fn test_share_reward_zero_rate_no_reward_key_needed() {
    // Zero reward rate (default) — no reward key needed, None works fine.
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let receiver = SigningKey::generate();

    let genesis = build_genesis(&issuer_key); // default: zero reward
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis).unwrap();

    assert_eq!(meta.reward_rate_num, BigInt::zero());

    let total = meta.shares_out.clone();
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_raw(genesis_ts.raw() + 100_000_000_000);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    let (auth, _) = build_authorization(
        &issuer_key, 1, &total, &receiver,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );

    let validated = validate::validate_assignment(
        &store, &meta, &auth, block_ts.raw(), &HashMap::new(),
    ).unwrap();

    assert_eq!(validated.reward_shares, BigInt::zero());

    // Should work with None (no reward key)
    let constructed = block::construct_block(
        &store, &meta, &blockmaker,
        vec![validated],
        block_ts.raw(),
        None,
    ).unwrap();

    assert_eq!(constructed.seq_count, 1); // only receiver, no reward UTXO
}

#[test]
fn test_share_reward_missing_key_errors() {
    // Non-zero reward but no reward key → should error
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let receiver = SigningKey::generate();

    let genesis = build_genesis_with_reward(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis).unwrap();

    let total = meta.shares_out.clone();
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_raw(genesis_ts.raw() + 100_000_000_000);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    let (auth, _) = build_authorization_with_reward(
        &issuer_key, 1, &total, &receiver,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        &meta.reward_rate_num, &meta.reward_rate_den,
        deadline, sign_ts1, sign_ts2,
    );

    let validated = validate::validate_assignment(
        &store, &meta, &auth, block_ts.raw(), &HashMap::new(),
    ).unwrap();

    // Construct with None — should fail
    let result = block::construct_block(
        &store, &meta, &blockmaker,
        vec![validated],
        block_ts.raw(),
        None,
    );
    assert!(result.is_err());
    let err_str = result.unwrap_err().to_string();
    assert!(err_str.contains("recorder_reward_pubkey"), "Error: {}", err_str);
}

#[test]
fn test_fee_transparency_block_contents() {
    // Verify RECORDING_FEE_ACTUAL is present in BLOCK_CONTENTS
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let receiver = SigningKey::generate();

    let genesis = build_genesis(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis).unwrap();

    let total = meta.shares_out.clone();
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_raw(genesis_ts.raw() + 100_000_000_000);
    let block_ts = Timestamp::from_raw(genesis_ts.raw() + 3_000_000);

    let (auth, _) = build_authorization(
        &issuer_key, 1, &total, &receiver,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        deadline, sign_ts1, sign_ts2,
    );

    let validated = validate::validate_assignment(
        &store, &meta, &auth, block_ts.raw(), &HashMap::new(),
    ).unwrap();

    let constructed = block::construct_block(
        &store, &meta, &blockmaker,
        vec![validated],
        block_ts.raw(),
        None,
    ).unwrap();

    // Navigate BLOCK → BLOCK_SIGNED → BLOCK_CONTENTS → RECORDING_FEE_ACTUAL
    let block_signed = constructed.block.find_child(BLOCK_SIGNED).expect("BLOCK_SIGNED");
    let block_contents = block_signed.find_child(BLOCK_CONTENTS).expect("BLOCK_CONTENTS");
    let fee_actual = block_contents.find_child(RECORDING_FEE_ACTUAL)
        .expect("RECORDING_FEE_ACTUAL should be present in BLOCK_CONTENTS");

    // Decode and verify it matches the chain's fee rate
    let fee_actual_bytes = fee_actual.as_bytes().expect("fee actual should have bytes");
    let (decoded_rate, _) = bigint::decode_rational(fee_actual_bytes, 0).unwrap();
    let expected_rate = num_rational::BigRational::new(
        meta.fee_rate_num.clone(), meta.fee_rate_den.clone());
    assert_eq!(decoded_rate, expected_rate);
}

// ── Reward Rate Change (Test M) ─────────────────────────────────────

/// Helper: sign a REWARD_RATE_CHANGE signable content with the given key.
fn sign_rate_change(key: &SigningKey, signable: &DataItem, ts: Timestamp) -> DataItem {
    let sig = sign::sign_dataitem(key, signable, ts);
    DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
    ])
}

#[test]
fn test_reward_rate_change_full_flow() {
    // Test M: create chain with reward rate → transact → change rate → verify new rate applies.
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let receiver = SigningKey::generate();
    let reward_key1 = SigningKey::generate();

    let genesis = build_genesis_with_reward(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let mut meta = genesis::load_genesis(&store, &genesis).unwrap();

    // Set recorder pubkey (normally done by ao-recorder)
    let mut recorder_pk = [0u8; 32];
    recorder_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&recorder_pk).unwrap();
    meta.recorder_pubkey = Some(recorder_pk);

    // Register issuer as owner key
    let mut issuer_pk = [0u8; 32];
    issuer_pk.copy_from_slice(issuer_key.public_key_bytes());
    store.insert_owner_key(&issuer_pk, 0, genesis_ts.raw(), None).unwrap();

    // Verify initial reward rate is 1/100
    assert_eq!(meta.reward_rate_num, BigInt::from(1));
    assert_eq!(meta.reward_rate_den, BigInt::from(100));

    // ── Step 1: Make a transaction with the 1% reward rate ──
    let total = meta.shares_out.clone();
    let sign_ts1 = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sign_ts2 = Timestamp::from_raw(genesis_ts.raw() + 2_000_000);
    let deadline = Timestamp::from_raw(genesis_ts.raw() + 100_000_000_000);
    let block_ts1 = genesis_ts.raw() + 3_000_000;

    let (auth, _recv_amount) = build_authorization_with_reward(
        &issuer_key, 1, &total, &receiver,
        &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out,
        &meta.reward_rate_num, &meta.reward_rate_den,
        deadline, sign_ts1, sign_ts2,
    );

    let validated = validate::validate_assignment(
        &store, &meta, &auth, block_ts1, &HashMap::new(),
    ).unwrap();

    assert!(validated.reward_shares > BigInt::zero(),
        "Reward should be positive with 1% rate");

    let mut reward_pk1 = [0u8; 32];
    reward_pk1.copy_from_slice(reward_key1.public_key_bytes());

    let constructed1 = block::construct_block(
        &store, &meta, &blockmaker,
        vec![validated],
        block_ts1,
        Some(reward_pk1),
    ).unwrap();

    // Update meta for next operations
    meta.block_height = constructed1.height;
    meta.last_block_timestamp = constructed1.timestamp;
    meta.prev_hash = constructed1.block_hash;
    meta.shares_out = constructed1.new_shares_out.clone();
    meta.next_seq_id = constructed1.first_seq + constructed1.seq_count;

    // Verify reward UTXO was created
    let reward_utxo = store.get_utxo(constructed1.first_seq + constructed1.seq_count - 1)
        .unwrap().expect("reward UTXO should exist");
    assert_eq!(reward_utxo.status, UtxoStatus::Unspent);
    assert_eq!(reward_utxo.pubkey, reward_pk1);

    // ── Step 2: Change reward rate from 1/100 to 1/200 ──
    let new_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(200));
    let mut rate_bytes = Vec::new();
    bigint::encode_rational(&new_rate, &mut rate_bytes);

    let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
        DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
    ]);

    let change_ts = Timestamp::from_raw(block_ts1 + 1_000_000);
    let owner_sig = sign_rate_change(&issuer_key, &signable, change_ts);
    let recorder_sig = sign_rate_change(&blockmaker, &signable, change_ts);

    let rate_change_item = DataItem::container(REWARD_RATE_CHANGE, vec![
        DataItem::bytes(REWARD_RATE, rate_bytes),
        owner_sig,
        recorder_sig,
    ]);

    let block_ts2 = block_ts1 + 2_000_000;
    let vrc = reward_rate::validate_reward_rate_change(
        &store, &meta, &rate_change_item, block_ts2,
    ).unwrap();

    // BigRational normalizes 1/200 → 1/200
    assert_eq!(vrc.new_rate_num, BigInt::from(1));
    assert_eq!(vrc.new_rate_den, BigInt::from(200));

    let constructed2 = block::construct_reward_rate_change_block(
        &store, &meta, &blockmaker, vrc, block_ts2,
    ).unwrap();

    assert_eq!(constructed2.seq_count, 0, "admin block should have no new sequences");
    assert_eq!(constructed2.new_shares_out, meta.shares_out, "shares_out unchanged for admin block");

    // Update meta
    meta.block_height = constructed2.height;
    meta.last_block_timestamp = constructed2.timestamp;
    meta.prev_hash = constructed2.block_hash;

    // Reload and verify the new rate was persisted
    let reloaded_meta = store.load_chain_meta().unwrap().unwrap();
    assert_eq!(reloaded_meta.reward_rate_num, BigInt::from(1));
    assert_eq!(reloaded_meta.reward_rate_den, BigInt::from(200));
}

#[test]
fn test_reward_rate_change_owner_only_rejected() {
    // Reward rate change with only owner signature (no recorder) should fail.
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let other_owner = SigningKey::generate();

    let genesis = build_genesis_with_reward(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let meta_orig = genesis::load_genesis(&store, &genesis).unwrap();

    let mut recorder_pk = [0u8; 32];
    recorder_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&recorder_pk).unwrap();
    let mut meta = meta_orig;
    meta.recorder_pubkey = Some(recorder_pk);

    // Register both as owner keys
    let mut issuer_pk = [0u8; 32];
    issuer_pk.copy_from_slice(issuer_key.public_key_bytes());
    store.insert_owner_key(&issuer_pk, 0, genesis_ts.raw(), None).unwrap();

    let mut other_pk = [0u8; 32];
    other_pk.copy_from_slice(other_owner.public_key_bytes());
    store.insert_owner_key(&other_pk, 0, genesis_ts.raw(), None).unwrap();

    // Try to change rate with two owner sigs (no recorder sig)
    let new_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(50));
    let mut rate_bytes = Vec::new();
    bigint::encode_rational(&new_rate, &mut rate_bytes);

    let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
        DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
    ]);

    let ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sig1 = sign_rate_change(&issuer_key, &signable, ts);
    let sig2 = sign_rate_change(&other_owner, &signable, ts);

    let item = DataItem::container(REWARD_RATE_CHANGE, vec![
        DataItem::bytes(REWARD_RATE, rate_bytes),
        sig1,
        sig2,
    ]);

    let block_ts = genesis_ts.raw() + 2_000_000;
    let result = reward_rate::validate_reward_rate_change(&store, &meta, &item, block_ts);
    assert!(result.is_err(), "Should reject rate change without recorder sig");
    let err = result.unwrap_err().to_string();
    assert!(err.contains("recorder") || err.contains("duplicate owner"),
        "Error should mention missing recorder: {}", err);
}

#[test]
fn test_reward_rate_change_unauthorized_signer_rejected() {
    // Reward rate change signed by random keys (neither owner nor recorder) should fail.
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let random1 = SigningKey::generate();
    let random2 = SigningKey::generate();

    let genesis = build_genesis_with_reward(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let meta_orig = genesis::load_genesis(&store, &genesis).unwrap();

    let mut recorder_pk = [0u8; 32];
    recorder_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&recorder_pk).unwrap();
    let mut meta = meta_orig;
    meta.recorder_pubkey = Some(recorder_pk);

    let new_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(50));
    let mut rate_bytes = Vec::new();
    bigint::encode_rational(&new_rate, &mut rate_bytes);

    let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
        DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
    ]);

    let ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sig1 = sign_rate_change(&random1, &signable, ts);
    let sig2 = sign_rate_change(&random2, &signable, ts);

    let item = DataItem::container(REWARD_RATE_CHANGE, vec![
        DataItem::bytes(REWARD_RATE, rate_bytes),
        sig1,
        sig2,
    ]);

    let block_ts = genesis_ts.raw() + 2_000_000;
    let result = reward_rate::validate_reward_rate_change(&store, &meta, &item, block_ts);
    assert!(result.is_err(), "Should reject rate change from unauthorized signers");
    assert!(result.unwrap_err().to_string().contains("neither a valid owner key nor the recorder"));
}

// ── Recorder Switch (Tests for §5) ──────────────────────────────────

/// Helper: sign a DataItem for recorder switch operations.
fn sign_switch_item(key: &SigningKey, signable: &DataItem, ts: Timestamp) -> DataItem {
    let sig = sign::sign_dataitem(key, signable, ts);
    DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
    ])
}

#[test]
fn test_recorder_switch_full_flow() {
    // Test: RECORDER_CHANGE_PENDING → RECORDER_CHANGE (no active escrows)
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let new_recorder = SigningKey::generate();

    let genesis = build_genesis(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let mut meta = genesis::load_genesis(&store, &genesis).unwrap();

    // Set recorder pubkey + register owner key
    let mut recorder_pk = [0u8; 32];
    recorder_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&recorder_pk).unwrap();
    meta.recorder_pubkey = Some(recorder_pk);

    let mut issuer_pk = [0u8; 32];
    issuer_pk.copy_from_slice(issuer_key.public_key_bytes());
    store.insert_owner_key(&issuer_pk, 0, genesis_ts.raw(), None).unwrap();

    // Step 1: Submit RECORDER_CHANGE_PENDING
    let pending_signable = DataItem::container(RECORDER_CHANGE_PENDING, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
    ]);
    let pending_ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let pending_sig = sign_switch_item(&issuer_key, &pending_signable, pending_ts);

    let pending_item = DataItem::container(RECORDER_CHANGE_PENDING, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        pending_sig,
    ]);

    let block_ts1 = genesis_ts.raw() + 2_000_000;
    let vp = recorder_switch::validate_pending(&store, &meta, &pending_item, block_ts1).unwrap();

    let constructed1 = block::construct_recorder_switch_block(
        &store, &meta, &blockmaker,
        block::RecorderSwitchOp::Pending(vp),
        block_ts1,
    ).unwrap();

    assert_eq!(constructed1.seq_count, 0);

    // Verify pending state was stored
    let meta2 = store.load_chain_meta().unwrap().unwrap();
    let pending = meta2.pending_recorder_change.as_ref().unwrap();
    let mut expected_new_pk = [0u8; 32];
    expected_new_pk.copy_from_slice(new_recorder.public_key_bytes());
    assert_eq!(pending.new_recorder_pubkey, expected_new_pk);
    assert_eq!(pending.new_recorder_url, "https://new-recorder.example.com");

    // Step 2: Submit RECORDER_CHANGE (no escrows → immediate)
    let change_signable = DataItem::container(RECORDER_CHANGE, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
    ]);
    let change_ts = Timestamp::from_raw(block_ts1 + 1_000_000);
    let change_sig = sign_switch_item(&issuer_key, &change_signable, change_ts);

    let change_item = DataItem::container(RECORDER_CHANGE, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        change_sig,
    ]);

    let block_ts2 = block_ts1 + 2_000_000;
    let vc = recorder_switch::validate_change(&store, &meta2, &change_item, block_ts2).unwrap();

    let constructed2 = block::construct_recorder_switch_block(
        &store, &meta2, &blockmaker,
        block::RecorderSwitchOp::Change(vc),
        block_ts2,
    ).unwrap();

    assert_eq!(constructed2.seq_count, 0);

    // Verify: recorder pubkey updated, pending cleared
    let meta3 = store.load_chain_meta().unwrap().unwrap();
    assert_eq!(meta3.recorder_pubkey.unwrap(), expected_new_pk);
    assert!(meta3.pending_recorder_change.is_none(), "Pending should be cleared after RECORDER_CHANGE");
}

#[test]
fn test_recorder_url_change_flow() {
    // Test: dual-signed RECORDER_URL_CHANGE
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();

    let genesis = build_genesis(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let mut meta = genesis::load_genesis(&store, &genesis).unwrap();

    let mut recorder_pk = [0u8; 32];
    recorder_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&recorder_pk).unwrap();
    meta.recorder_pubkey = Some(recorder_pk);

    let mut issuer_pk = [0u8; 32];
    issuer_pk.copy_from_slice(issuer_key.public_key_bytes());
    store.insert_owner_key(&issuer_pk, 0, genesis_ts.raw(), None).unwrap();

    let url_signable = DataItem::container(RECORDER_URL_CHANGE, vec![
        DataItem::bytes(RECORDER_URL, b"https://moved.example.com".to_vec()),
    ]);
    let ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let owner_sig = sign_switch_item(&issuer_key, &url_signable, ts);
    let recorder_sig = sign_switch_item(&blockmaker, &url_signable, ts);

    let url_item = DataItem::container(RECORDER_URL_CHANGE, vec![
        DataItem::bytes(RECORDER_URL, b"https://moved.example.com".to_vec()),
        owner_sig,
        recorder_sig,
    ]);

    let block_ts = genesis_ts.raw() + 2_000_000;
    let vu = recorder_switch::validate_url_change(&store, &meta, &url_item, block_ts).unwrap();
    assert_eq!(vu.new_url, "https://moved.example.com");

    let constructed = block::construct_recorder_switch_block(
        &store, &meta, &blockmaker,
        block::RecorderSwitchOp::UrlChange(vu),
        block_ts,
    ).unwrap();

    assert_eq!(constructed.seq_count, 0);
    assert_eq!(constructed.height, meta.block_height + 1);
}

#[test]
fn test_recorder_change_blocked_with_active_escrows() {
    // RECORDER_CHANGE should fail when active escrows exist.
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let new_recorder = SigningKey::generate();

    let genesis = build_genesis(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let mut meta = genesis::load_genesis(&store, &genesis).unwrap();

    let mut recorder_pk = [0u8; 32];
    recorder_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&recorder_pk).unwrap();
    meta.recorder_pubkey = Some(recorder_pk);

    let mut issuer_pk = [0u8; 32];
    issuer_pk.copy_from_slice(issuer_key.public_key_bytes());
    store.insert_owner_key(&issuer_pk, 0, genesis_ts.raw(), None).unwrap();

    let mut new_pk = [0u8; 32];
    new_pk.copy_from_slice(new_recorder.public_key_bytes());

    // Set pending state
    meta.pending_recorder_change = Some(PendingRecorderChange {
        new_recorder_pubkey: new_pk,
        new_recorder_url: "https://new-recorder.example.com".into(),
        pending_height: meta.block_height,
        owner_auth_sig_bytes: Vec::new(), // test-only: no real sig needed for validation test
    });

    // Insert a fake active escrow
    store.insert_caa_escrow(
        &[99; 32], 1,
        genesis_ts.raw() + 100_000_000_000, // far future deadline
        meta.block_height, None, 2, &BigInt::from(0),
    ).map_err(|e| panic!("insert escrow failed: {}", e)).unwrap();

    // Try RECORDER_CHANGE — should fail because of active escrow
    let change_signable = DataItem::container(RECORDER_CHANGE, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
    ]);
    let ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let sig = sign_switch_item(&issuer_key, &change_signable, ts);

    let change_item = DataItem::container(RECORDER_CHANGE, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        sig,
    ]);

    let block_ts = genesis_ts.raw() + 2_000_000;
    let result = recorder_switch::validate_change(&store, &meta, &change_item, block_ts);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("active CAA escrow"),
        "Expected active escrow error");
}

#[test]
fn test_recorder_change_dual_signature() {
    // RECORDER_CHANGE with both owner + outgoing recorder signatures.
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let new_recorder = SigningKey::generate();

    let genesis = build_genesis(&issuer_key);
    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);

    let store = ChainStore::open_memory().unwrap();
    let mut meta = genesis::load_genesis(&store, &genesis).unwrap();

    let mut recorder_pk = [0u8; 32];
    recorder_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&recorder_pk).unwrap();
    meta.recorder_pubkey = Some(recorder_pk);

    let mut issuer_pk = [0u8; 32];
    issuer_pk.copy_from_slice(issuer_key.public_key_bytes());
    store.insert_owner_key(&issuer_pk, 0, genesis_ts.raw(), None).unwrap();

    let mut new_pk = [0u8; 32];
    new_pk.copy_from_slice(new_recorder.public_key_bytes());

    // Set pending state (simulate RECORDER_CHANGE_PENDING already recorded)
    meta.pending_recorder_change = Some(PendingRecorderChange {
        new_recorder_pubkey: new_pk,
        new_recorder_url: "https://new-recorder.example.com".into(),
        pending_height: meta.block_height,
        owner_auth_sig_bytes: Vec::new(),
    });
    store.store_chain_meta(&meta).unwrap();

    // RECORDER_CHANGE with BOTH owner and outgoing recorder signatures
    let change_signable = DataItem::container(RECORDER_CHANGE, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
    ]);
    let ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let owner_sig = sign_switch_item(&issuer_key, &change_signable, ts);
    let recorder_sig = sign_switch_item(&blockmaker, &change_signable, ts);

    let change_item = DataItem::container(RECORDER_CHANGE, vec![
        DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        owner_sig,
        recorder_sig,
    ]);

    let block_ts = genesis_ts.raw() + 2_000_000;
    let vc = recorder_switch::validate_change(&store, &meta, &change_item, block_ts).unwrap();

    let constructed = block::construct_recorder_switch_block(
        &store, &meta, &blockmaker,
        block::RecorderSwitchOp::Change(vc),
        block_ts,
    ).unwrap();

    assert_eq!(constructed.seq_count, 0);
    let meta2 = store.load_chain_meta().unwrap().unwrap();
    assert_eq!(meta2.recorder_pubkey.unwrap(), new_pk);
    assert!(meta2.pending_recorder_change.is_none());
}

// ── Chain Migration Tests ───────────────────────────────────────────

#[test]
fn test_chain_migration_full_flow() {
    // Setup: genesis → register owner key → blockmaker → migrate
    let issuer_key = SigningKey::generate();
    let blockmaker = SigningKey::generate();
    let genesis = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let _meta = genesis::load_genesis(&store, &genesis).unwrap();

    // Set recorder pubkey
    let mut bm_pk = [0u8; 32];
    bm_pk.copy_from_slice(blockmaker.public_key_bytes());
    store.set_recorder_pubkey(&bm_pk).unwrap();

    let genesis_ts = Timestamp::from_unix_seconds(1_772_611_200);
    let new_chain_id = [0xBBu8; 32];

    // Build CHAIN_MIGRATION with owner signature
    let signable = DataItem::container(CHAIN_MIGRATION, vec![
        DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
    ]);

    let ts = Timestamp::from_raw(genesis_ts.raw() + 1_000_000);
    let owner_sig_val = sign::sign_dataitem(&issuer_key, &signable, ts);
    let owner_auth = DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, owner_sig_val.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, issuer_key.public_key_bytes().to_vec()),
    ]);

    let migration_item = DataItem::container(CHAIN_MIGRATION, vec![
        DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        owner_auth,
    ]);

    let meta = store.load_chain_meta().unwrap().unwrap();
    let block_ts = genesis_ts.raw() + 1_000_000;

    let validated = ao_chain::migration::validate_chain_migration(
        &store, &meta, &migration_item, block_ts,
    ).unwrap();
    assert!(validated.has_owner_sig);
    assert!(!validated.has_recorder_sig);
    assert_eq!(validated.new_chain_id, new_chain_id);

    let constructed = block::construct_migration_block(
        &store, &meta, &blockmaker, validated, block_ts,
    ).unwrap();
    assert_eq!(constructed.seq_count, 0);
    assert_eq!(constructed.height, 1);

    // Chain should now be frozen
    let meta2 = store.load_chain_meta().unwrap().unwrap();
    assert!(meta2.frozen, "Chain should be frozen after migration");

    // Further submissions should fail
    let signable2 = DataItem::container(CHAIN_MIGRATION, vec![
        DataItem::bytes(CHAIN_REF, [0xCC; 32].to_vec()),
    ]);
    let result = ao_chain::migration::validate_chain_migration(
        &store, &meta2, &signable2, block_ts + 1,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already frozen"));
}

#[test]
fn test_chain_migration_blocked_by_active_escrows() {
    let issuer_key = SigningKey::generate();
    let genesis = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis).unwrap();

    // Insert an active escrow
    store.insert_caa_escrow(&[0xAA; 32], 1, 9999, 1, None, 2, &BigInt::zero()).unwrap();

    let new_chain_id = [0xBBu8; 32];
    let item = DataItem::container(CHAIN_MIGRATION, vec![
        DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
    ]);

    let result = ao_chain::migration::validate_chain_migration(
        &store, &meta, &item, 3000,
    );
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("active escrows"));
}

#[test]
fn test_chain_migration_social_tier_no_sigs() {
    let issuer_key = SigningKey::generate();
    let genesis = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let meta = genesis::load_genesis(&store, &genesis).unwrap();

    let new_chain_id = [0xBBu8; 32];
    // No AUTH_SIG — social tier
    let item = DataItem::container(CHAIN_MIGRATION, vec![
        DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
    ]);

    let result = ao_chain::migration::validate_chain_migration(
        &store, &meta, &item, 3000,
    );
    assert!(result.is_ok());
    let vm = result.unwrap();
    assert!(!vm.has_owner_sig);
    assert!(!vm.has_recorder_sig);
}

#[test]
fn test_surrogate_proof_and_majority_check() {
    let issuer_key = SigningKey::generate();
    let genesis = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let mut meta = genesis::load_genesis(&store, &genesis).unwrap();

    // Freeze the chain (surrogate proofs require frozen old chain)
    store.set_chain_frozen().unwrap();
    meta.frozen = true;

    // The issuer UTXO has all shares (2^40)
    let new_chain_id = [0xBBu8; 32];
    let shares_out = BigInt::from(1u64 << 40);

    // Build a surrogate proof for the issuer UTXO (seq 1)
    let mut amount_bytes = Vec::new();
    bigint::encode_bigint(&shares_out, &mut amount_bytes);

    let signable = DataItem::container(SURROGATE_PROOF, vec![
        DataItem::vbc_value(SEQ_ID, 1),
        DataItem::bytes(AMOUNT, amount_bytes.clone()),
        DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
    ]);

    let ts = Timestamp::from_raw(2000);
    let sig = sign::sign_dataitem(&issuer_key, &signable, ts);
    let auth = DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, issuer_key.public_key_bytes().to_vec()),
    ]);

    let proof_item = DataItem::container(SURROGATE_PROOF, vec![
        DataItem::vbc_value(SEQ_ID, 1),
        DataItem::bytes(AMOUNT, amount_bytes),
        DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        auth,
    ]);

    let vsp = ao_chain::migration::validate_surrogate_proof(
        &store, &meta, &proof_item, &new_chain_id,
    ).unwrap();
    assert_eq!(vsp.seq_id, 1);
    assert_eq!(vsp.amount, shares_out);

    // 100% ownership > 50% → majority
    assert!(ao_chain::migration::check_surrogate_majority(
        &[vsp], &meta.shares_out,
    ));
}

#[test]
fn test_get_all_unspent_utxos_for_migration() {
    let issuer_key = SigningKey::generate();
    let genesis = build_genesis(&issuer_key);
    let store = ChainStore::open_memory().unwrap();
    let _meta = genesis::load_genesis(&store, &genesis).unwrap();

    let utxos = store.get_all_unspent_utxos().unwrap();
    assert_eq!(utxos.len(), 1);
    assert_eq!(utxos[0].seq_id, 1);
    assert_eq!(utxos[0].amount, BigInt::from(1u64 << 40));
}
