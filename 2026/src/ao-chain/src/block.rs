use num_bigint::BigInt;
use num_traits::Zero;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_crypto::hash;
use ao_crypto::sign::{self, SigningKey};
use num_rational;

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta, Utxo, UtxoStatus};
use crate::validate::ValidatedAssignment;
use crate::owner_keys::{ValidatedRotation, ValidatedRevocation, ValidatedOverride};
use crate::reward_rate::ValidatedRateChange;
use crate::recorder_switch::{ValidatedPending, ValidatedChange, ValidatedUrlChange};
use crate::migration::ValidatedMigration;
use crate::expiry;

/// Constructed block result.
#[derive(Debug)]
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
/// 3. Creates recorder reward UTXO if reward > 0
/// 4. Adds RECORDING_FEE_ACTUAL for fee transparency
/// 5. Signs with blockmaker key
/// 6. Computes block hash
/// 7. Updates chain state in the store
///
/// `recorder_reward_pubkey`: fresh one-time-use pubkey for the recorder's share reward.
/// Required when the chain's reward rate is non-zero and total reward > 0.
pub fn construct_block(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    assignments: Vec<ValidatedAssignment>,
    block_timestamp: i64,
    recorder_reward_pubkey: Option<[u8; 32]>,
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
    let result = construct_block_inner(
        store, meta, blockmaker_key, assignments, block_timestamp, recorder_reward_pubkey);
    match &result {
        Ok(_) => store.commit()?,
        Err(_) => {
            // Rollback best-effort: if rollback fails, the original error is more
            // informative than the rollback error, so we keep it.
            let _ = store.rollback();
        }
    }
    result
}

