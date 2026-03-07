use num_bigint::BigInt;
use num_traits::Zero;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_types::fees;
use ao_crypto::sign;
use ao_crypto::hash;

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta, UtxoStatus};

type GiverList = Vec<(u64, BigInt)>;
type ReceiverList = Vec<([u8; 32], BigInt)>;

/// A parsed and validated assignment ready for recording.
#[derive(Debug)]
pub struct ValidatedAssignment {
    /// The full AUTHORIZATION DataItem.
    pub authorization: DataItem,
    /// Giver entries: (seq_id, amount).
    pub givers: GiverList,
    /// Receiver entries: (pubkey_32_bytes, amount).
    pub receivers: ReceiverList,
    /// Computed recording fee in shares.
    pub fee_shares: BigInt,
    /// Page size in bytes (for fee calculation).
    pub page_bytes: u64,
}

/// Validate a submitted AUTHORIZATION against current chain state.
/// Returns a ValidatedAssignment if everything checks out.
pub fn validate_assignment(
    store: &ChainStore,
    meta: &ChainMeta,
    authorization: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedAssignment> {
    if authorization.type_code != AUTHORIZATION {
        return Err(ChainError::InvalidAssignment(
            format!("expected AUTHORIZATION ({}), got {}", AUTHORIZATION, authorization.type_code)));
    }

    let assignment = authorization.find_child(ASSIGNMENT)
        .ok_or_else(|| ChainError::InvalidAssignment("missing ASSIGNMENT".into()))?;

    let (givers, receivers) = parse_participants(assignment)?;

    check_deadline(store, assignment, current_timestamp)?;

    validate_utxos(store, meta, &givers, &receivers, current_timestamp)?;

    let (fee_shares, page_bytes) = calculate_fee(authorization, meta)?;

    validate_balance(assignment, meta, &givers, &receivers, &fee_shares)?;

    verify_signatures(store, authorization, assignment, &givers, &receivers)?;

    Ok(ValidatedAssignment {
        authorization: authorization.clone(),
        givers,
        receivers,
        fee_shares,
        page_bytes,
    })
}

/// Parse PARTICIPANT items into giver and receiver lists.
fn parse_participants(assignment: &DataItem) -> Result<(GiverList, ReceiverList)> {
    let _list_size = assignment.find_child(LIST_SIZE)
        .and_then(|c| c.as_vbc_value())
        .ok_or_else(|| ChainError::InvalidAssignment("missing LIST_SIZE".into()))?;

    let participants = assignment.find_children(PARTICIPANT);
    if participants.is_empty() {
        return Err(ChainError::InvalidAssignment("no participants".into()));
    }

    let mut givers: GiverList = Vec::new();
    let mut receivers: ReceiverList = Vec::new();

    for p in &participants {
        let has_seq = p.find_child(SEQ_ID).is_some();
        let has_pub = p.find_child(ED25519_PUB).is_some();

        if has_seq {
            let seq_id = p.find_child(SEQ_ID)
                .and_then(|c| c.as_vbc_value())
                .ok_or_else(|| ChainError::InvalidAssignment("giver missing SEQ_ID".into()))?;
            let amount = parse_amount(p)?;
            givers.push((seq_id, amount));
        } else if has_pub {
            let pub_bytes = p.find_child(ED25519_PUB)
                .and_then(|c| c.as_bytes())
                .ok_or_else(|| ChainError::InvalidAssignment("receiver missing ED25519_PUB".into()))?;
            if pub_bytes.len() != 32 {
                return Err(ChainError::InvalidAssignment("receiver pubkey must be 32 bytes".into()));
            }
            let mut pk = [0u8; 32];
            pk.copy_from_slice(pub_bytes);
            let amount = parse_amount(p)?;
            receivers.push((pk, amount));
        } else {
            return Err(ChainError::InvalidAssignment(
                "participant must have either SEQ_ID (giver) or ED25519_PUB (receiver)".into()));
        }
    }

    if givers.is_empty() {
        return Err(ChainError::InvalidAssignment("no givers".into()));
    }
    if receivers.is_empty() {
        return Err(ChainError::InvalidAssignment("no receivers".into()));
    }

    Ok((givers, receivers))
}

/// Check deadline and late-recording refutation rules.
fn check_deadline(store: &ChainStore, assignment: &DataItem, current_timestamp: i64) -> Result<()> {
    if let Some(deadline_item) = assignment.find_child(DEADLINE) {
        let deadline_bytes = deadline_item.as_bytes()
            .ok_or_else(|| ChainError::InvalidAssignment("DEADLINE has no bytes".into()))?;
        if deadline_bytes.len() != 8 {
            return Err(ChainError::InvalidAssignment("DEADLINE must be 8 bytes".into()));
        }
        let deadline = i64::from_be_bytes(deadline_bytes.try_into().expect("length validated above"));
        if current_timestamp > deadline {
            let agreement_hash = hash::sha256(&assignment.to_bytes());
            if store.is_refuted(&agreement_hash)? {
                return Err(ChainError::AgreementRefuted);
            }
        }
    }
    Ok(())
}

/// Validate each giver's UTXO status and expiration, and check receiver key uniqueness.
fn validate_utxos(
    store: &ChainStore,
    meta: &ChainMeta,
    givers: &[(u64, BigInt)],
    receivers: &[([u8; 32], BigInt)],
    current_timestamp: i64,
) -> Result<()> {
    for (seq_id, amount) in givers {
        let utxo = store.get_utxo(*seq_id)?
            .ok_or(ChainError::UtxoNotFound(*seq_id))?;

        match utxo.status {
            UtxoStatus::Unspent => {}
            UtxoStatus::Spent => return Err(ChainError::UtxoAlreadySpent(*seq_id)),
            UtxoStatus::Expired => return Err(ChainError::UtxoExpired(*seq_id)),
        }

        if current_timestamp > utxo.block_timestamp.saturating_add(meta.expiry_period) {
            return Err(ChainError::UtxoExpired(*seq_id));
        }

        if *amount != utxo.amount {
            return Err(ChainError::InvalidAssignment(
                format!("giver seq {} amount mismatch: assignment says {}, UTXO has {}",
                    seq_id, amount, utxo.amount)));
        }
    }

    for (pk, _) in receivers {
        if store.is_key_used(pk)? {
            return Err(ChainError::KeyReuse);
        }
    }

    Ok(())
}

/// Calculate recording fee from PAGE encoding size.
fn calculate_fee(authorization: &DataItem, meta: &ChainMeta) -> Result<(BigInt, u64)> {
    let page_item = DataItem::container(PAGE, vec![
        DataItem::vbc_value(PAGE_INDEX, 0),
        authorization.clone(),
    ]);
    let page_bytes = page_item.to_bytes().len() as u64;
    let fee_shares = fees::recording_fee(
        page_bytes,
        &meta.fee_rate_num,
        &meta.fee_rate_den,
        &meta.shares_out,
    );
    Ok((fee_shares, page_bytes))
}

/// Validate recording bid and balance equation (givers = receivers + fee).
fn validate_balance(
    assignment: &DataItem,
    meta: &ChainMeta,
    givers: &[(u64, BigInt)],
    receivers: &[([u8; 32], BigInt)],
    fee_shares: &BigInt,
) -> Result<()> {
    if let Some(bid_item) = assignment.find_child(RECORDING_BID) {
        let bid_bytes = bid_item.as_bytes()
            .ok_or_else(|| ChainError::InvalidAssignment("RECORDING_BID has no bytes".into()))?;
        let (bid_rational, _) = bigint::decode_rational(bid_bytes, 0)
            .map_err(|e| ChainError::InvalidAssignment(format!("RECORDING_BID: {}", e)))?;
        let chain_rate = num_rational::BigRational::new(
            meta.fee_rate_num.clone(), meta.fee_rate_den.clone());
        if bid_rational < chain_rate {
            return Err(ChainError::InvalidAssignment(
                format!("recording bid {} is below chain fee rate {}", bid_rational, chain_rate)));
        }
    }

    let giver_total: BigInt = givers.iter().map(|(_, a)| a).sum();
    let receiver_total: BigInt = receivers.iter().map(|(_, a)| a).sum();

    if giver_total != &receiver_total + fee_shares {
        return Err(ChainError::BalanceMismatch {
            givers: giver_total.to_string(),
            receivers: receiver_total.to_string(),
            fee: fee_shares.to_string(),
        });
    }

    Ok(())
}

/// Verify all AUTH_SIG signatures with timestamp ordering checks.
fn verify_signatures(
    store: &ChainStore,
    authorization: &DataItem,
    assignment: &DataItem,
    givers: &[(u64, BigInt)],
    receivers: &[([u8; 32], BigInt)],
) -> Result<()> {
    let auth_sigs = authorization.find_children(AUTH_SIG);
    let expected_sig_count = givers.len() + receivers.len();
    if auth_sigs.len() != expected_sig_count {
        return Err(ChainError::InvalidAssignment(
            format!("expected {} AUTH_SIG items, got {}", expected_sig_count, auth_sigs.len())));
    }

    let mut seen_indices = std::collections::HashSet::new();

    for auth_sig in &auth_sigs {
        let sig_bytes = auth_sig.find_child(ED25519_SIG)
            .and_then(|c| c.as_bytes())
            .ok_or_else(|| ChainError::SignatureFailure("missing ED25519_SIG in AUTH_SIG".into()))?;
        let ts_bytes = auth_sig.find_child(TIMESTAMP)
            .and_then(|c| c.as_bytes())
            .ok_or_else(|| ChainError::SignatureFailure("missing TIMESTAMP in AUTH_SIG".into()))?;
        let page_index = auth_sig.find_child(PAGE_INDEX)
            .and_then(|c| c.as_vbc_value())
            .ok_or_else(|| ChainError::SignatureFailure("missing PAGE_INDEX in AUTH_SIG".into()))?;

        if !seen_indices.insert(page_index) {
            return Err(ChainError::SignatureFailure(
                format!("duplicate PAGE_INDEX {} in AUTH_SIG", page_index)));
        }

        if sig_bytes.len() != 64 {
            return Err(ChainError::SignatureFailure("signature must be 64 bytes".into()));
        }
        if ts_bytes.len() != 8 {
            return Err(ChainError::SignatureFailure("timestamp must be 8 bytes".into()));
        }

        let sig: [u8; 64] = sig_bytes.try_into().expect("length validated above");
        let timestamp = Timestamp::from_bytes(ts_bytes.try_into().expect("length validated above"));

        let idx = page_index as usize;
        let pubkey = if idx < givers.len() {
            let (seq_id, _) = &givers[idx];
            let utxo = store.get_utxo(*seq_id)?.unwrap();
            utxo.pubkey
        } else {
            let recv_idx = idx - givers.len();
            if recv_idx >= receivers.len() {
                return Err(ChainError::SignatureFailure(
                    format!("PAGE_INDEX {} out of range", page_index)));
            }
            receivers[recv_idx].0
        };

        if idx < givers.len() {
            let (seq_id, _) = &givers[idx];
            let utxo = store.get_utxo(*seq_id)?.unwrap();
            if timestamp.raw() <= utxo.block_timestamp {
                return Err(ChainError::TimestampOrder(
                    format!("giver seq {} signature timestamp {} <= receipt timestamp {}",
                        seq_id, timestamp.raw(), utxo.block_timestamp)));
            }
        }

        if !sign::verify_dataitem(&pubkey, assignment, timestamp, &sig) {
            return Err(ChainError::SignatureFailure(
                format!("signature verification failed for participant {}", page_index)));
        }
    }

    Ok(())
}

fn parse_amount(participant: &DataItem) -> Result<BigInt> {
    let bytes = participant.find_child(AMOUNT)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidAssignment("participant missing AMOUNT".into()))?;
    let (amount, _) = bigint::decode_bigint(bytes, 0)
        .map_err(|e| ChainError::InvalidAssignment(format!("AMOUNT: {}", e)))?;
    if amount <= BigInt::zero() {
        return Err(ChainError::InvalidAssignment("amount must be positive".into()));
    }
    Ok(amount)
}
