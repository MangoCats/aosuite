use num_bigint::BigInt;
use num_traits::Zero;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_crypto::hash;
use ao_crypto::sign::{self, SigningKey};

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta, Utxo, UtxoStatus};
use crate::validate::ValidatedAssignment;
use crate::expiry;

/// Constructed block result.
pub struct ConstructedBlock {
    pub block: DataItem,
    pub block_bytes: Vec<u8>,
    pub block_hash: [u8; 32],
    pub height: u64,
    pub timestamp: i64,
    pub new_shares_out: BigInt,
    pub first_seq: u64,
    pub seq_count: u64,
}

/// Build and record a block containing the given validated assignments.
///
/// This is the main block construction entry point. It:
/// 1. Runs expiration sweep
/// 2. Builds BLOCK_CONTENTS with pages
/// 3. Signs with blockmaker key
/// 4. Computes block hash
/// 5. Updates chain state in the store
pub fn construct_block(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    assignments: Vec<ValidatedAssignment>,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    if assignments.is_empty() {
        return Err(ChainError::InvalidBlock("no assignments to record".into()));
    }

    // Block timestamp must exceed previous block
    if block_timestamp <= meta.last_block_timestamp {
        return Err(ChainError::TimestampOrder(
            format!("block timestamp {} <= previous {}",
                block_timestamp, meta.last_block_timestamp)));
    }

    store.begin_transaction()?;
    let result = construct_block_inner(store, meta, blockmaker_key, assignments, block_timestamp);
    match &result {
        Ok(_) => store.commit()?,
        Err(_) => store.rollback()?,
    }
    result
}

fn construct_block_inner(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    assignments: Vec<ValidatedAssignment>,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    let height = meta.block_height + 1;
    let mut shares_out = meta.shares_out.clone();
    let mut next_seq = meta.next_seq_id;
    let first_seq = next_seq;

    // Run expiration sweep (mode 1 only for now)
    let expired_shares = expiry::run_expiry_sweep(store, meta, block_timestamp)?;
    shares_out -= &expired_shares;

    // Total fees across all assignments
    let mut total_fees = BigInt::zero();
    let mut pages = Vec::new();

    for (page_idx, va) in assignments.iter().enumerate() {
        // Mark giver UTXOs as spent
        for (seq_id, _) in &va.givers {
            store.mark_spent(*seq_id)?;
        }

        // Create receiver UTXOs
        for (pk, amount) in &va.receivers {
            store.insert_utxo(&Utxo {
                seq_id: next_seq,
                pubkey: *pk,
                amount: amount.clone(),
                block_height: height,
                block_timestamp,
                status: UtxoStatus::Unspent,
            })?;
            store.mark_key_used(pk)?;
            next_seq += 1;
        }

        total_fees += &va.fee_shares;

        // Build PAGE
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, page_idx as u64),
            va.authorization.clone(),
        ]);
        pages.push(page);
    }

    shares_out -= &total_fees;
    let seq_count = next_seq - first_seq;

    // Encode shares_out for the block
    let mut shares_bytes = Vec::new();
    bigint::encode_bigint(&shares_out, &mut shares_bytes);

    // Build BLOCK_CONTENTS
    let mut block_contents_children = vec![
        DataItem::bytes(PREV_HASH, meta.prev_hash.to_vec()),
        DataItem::vbc_value(FIRST_SEQ, first_seq),
        DataItem::vbc_value(SEQ_COUNT, seq_count),
        DataItem::vbc_value(LIST_SIZE, pages.len() as u64),
        DataItem::bytes(SHARES_OUT, shares_bytes),
    ];
    block_contents_children.extend(pages);

    let block_contents = DataItem::container(BLOCK_CONTENTS, block_contents_children);

    // Sign block contents
    let ts = Timestamp::from_raw(block_timestamp);
    let sig = sign::sign_dataitem(blockmaker_key, &block_contents, ts);

    let block_signed = DataItem::container(BLOCK_SIGNED, vec![
        block_contents,
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::bytes(ED25519_PUB, blockmaker_key.public_key_bytes().to_vec()),
        ]),
    ]);

    // Compute block hash = SHA256 of BLOCK_SIGNED encoding
    let block_signed_bytes = block_signed.to_bytes();
    let block_hash = hash::sha256(&block_signed_bytes);

    let block = DataItem::container(BLOCK, vec![
        block_signed,
        DataItem::bytes(SHA256, block_hash.to_vec()),
    ]);

    let block_bytes = block.to_bytes();

    // Update chain state
    store.store_block(height, block_timestamp, &block_hash, &block_bytes)?;
    store.advance_block(height, block_timestamp, &block_hash)?;
    store.update_shares_out(&shares_out)?;
    store.set_next_seq_id(next_seq)?;

    Ok(ConstructedBlock {
        block,
        block_bytes,
        block_hash,
        height,
        timestamp: block_timestamp,
        new_shares_out: shares_out,
        first_seq,
        seq_count,
    })
}