fn construct_block_inner(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    assignments: Vec<ValidatedAssignment>,
    block_timestamp: i64,
    recorder_reward_pubkey: Option<[u8; 32]>,
) -> Result<ConstructedBlock> {
    let height = meta.block_height + 1;
    let mut shares_out = meta.shares_out.clone();
    let mut next_seq = meta.next_seq_id;
    let first_seq = next_seq;

    // Run expiration sweep (mode 1 only for now)
    let expired_shares = expiry::run_expiry_sweep(store, meta, block_timestamp)?;
    shares_out -= &expired_shares;

    // Run escrow sweep — release expired CAA escrows back to unspent.
    // Non-fatal: sweep failure should not prevent block production.
    if let Ok((_, fee_restore)) = crate::caa::run_escrow_sweep(store, block_timestamp) {
        shares_out += fee_restore;
    }

    // Totals across all assignments
    let mut total_fees = BigInt::zero();
    let mut total_reward = BigInt::zero();
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
        total_reward += &va.reward_shares;

        // Build PAGE
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, page_idx as u64),
            va.authorization.clone(),
        ]);
        pages.push(page);
    }

    // Create recorder reward UTXO if reward > 0
    if total_reward > BigInt::zero() {
        let reward_pk = recorder_reward_pubkey.ok_or_else(|| {
            ChainError::InvalidBlock("reward > 0 but no recorder_reward_pubkey provided".into())
        })?;
        if store.is_key_used(&reward_pk)? {
            return Err(ChainError::KeyReuse);
        }
        store.insert_utxo(&Utxo {
            seq_id: next_seq,
            pubkey: reward_pk,
            amount: total_reward.clone(),
            block_height: height,
            block_timestamp,
            status: UtxoStatus::Unspent,
        })?;
        store.mark_key_used(&reward_pk)?;
        next_seq += 1;
    }

    // Only the burn fee reduces shares_out; reward is a transfer, not a burn.
    shares_out -= &total_fees;
    let seq_count = next_seq - first_seq;

    // Encode shares_out for the block
    let mut shares_bytes = Vec::new();
    bigint::encode_bigint(&shares_out, &mut shares_bytes);

    // Encode RECORDING_FEE_ACTUAL as rational (current chain fee rate)
    let fee_actual_rational = num_rational::BigRational::new(
        meta.fee_rate_num.clone(), meta.fee_rate_den.clone());
    let mut fee_actual_bytes = Vec::new();
    bigint::encode_rational(&fee_actual_rational, &mut fee_actual_bytes);

    // Build BLOCK_CONTENTS
    let mut block_contents_children = vec![
        DataItem::bytes(PREV_HASH, meta.prev_hash.to_vec()),
        DataItem::vbc_value(FIRST_SEQ, first_seq),
        DataItem::vbc_value(SEQ_COUNT, seq_count),
        DataItem::vbc_value(LIST_SIZE, pages.len() as u64),
        DataItem::bytes(SHARES_OUT, shares_bytes),
        DataItem::bytes(RECORDING_FEE_ACTUAL, fee_actual_bytes),
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

/// Possible owner key operations for `construct_owner_key_block`.
pub enum OwnerKeyOp {
    Rotation(ValidatedRotation),
    Revocation(ValidatedRevocation),
    Override(ValidatedOverride),
}

/// Construct and record a block containing an owner key operation.
///
/// Owner key blocks are administrative: no UTXO mutations, no fee deduction.
/// The block wraps the operation DataItem in BLOCK_CONTENTS → BLOCK_SIGNED → BLOCK.
pub fn construct_owner_key_block(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    op: OwnerKeyOp,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    if block_timestamp <= meta.last_block_timestamp {
        return Err(ChainError::TimestampOrder(
            format!("block timestamp {} <= previous {}",
                block_timestamp, meta.last_block_timestamp)));
    }

    store.begin_transaction()?;
    let result = construct_owner_key_block_inner(store, meta, blockmaker_key, op, block_timestamp);
    match &result {
        Ok(_) => store.commit()?,
        Err(_) => { let _ = store.rollback(); }
    }
    result
}

fn construct_owner_key_block_inner(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    op: OwnerKeyOp,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    let height = meta.block_height + 1;
    let shares_out = meta.shares_out.clone();

    // Apply the operation to the store
    let op_item = match &op {
        OwnerKeyOp::Rotation(vr) => {
            crate::owner_keys::apply_rotation(store, vr, height, block_timestamp)?;
            vr.item.clone()
        }
        OwnerKeyOp::Revocation(vr) => {
            crate::owner_keys::apply_revocation(store, vr, height)?;
            vr.item.clone()
        }
        OwnerKeyOp::Override(vo) => {
            crate::owner_keys::apply_override(store, vo, height)?;
            vo.item.clone()
        }
    };

    // Encode shares_out (unchanged for admin blocks)
    let mut shares_bytes = Vec::new();
    bigint::encode_bigint(&shares_out, &mut shares_bytes);

    // Build BLOCK_CONTENTS with the admin operation as a page
    let page = DataItem::container(PAGE, vec![
        DataItem::vbc_value(PAGE_INDEX, 0),
        op_item,
    ]);

    let block_contents = DataItem::container(BLOCK_CONTENTS, vec![
        DataItem::bytes(PREV_HASH, meta.prev_hash.to_vec()),
        DataItem::vbc_value(FIRST_SEQ, meta.next_seq_id),
        DataItem::vbc_value(SEQ_COUNT, 0), // no new sequences
        DataItem::vbc_value(LIST_SIZE, 1),
        DataItem::bytes(SHARES_OUT, shares_bytes),
        page,
    ]);

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

    let block_signed_bytes = block_signed.to_bytes();
    let block_hash = hash::sha256(&block_signed_bytes);

    let block = DataItem::container(BLOCK, vec![
        block_signed,
        DataItem::bytes(SHA256, block_hash.to_vec()),
    ]);

    let block_bytes = block.to_bytes();

    // Update chain state (no UTXO changes, no fee changes)
    store.store_block(height, block_timestamp, &block_hash, &block_bytes)?;
    store.advance_block(height, block_timestamp, &block_hash)?;
    // shares_out unchanged, next_seq_id unchanged

    Ok(ConstructedBlock {
        block,
        block_bytes,
        block_hash,
        height,
        timestamp: block_timestamp,
        new_shares_out: shares_out,
        first_seq: meta.next_seq_id,
        seq_count: 0,
    })
}

/// Construct and record a block containing a REWARD_RATE_CHANGE.
///
/// Administrative block: no UTXO mutations, no fee deduction.
/// Updates the chain's reward_rate_num/reward_rate_den after recording.
pub fn construct_reward_rate_change_block(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    vrc: ValidatedRateChange,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    if block_timestamp <= meta.last_block_timestamp {
        return Err(ChainError::TimestampOrder(
            format!("block timestamp {} <= previous {}",
                block_timestamp, meta.last_block_timestamp)));
    }

    store.begin_transaction()?;
    let result = (|| -> Result<ConstructedBlock> {
        let height = meta.block_height + 1;
        let shares_out = meta.shares_out.clone();

        // Apply the rate change to the store
        crate::reward_rate::apply_reward_rate_change(store, &vrc)?;

        // Encode shares_out (unchanged for admin blocks)
        let mut shares_bytes = Vec::new();
        bigint::encode_bigint(&shares_out, &mut shares_bytes);

        // Build BLOCK_CONTENTS with the rate change as a page
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            vrc.item,
        ]);

        let block_contents = DataItem::container(BLOCK_CONTENTS, vec![
            DataItem::bytes(PREV_HASH, meta.prev_hash.to_vec()),
            DataItem::vbc_value(FIRST_SEQ, meta.next_seq_id),
            DataItem::vbc_value(SEQ_COUNT, 0),
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::bytes(SHARES_OUT, shares_bytes),
            page,
        ]);

        // Sign block contents
        let ts = Timestamp::from_raw(block_timestamp);
        let sig_val = sign::sign_dataitem(blockmaker_key, &block_contents, ts);

        let block_signed = DataItem::container(BLOCK_SIGNED, vec![
            block_contents,
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig_val.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, blockmaker_key.public_key_bytes().to_vec()),
            ]),
        ]);

        let block_signed_bytes = block_signed.to_bytes();
        let block_hash = hash::sha256(&block_signed_bytes);

        let block = DataItem::container(BLOCK, vec![
            block_signed,
            DataItem::bytes(SHA256, block_hash.to_vec()),
        ]);

        let block_bytes = block.to_bytes();

        // Update chain state (no UTXO changes, no fee changes)
        store.store_block(height, block_timestamp, &block_hash, &block_bytes)?;
        store.advance_block(height, block_timestamp, &block_hash)?;

        Ok(ConstructedBlock {
            block,
            block_bytes,
            block_hash,
            height,
            timestamp: block_timestamp,
            new_shares_out: shares_out,
            first_seq: meta.next_seq_id,
            seq_count: 0,
        })
    })();
    match &result {
        Ok(_) => store.commit()?,
        Err(_) => { let _ = store.rollback(); }
    }
    result
}

/// Possible recorder switch operations for `construct_recorder_switch_block`.
pub enum RecorderSwitchOp {
    Pending(ValidatedPending),
    Change(ValidatedChange),
    UrlChange(ValidatedUrlChange),
}

/// Construct and record a block containing a recorder switch operation.
///
/// Administrative block: no UTXO mutations, no fee deduction.
/// For RECORDER_CHANGE_PENDING: sets pending state on the chain.
/// For RECORDER_CHANGE: updates recorder pubkey, clears pending state.
/// For RECORDER_URL_CHANGE: no state change beyond the on-chain record.
pub fn construct_recorder_switch_block(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    op: RecorderSwitchOp,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    if block_timestamp <= meta.last_block_timestamp {
        return Err(ChainError::TimestampOrder(
            format!("block timestamp {} <= previous {}",
                block_timestamp, meta.last_block_timestamp)));
    }

    store.begin_transaction()?;
    let result = construct_recorder_switch_inner(store, meta, blockmaker_key, op, block_timestamp);
    match &result {
        Ok(_) => store.commit()?,
        Err(_) => { let _ = store.rollback(); }
    }
    result
}

fn construct_recorder_switch_inner(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    op: RecorderSwitchOp,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    let height = meta.block_height + 1;
    let shares_out = meta.shares_out.clone();

    // Apply the operation and get the DataItem for the block page
    let op_item = match &op {
        RecorderSwitchOp::Pending(vp) => {
            crate::recorder_switch::apply_pending(store, vp, height)?;
            vp.item.clone()
        }
        RecorderSwitchOp::Change(vc) => {
            crate::recorder_switch::apply_change(store, vc)?;
            vc.item.clone()
        }
        RecorderSwitchOp::UrlChange(vu) => {
            // URL change is recorded on-chain only — no store metadata update needed.
            // Clients discover the current URL by scanning block history for the latest
            // RECORDER_URL_CHANGE or RECORDER_CHANGE block. There is no recorder_url
            // field in ChainMeta; the URL is purely an on-chain record.
            vu.item.clone()
        }
    };

    // Encode shares_out (unchanged for admin blocks)
    let mut shares_bytes = Vec::new();
    bigint::encode_bigint(&shares_out, &mut shares_bytes);

    let page = DataItem::container(PAGE, vec![
        DataItem::vbc_value(PAGE_INDEX, 0),
        op_item,
    ]);

    let block_contents = DataItem::container(BLOCK_CONTENTS, vec![
        DataItem::bytes(PREV_HASH, meta.prev_hash.to_vec()),
        DataItem::vbc_value(FIRST_SEQ, meta.next_seq_id),
        DataItem::vbc_value(SEQ_COUNT, 0),
        DataItem::vbc_value(LIST_SIZE, 1),
        DataItem::bytes(SHARES_OUT, shares_bytes),
        page,
    ]);

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

    let block_signed_bytes = block_signed.to_bytes();
    let block_hash = hash::sha256(&block_signed_bytes);

    let block = DataItem::container(BLOCK, vec![
        block_signed,
        DataItem::bytes(SHA256, block_hash.to_vec()),
    ]);

    let block_bytes = block.to_bytes();

    store.store_block(height, block_timestamp, &block_hash, &block_bytes)?;
    store.advance_block(height, block_timestamp, &block_hash)?;

    Ok(ConstructedBlock {
        block,
        block_bytes,
        block_hash,
        height,
        timestamp: block_timestamp,
        new_shares_out: shares_out,
        first_seq: meta.next_seq_id,
        seq_count: 0,
    })
}

/// Construct and record a CHAIN_MIGRATION (freeze) block.
///
/// This is the final block on the chain. After recording, the chain is frozen —
/// no further blocks may be produced. Administrative block: no UTXO mutations.
pub fn construct_migration_block(
    store: &ChainStore,
    meta: &ChainMeta,
    blockmaker_key: &SigningKey,
    vm: ValidatedMigration,
    block_timestamp: i64,
) -> Result<ConstructedBlock> {
    if block_timestamp <= meta.last_block_timestamp {
        return Err(ChainError::TimestampOrder(
            format!("block timestamp {} <= previous {}",
                block_timestamp, meta.last_block_timestamp)));
    }

    store.begin_transaction()?;
    let result = (|| -> Result<ConstructedBlock> {
        let height = meta.block_height + 1;
        let mut shares_out = meta.shares_out.clone();

        // Run expiry sweep before freezing so carried-forward UTXOs are accurate
        let expired_shares = expiry::run_expiry_sweep(store, meta, block_timestamp)?;
        shares_out -= &expired_shares;

        // Apply migration: mark chain as frozen
        crate::migration::apply_chain_migration(store)?;

        // Encode shares_out (updated after expiry sweep)
        let mut shares_bytes = Vec::new();
        bigint::encode_bigint(&shares_out, &mut shares_bytes);

        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            vm.item,
        ]);

        let block_contents = DataItem::container(BLOCK_CONTENTS, vec![
            DataItem::bytes(PREV_HASH, meta.prev_hash.to_vec()),
            DataItem::vbc_value(FIRST_SEQ, meta.next_seq_id),
            DataItem::vbc_value(SEQ_COUNT, 0),
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::bytes(SHARES_OUT, shares_bytes),
            page,
        ]);

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

        let block_signed_bytes = block_signed.to_bytes();
        let block_hash = hash::sha256(&block_signed_bytes);

        let block = DataItem::container(BLOCK, vec![
            block_signed,
            DataItem::bytes(SHA256, block_hash.to_vec()),
        ]);

        let block_bytes = block.to_bytes();

        store.store_block(height, block_timestamp, &block_hash, &block_bytes)?;
        store.advance_block(height, block_timestamp, &block_hash)?;

        Ok(ConstructedBlock {
            block,
            block_bytes,
            block_hash,
            height,
            timestamp: block_timestamp,
            new_shares_out: shares_out,
            first_seq: meta.next_seq_id,
            seq_count: 0,
        })
    })();
    match &result {
        Ok(_) => store.commit()?,
        Err(_) => { let _ = store.rollback(); }
    }
    result
}
