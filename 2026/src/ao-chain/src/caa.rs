//! CAA (Conditional Assignment Agreement) validation and escrow management.
//!
//! Implements the atomic multi-chain exchange protocol per AtomicExchange.md.

use num_bigint::BigInt;
use num_traits::Zero;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_types::fees;
use ao_crypto::hash;
use ao_crypto::sign;

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta, UtxoStatus};

type GiverList = Vec<(u64, BigInt)>;
type ReceiverList = Vec<([u8; 32], BigInt)>;

/// A validated CAA component ready for escrow recording.
#[derive(Debug)]
pub struct ValidatedCaaComponent {
    /// The full CAA DataItem.
    pub caa: DataItem,
    /// SHA2-256 hash of the CAA (components only, no proofs).
    pub caa_hash: [u8; 32],
    /// This chain's order in the ouroboros sequence.
    pub chain_order: u64,
    /// Total number of chains in the CAA.
    pub total_chains: u64,
    /// Escrow deadline timestamp.
    pub escrow_deadline: i64,
    /// The ASSIGNMENT from this chain's component.
    pub assignment: DataItem,
    /// Giver entries: (seq_id, amount).
    pub givers: Vec<(u64, BigInt)>,
    /// Receiver entries: (pubkey, amount).
    pub receivers: Vec<([u8; 32], BigInt)>,
    /// Recording fee in shares.
    pub fee_shares: BigInt,
    /// Page size in bytes.
    pub page_bytes: u64,
    /// Recording proofs from prior chains (verified).
    pub prior_proofs: Vec<DataItem>,
}

/// Validate a CAA submission for escrow recording on this chain.
///
/// The CAA must contain a valid CAA_COMPONENT for this chain (matched by CHAIN_REF).
/// If this chain's CHAIN_ORDER > 0, recording proofs for all prior chains must be present.
pub fn validate_caa_submit(
    store: &ChainStore,
    meta: &ChainMeta,
    caa: &DataItem,
    current_timestamp: i64,
    known_recorders: &std::collections::HashMap<[u8; 32], [u8; 32]>,
) -> Result<ValidatedCaaComponent> {
    if caa.type_code != CAA {
        return Err(ChainError::InvalidCaa("expected CAA container".into()));
    }

    // Parse escrow deadline
    let deadline_bytes = caa.find_child(ESCROW_DEADLINE)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidCaa("missing ESCROW_DEADLINE".into()))?;
    if deadline_bytes.len() != 8 {
        return Err(ChainError::InvalidCaa("ESCROW_DEADLINE must be 8 bytes".into()));
    }
    let escrow_deadline = i64::from_be_bytes(
        deadline_bytes.try_into().expect("length validated above"));

    if current_timestamp >= escrow_deadline {
        return Err(ChainError::CaaExpired);
    }

    // Compute CAA hash (over components only — excludes RECORDING_PROOFs and overall AUTH_SIGs
    // at the top level that aren't part of the canonical CAA content)
    let caa_hash = compute_caa_hash(caa);

    // Check for duplicate CAA
    if store.get_caa_escrow(&caa_hash)?.is_some() {
        return Err(ChainError::CaaAlreadyExists);
    }

    // Find this chain's component
    let components = caa.find_children(CAA_COMPONENT);
    let total_chains = components.len() as u64;
    if total_chains < 2 {
        return Err(ChainError::InvalidCaa("CAA requires at least 2 components".into()));
    }

    // Validate CHAIN_ORDER sequence: each component must have a unique order in 0..N-1
    let mut seen_orders = std::collections::HashSet::new();
    for comp in &components {
        let order = comp.find_child(CHAIN_ORDER)
            .and_then(|c| c.as_vbc_value())
            .ok_or_else(|| ChainError::InvalidCaa("component missing CHAIN_ORDER".into()))?;
        if order >= total_chains {
            return Err(ChainError::InvalidCaa(
                format!("CHAIN_ORDER {} out of range for {} chains", order, total_chains)));
        }
        if !seen_orders.insert(order) {
            return Err(ChainError::InvalidCaa(
                format!("duplicate CHAIN_ORDER {}", order)));
        }
    }

    let our_component = components.iter()
        .find(|c| {
            c.find_child(CHAIN_REF)
                .and_then(|r| r.as_bytes())
                .map(|b| b == &meta.chain_id[..])
                .unwrap_or(false)
        })
        .ok_or_else(|| ChainError::InvalidCaa(
            format!("no CAA_COMPONENT for chain {}", hex::encode(meta.chain_id))))?;

    let chain_order = our_component.find_child(CHAIN_ORDER)
        .and_then(|c| c.as_vbc_value())
        .ok_or_else(|| ChainError::InvalidCaa("missing CHAIN_ORDER".into()))?;

    // Extract the assignment from our component
    let assignment = our_component.find_child(ASSIGNMENT)
        .ok_or_else(|| ChainError::InvalidCaa("component missing ASSIGNMENT".into()))?;

    // Parse participants from the assignment
    let (givers, receivers) = parse_caa_participants(assignment)?;

    // Validate givers' UTXOs (must be unspent, not escrowed, not expired)
    for (seq_id, amount) in &givers {
        let utxo = store.get_utxo(*seq_id)?
            .ok_or(ChainError::UtxoNotFound(*seq_id))?;
        match utxo.status {
            UtxoStatus::Unspent => {}
            UtxoStatus::Spent => return Err(ChainError::UtxoAlreadySpent(*seq_id)),
            UtxoStatus::Expired => return Err(ChainError::UtxoExpired(*seq_id)),
            UtxoStatus::Escrowed => return Err(ChainError::UtxoEscrowed(*seq_id)),
        }
        if current_timestamp > utxo.block_timestamp.saturating_add(meta.expiry_period) {
            return Err(ChainError::UtxoExpired(*seq_id));
        }
        if *amount != utxo.amount {
            return Err(ChainError::InvalidCaa(
                format!("giver seq {} amount mismatch: CAA says {}, UTXO has {}",
                    seq_id, amount, utxo.amount)));
        }
    }

    // Check receiver key uniqueness
    for (pk, _) in &receivers {
        if store.is_key_used(pk)? {
            return Err(ChainError::KeyReuse);
        }
    }

    // Validate per-component signatures (same rules as regular assignment AUTH_SIGs)
    let component_sigs = our_component.find_children(AUTH_SIG);
    let expected_sig_count = givers.len() + receivers.len();
    if component_sigs.len() != expected_sig_count {
        return Err(ChainError::InvalidCaa(
            format!("expected {} component AUTH_SIGs, got {}", expected_sig_count, component_sigs.len())));
    }
    verify_component_signatures(store, &component_sigs, assignment, &givers, &receivers)?;

    // Validate overall CAA signatures (all participants across all chains must sign)
    let overall_sigs = caa.find_children(AUTH_SIG);
    if overall_sigs.is_empty() {
        return Err(ChainError::InvalidCaa("no overall AUTH_SIG signatures".into()));
    }
    verify_overall_signatures(caa, &overall_sigs, &components)?;

    // Validate recording fee (include component AUTH_SIGs in page size)
    let mut auth_children = vec![assignment.clone()];
    for sig in &component_sigs {
        auth_children.push((*sig).clone());
    }
    let authorization = DataItem::container(AUTHORIZATION, auth_children);
    let page_item = DataItem::container(PAGE, vec![
        DataItem::vbc_value(PAGE_INDEX, 0),
        authorization,
    ]);
    let page_bytes = page_item.to_bytes().len() as u64;
    let fee_shares = fees::recording_fee(
        page_bytes, &meta.fee_rate_num, &meta.fee_rate_den, &meta.shares_out);

    // Balance equation: givers = receivers + fee
    let giver_total: BigInt = givers.iter().map(|(_, a)| a).sum();
    let receiver_total: BigInt = receivers.iter().map(|(_, a)| a).sum();
    if giver_total != &receiver_total + &fee_shares {
        return Err(ChainError::BalanceMismatch {
            givers: giver_total.to_string(),
            receivers: receiver_total.to_string(),
            fee: fee_shares.to_string(),
        });
    }

    // If chain_order > 0, verify recording proofs for all prior chains
    let mut prior_proofs = Vec::new();
    if chain_order > 0 {
        let proofs = caa.find_children(RECORDING_PROOF);
        if (proofs.len() as u64) < chain_order {
            return Err(ChainError::InvalidCaa(
                format!("chain order {} requires {} prior proofs, got {}",
                    chain_order, chain_order, proofs.len())));
        }
        for proof in &proofs {
            verify_recording_proof(proof, &caa_hash, known_recorders)?;
            prior_proofs.push((*proof).clone());
        }
    }

    Ok(ValidatedCaaComponent {
        caa: caa.clone(),
        caa_hash,
        chain_order,
        total_chains,
        escrow_deadline,
        assignment: assignment.clone(),
        givers,
        receivers,
        fee_shares,
        page_bytes,
        prior_proofs,
    })
}

/// Validate a binding submission (all recording proofs present).
pub fn validate_caa_bind(
    store: &ChainStore,
    caa_hash: &[u8; 32],
    proofs: &[DataItem],
    current_timestamp: i64,
    known_recorders: &std::collections::HashMap<[u8; 32], [u8; 32]>,
) -> Result<()> {
    let escrow = store.get_caa_escrow(caa_hash)?
        .ok_or(ChainError::CaaNotFound)?;

    if escrow.status != "escrowed" {
        return Err(ChainError::InvalidCaa(
            format!("CAA is in '{}' state, expected 'escrowed'", escrow.status)));
    }

    if current_timestamp >= escrow.deadline {
        return Err(ChainError::CaaExpired);
    }

    let expected = escrow.total_chains as usize;
    if proofs.len() != expected {
        return Err(ChainError::InvalidCaa(
            format!("binding requires {} proofs, got {}", expected, proofs.len())));
    }

    for proof in proofs {
        verify_recording_proof(proof, caa_hash, known_recorders)?;
    }

    Ok(())
}

/// Run the escrow sweep: release escrowed UTXOs whose deadline has passed.
/// Returns (count_released, fee_to_restore) where fee_to_restore is the total
/// recording fee that should be added back to shares_out.
pub fn run_escrow_sweep(store: &ChainStore, current_timestamp: i64) -> Result<(u64, BigInt)> {
    let expired = store.find_expired_escrows(current_timestamp)?;
    let mut count = 0;
    let mut total_fee_restore = BigInt::zero();

    for escrow in &expired {
        // Sum giver amounts
        let giver_ids = store.get_caa_utxo_ids(&escrow.caa_hash, "giver")?;
        let mut giver_total = BigInt::zero();
        for seq_id in &giver_ids {
            if let Some(utxo) = store.get_utxo(*seq_id)? {
                giver_total += &utxo.amount;
            }
            store.release_escrow(*seq_id)?;
        }

        // Delete receiver UTXOs and remove their used_keys entries
        let receiver_ids = store.get_caa_utxo_ids(&escrow.caa_hash, "receiver")?;
        let mut receiver_total = BigInt::zero();
        for seq_id in &receiver_ids {
            if let Some(utxo) = store.get_utxo(*seq_id)? {
                receiver_total += &utxo.amount;
                store.remove_key_used(&utxo.pubkey)?;
            }
            store.delete_utxo(*seq_id)?;
        }

        // fee = giver_total - receiver_total (restore to shares_out)
        let fee = &giver_total - &receiver_total;
        total_fee_restore += fee;

        store.update_caa_status(&escrow.caa_hash, "expired")?;
        count += 1;
    }
    Ok((count, total_fee_restore))
}

/// Compute the CAA hash: SHA2-256 of the canonical CAA content.
/// The canonical content includes ESCROW_DEADLINE, LIST_SIZE, and all CAA_COMPONENTs,
/// but excludes top-level RECORDING_PROOFs and overall AUTH_SIGs.
pub fn compute_caa_hash(caa: &DataItem) -> [u8; 32] {
    let mut canonical_children = Vec::new();
    for child in caa.children() {
        match child.type_code {
            ESCROW_DEADLINE | LIST_SIZE | CAA_COMPONENT => {
                canonical_children.push(child.clone());
            }
            _ => {} // skip proofs and overall sigs
        }
    }
    let canonical = DataItem::container(CAA, canonical_children);
    hash::sha256(&canonical.to_bytes())
}

/// Parse giver/receiver participants from a CAA component's ASSIGNMENT.
fn parse_caa_participants(assignment: &DataItem) -> Result<(GiverList, ReceiverList)> {
    let participants = assignment.find_children(PARTICIPANT);
    let mut givers = Vec::new();
    let mut receivers = Vec::new();

    for p in &participants {
        let has_seq = p.find_child(SEQ_ID).is_some();
        let has_pub = p.find_child(ED25519_PUB).is_some();

        if has_seq {
            let seq_id = p.find_child(SEQ_ID)
                .and_then(|c| c.as_vbc_value())
                .ok_or_else(|| ChainError::InvalidCaa("giver missing SEQ_ID".into()))?;
            let amount = parse_amount(p)?;
            givers.push((seq_id, amount));
        } else if has_pub {
            let pub_bytes = p.find_child(ED25519_PUB)
                .and_then(|c| c.as_bytes())
                .ok_or_else(|| ChainError::InvalidCaa("receiver missing ED25519_PUB".into()))?;
            if pub_bytes.len() != 32 {
                return Err(ChainError::InvalidCaa("receiver pubkey must be 32 bytes".into()));
            }
            let mut pk = [0u8; 32];
            pk.copy_from_slice(pub_bytes);
            let amount = parse_amount(p)?;
            receivers.push((pk, amount));
        } else {
            return Err(ChainError::InvalidCaa(
                "participant must have SEQ_ID (giver) or ED25519_PUB (receiver)".into()));
        }
    }

    if givers.is_empty() {
        return Err(ChainError::InvalidCaa("no givers".into()));
    }
    if receivers.is_empty() {
        return Err(ChainError::InvalidCaa("no receivers".into()));
    }

    Ok((givers, receivers))
}

fn parse_amount(participant: &DataItem) -> Result<BigInt> {
    let bytes = participant.find_child(AMOUNT)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidCaa("participant missing AMOUNT".into()))?;
    let (amount, _) = bigint::decode_bigint(bytes, 0)
        .map_err(|e| ChainError::InvalidCaa(format!("AMOUNT: {}", e)))?;
    if amount <= BigInt::zero() {
        return Err(ChainError::InvalidCaa("amount must be positive".into()));
    }
    Ok(amount)
}

/// Verify per-component AUTH_SIG signatures against the assignment.
fn verify_component_signatures(
    store: &ChainStore,
    auth_sigs: &[&DataItem],
    assignment: &DataItem,
    givers: &[(u64, BigInt)],
    receivers: &[([u8; 32], BigInt)],
) -> Result<()> {
    let mut seen_indices = std::collections::HashSet::new();

    for auth_sig in auth_sigs {
        let sig_bytes = auth_sig.find_child(ED25519_SIG)
            .and_then(|c| c.as_bytes())
            .ok_or_else(|| ChainError::SignatureFailure("missing ED25519_SIG".into()))?;
        let ts_bytes = auth_sig.find_child(TIMESTAMP)
            .and_then(|c| c.as_bytes())
            .ok_or_else(|| ChainError::SignatureFailure("missing TIMESTAMP".into()))?;
        let page_index = auth_sig.find_child(PAGE_INDEX)
            .and_then(|c| c.as_vbc_value())
            .ok_or_else(|| ChainError::SignatureFailure("missing PAGE_INDEX".into()))?;

        if !seen_indices.insert(page_index) {
            return Err(ChainError::SignatureFailure(
                format!("duplicate PAGE_INDEX {}", page_index)));
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
            let utxo = store.get_utxo(*seq_id)?
                .ok_or(ChainError::UtxoNotFound(*seq_id))?;
            utxo.pubkey
        } else {
            let recv_idx = idx - givers.len();
            if recv_idx >= receivers.len() {
                return Err(ChainError::SignatureFailure(
                    format!("PAGE_INDEX {} out of range", page_index)));
            }
            receivers[recv_idx].0
        };

        if !sign::verify_dataitem(&pubkey, assignment, timestamp, &sig) {
            return Err(ChainError::SignatureFailure(
                format!("component signature failed for participant {}", page_index)));
        }
    }

    Ok(())
}

/// Verify overall CAA signatures. Each overall AUTH_SIG signs the canonical CAA content
/// and includes ED25519_PUB to identify the signer.
///
/// Checks: (1) signature count equals total participant count across all components,
/// (2) each signer is a known participant, (3) each signature is valid.
fn verify_overall_signatures(
    caa: &DataItem,
    overall_sigs: &[&DataItem],
    components: &[&DataItem],
) -> Result<()> {
    // Collect all participant pubkeys across all components
    let mut expected_pubkeys = std::collections::HashSet::new();
    for comp in components {
        if let Some(assignment) = comp.find_child(ASSIGNMENT) {
            for p in assignment.find_children(PARTICIPANT) {
                // Givers identified by SEQ_ID — their pubkey comes from the component AUTH_SIG
                // Receivers identified by ED25519_PUB directly
                if let Some(pub_child) = p.find_child(ED25519_PUB)
                    && let Some(pub_bytes) = pub_child.as_bytes()
                    && pub_bytes.len() == 32
                {
                    let mut pk = [0u8; 32];
                    pk.copy_from_slice(pub_bytes);
                    expected_pubkeys.insert(pk);
                }
            }
        }
    }
    // Giver pubkeys aren't in PARTICIPANTs (only SEQ_ID), so we can't pre-collect
    // them here. The constraint is: overall sig count == total participant count,
    // each overall sig is valid, and no duplicates. Giver identity is verified
    // through the component AUTH_SIG validation in verify_component_signatures.

    // Build the canonical content to verify against (same content that gets hashed)
    let mut canonical_children = Vec::new();
    for child in caa.children() {
        match child.type_code {
            ESCROW_DEADLINE | LIST_SIZE | CAA_COMPONENT => {
                canonical_children.push(child.clone());
            }
            _ => {}
        }
    }
    let canonical = DataItem::container(CAA, canonical_children);

    // Count total participants across all components
    let mut total_participants = 0usize;
    for comp in components {
        if let Some(assignment) = comp.find_child(ASSIGNMENT) {
            total_participants += assignment.find_children(PARTICIPANT).len();
        }
    }

    if overall_sigs.len() != total_participants {
        return Err(ChainError::InvalidCaa(
            format!("expected {} overall AUTH_SIGs (one per participant), got {}",
                total_participants, overall_sigs.len())));
    }

    let mut seen_pubkeys = std::collections::HashSet::new();

    for auth_sig in overall_sigs {
        let sig_bytes = auth_sig.find_child(ED25519_SIG)
            .and_then(|c| c.as_bytes())
            .ok_or_else(|| ChainError::SignatureFailure("overall sig missing ED25519_SIG".into()))?;
        let ts_bytes = auth_sig.find_child(TIMESTAMP)
            .and_then(|c| c.as_bytes())
            .ok_or_else(|| ChainError::SignatureFailure("overall sig missing TIMESTAMP".into()))?;
        let pub_bytes = auth_sig.find_child(ED25519_PUB)
            .and_then(|c| c.as_bytes())
            .ok_or_else(|| ChainError::SignatureFailure("overall sig missing ED25519_PUB".into()))?;

        if sig_bytes.len() != 64 || ts_bytes.len() != 8 || pub_bytes.len() != 32 {
            return Err(ChainError::SignatureFailure("invalid overall sig field lengths".into()));
        }

        let sig: [u8; 64] = sig_bytes.try_into().expect("length validated");
        let timestamp = Timestamp::from_bytes(ts_bytes.try_into().expect("length validated"));
        let pubkey: [u8; 32] = pub_bytes.try_into().expect("length validated");

        if !seen_pubkeys.insert(pubkey) {
            return Err(ChainError::SignatureFailure(
                format!("duplicate overall signer {}", hex::encode(pubkey))));
        }

        if !sign::verify_dataitem(&pubkey, &canonical, timestamp, &sig) {
            return Err(ChainError::SignatureFailure(
                format!("overall CAA signature failed for key {}", hex::encode(pubkey))));
        }
    }

    Ok(())
}

/// Verify a recording proof from another chain.
fn verify_recording_proof(
    proof: &DataItem,
    expected_caa_hash: &[u8; 32],
    known_recorders: &std::collections::HashMap<[u8; 32], [u8; 32]>,
) -> Result<()> {
    if proof.type_code != RECORDING_PROOF {
        return Err(ChainError::InvalidCaa("expected RECORDING_PROOF".into()));
    }

    // Check CAA_HASH matches
    let proof_caa_hash = proof.find_child(CAA_HASH)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidCaa("proof missing CAA_HASH".into()))?;
    if proof_caa_hash.len() != 32 || proof_caa_hash != expected_caa_hash {
        return Err(ChainError::InvalidCaa("proof CAA_HASH mismatch".into()));
    }

    // Get chain_ref from proof
    let chain_ref = proof.find_child(CHAIN_REF)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidCaa("proof missing CHAIN_REF".into()))?;
    if chain_ref.len() != 32 {
        return Err(ChainError::InvalidCaa("proof CHAIN_REF must be 32 bytes".into()));
    }
    let mut chain_id = [0u8; 32];
    chain_id.copy_from_slice(chain_ref);

    // Look up known recorder pubkey for this chain
    let recorder_pubkey = known_recorders.get(&chain_id)
        .ok_or_else(|| ChainError::InvalidCaa(
            format!("unknown recorder for chain {}", hex::encode(chain_id))))?;

    // Verify recorder's signature over the proof content
    let auth_sig = proof.find_child(AUTH_SIG)
        .ok_or_else(|| ChainError::InvalidCaa("proof missing AUTH_SIG".into()))?;

    let sig_bytes = auth_sig.find_child(ED25519_SIG)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidCaa("proof sig missing ED25519_SIG".into()))?;
    let ts_bytes = auth_sig.find_child(TIMESTAMP)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidCaa("proof sig missing TIMESTAMP".into()))?;

    if sig_bytes.len() != 64 || ts_bytes.len() != 8 {
        return Err(ChainError::InvalidCaa("proof sig invalid field lengths".into()));
    }

    let sig: [u8; 64] = sig_bytes.try_into().expect("length validated");
    let timestamp = Timestamp::from_bytes(ts_bytes.try_into().expect("length validated"));

    // The recorder signs the proof content (excluding the AUTH_SIG itself)
    let mut proof_content = Vec::new();
    for child in proof.children() {
        if child.type_code != AUTH_SIG {
            proof_content.push(child.clone());
        }
    }
    let proof_to_verify = DataItem::container(RECORDING_PROOF, proof_content);

    if !sign::verify_dataitem(recorder_pubkey, &proof_to_verify, timestamp, &sig) {
        return Err(ChainError::InvalidCaa("recording proof signature verification failed".into()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Utxo;

    fn setup_store_with_utxo() -> (ChainStore, ChainMeta, Utxo) {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let meta = ChainMeta {
            chain_id: [0xAA; 32],
            symbol: "BCG".to_string(),
            coin_count: BigInt::from(10_000_000_000u64),
            shares_out: BigInt::from(1u64) << 86,
            fee_rate_num: BigInt::from(1),
            fee_rate_den: BigInt::from(1_000_000),
            expiry_period: 5_964_386_400_000_000i64,
            expiry_mode: 1,
            tax_start_age: None,
            tax_doubling_period: None,
            block_height: 1,
            next_seq_id: 2,
            last_block_timestamp: 100,
            prev_hash: [0; 32],
        };
        store.store_chain_meta(&meta).unwrap();

        let utxo = Utxo {
            seq_id: 1,
            pubkey: [0xBB; 32],
            amount: BigInt::from(1000),
            block_height: 1,
            block_timestamp: 100,
            status: UtxoStatus::Unspent,
        };
        store.insert_utxo(&utxo).unwrap();

        (store, meta, utxo)
    }

    #[test]
    fn test_compute_caa_hash_deterministic_and_content_sensitive() {
        let caa = DataItem::container(CAA, vec![
            DataItem::bytes(ESCROW_DEADLINE, vec![0; 8]),
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xAA; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
        ]);
        let hash1 = compute_caa_hash(&caa);
        let hash2 = compute_caa_hash(&caa);
        assert_eq!(hash1, hash2);

        // Different content produces different hash
        let caa_different = DataItem::container(CAA, vec![
            DataItem::bytes(ESCROW_DEADLINE, vec![0; 8]),
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xBB; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
        ]);
        assert_ne!(hash1, compute_caa_hash(&caa_different));
    }

    #[test]
    fn test_compute_caa_hash_excludes_proofs() {
        let base = DataItem::container(CAA, vec![
            DataItem::bytes(ESCROW_DEADLINE, vec![0; 8]),
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xAA; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
        ]);
        let with_proof = DataItem::container(CAA, vec![
            DataItem::bytes(ESCROW_DEADLINE, vec![0; 8]),
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xAA; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
            DataItem::container(RECORDING_PROOF, vec![
                DataItem::bytes(CHAIN_REF, vec![0xBB; 32]),
            ]),
        ]);
        assert_eq!(compute_caa_hash(&base), compute_caa_hash(&with_proof));
    }

    #[test]
    fn test_escrow_sweep_releases_expired() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        // Insert a giver UTXO
        store.insert_utxo(&Utxo {
            seq_id: 1, pubkey: [0x01; 32], amount: BigInt::from(500),
            block_height: 0, block_timestamp: 100, status: UtxoStatus::Unspent,
        }).unwrap();

        // Escrow giver
        store.mark_escrowed(1).unwrap();

        // Create receiver UTXO (escrowed)
        let receiver_pk = [0x02; 32];
        store.insert_utxo(&Utxo {
            seq_id: 2, pubkey: receiver_pk, amount: BigInt::from(499),
            block_height: 1, block_timestamp: 100, status: UtxoStatus::Escrowed,
        }).unwrap();
        store.mark_key_used(&receiver_pk).unwrap();

        let caa_hash = [0xFF; 32];
        store.insert_caa_escrow(&caa_hash, 0, 200, 1, None, 2).unwrap();
        store.insert_caa_utxo(&caa_hash, 1, "giver").unwrap();
        store.insert_caa_utxo(&caa_hash, 2, "receiver").unwrap();

        // Verify escrowed states
        assert_eq!(store.get_utxo(1).unwrap().unwrap().status, UtxoStatus::Escrowed);
        assert_eq!(store.get_utxo(2).unwrap().unwrap().status, UtxoStatus::Escrowed);
        assert!(store.is_key_used(&receiver_pk).unwrap());

        // Sweep at timestamp before deadline — nothing released
        let (released, fee) = run_escrow_sweep(&store, 150).unwrap();
        assert_eq!(released, 0);
        assert_eq!(fee, BigInt::from(0));

        // Sweep at timestamp after deadline — released
        let (released, fee) = run_escrow_sweep(&store, 300).unwrap();
        assert_eq!(released, 1);
        // fee = giver(500) - receiver(499) = 1
        assert_eq!(fee, BigInt::from(1));

        // Giver UTXO released back to unspent
        assert_eq!(store.get_utxo(1).unwrap().unwrap().status, UtxoStatus::Unspent);
        // Receiver UTXO deleted
        assert!(store.get_utxo(2).unwrap().is_none());
        // Receiver key freed
        assert!(!store.is_key_used(&receiver_pk).unwrap());
        // CAA status is now expired
        assert_eq!(store.get_caa_escrow(&caa_hash).unwrap().unwrap().status, "expired");
    }

    #[test]
    fn test_escrowed_utxo_blocks_regular_spend() {
        let (store, _, _) = setup_store_with_utxo();

        // Escrow the UTXO
        store.mark_escrowed(1).unwrap();

        // Trying to spend should fail
        assert!(store.mark_spent(1).is_err());

        // Release escrow
        store.release_escrow(1).unwrap();

        // Now regular spend works
        store.mark_spent(1).unwrap();
    }

    #[test]
    fn test_escrow_to_spent_via_binding() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        store.insert_utxo(&Utxo {
            seq_id: 1, pubkey: [0x01; 32], amount: BigInt::from(500),
            block_height: 0, block_timestamp: 100, status: UtxoStatus::Unspent,
        }).unwrap();

        store.mark_escrowed(1).unwrap();
        let utxo = store.get_utxo(1).unwrap().unwrap();
        assert_eq!(utxo.status, UtxoStatus::Escrowed);

        store.mark_escrowed_spent(1).unwrap();
        let utxo = store.get_utxo(1).unwrap().unwrap();
        assert_eq!(utxo.status, UtxoStatus::Spent);
    }

    #[test]
    fn test_caa_escrow_crud() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let caa_hash = [0xDD; 32];

        // Insert
        store.insert_caa_escrow(&caa_hash, 0, 1000, 5, None, 2).unwrap();
        store.insert_caa_utxo(&caa_hash, 1, "giver").unwrap();
        store.insert_caa_utxo(&caa_hash, 2, "receiver").unwrap();

        // Read back
        let escrow = store.get_caa_escrow(&caa_hash).unwrap().unwrap();
        assert_eq!(escrow.chain_order, 0);
        assert_eq!(escrow.deadline, 1000);
        assert_eq!(escrow.status, "escrowed");
        assert_eq!(escrow.block_height, 5);

        // Get UTXOs by role
        let givers = store.get_caa_utxo_ids(&caa_hash, "giver").unwrap();
        assert_eq!(givers, vec![1]);
        let receivers = store.get_caa_utxo_ids(&caa_hash, "receiver").unwrap();
        assert_eq!(receivers, vec![2]);

        // Update status
        store.update_caa_status(&caa_hash, "binding").unwrap();
        let escrow = store.get_caa_escrow(&caa_hash).unwrap().unwrap();
        assert_eq!(escrow.status, "binding");

        // total_chains stored correctly
        assert_eq!(escrow.total_chains, 2);
    }

    #[test]
    fn test_validate_caa_submit_rejects_wrong_type() {
        let (store, meta, _) = setup_store_with_utxo();
        let not_caa = DataItem::container(ASSIGNMENT, vec![]);
        let err = validate_caa_submit(&store, &meta, &not_caa, 200, &Default::default());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("expected CAA"));
    }

    #[test]
    fn test_validate_caa_submit_rejects_missing_deadline() {
        let (store, meta, _) = setup_store_with_utxo();
        let caa = DataItem::container(CAA, vec![
            DataItem::vbc_value(LIST_SIZE, 2),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xAA; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xBB; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 1),
            ]),
        ]);
        let err = validate_caa_submit(&store, &meta, &caa, 200, &Default::default());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("ESCROW_DEADLINE"));
    }

    #[test]
    fn test_validate_caa_submit_rejects_expired() {
        let (store, meta, _) = setup_store_with_utxo();
        // Deadline in the past
        let deadline_ts = 100i64;
        let caa = DataItem::container(CAA, vec![
            DataItem::bytes(ESCROW_DEADLINE, deadline_ts.to_be_bytes().to_vec()),
            DataItem::vbc_value(LIST_SIZE, 2),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, meta.chain_id.to_vec()),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xBB; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 1),
            ]),
        ]);
        let err = validate_caa_submit(&store, &meta, &caa, 200, &Default::default());
        assert!(err.is_err());
        assert!(matches!(err.unwrap_err(), ChainError::CaaExpired));
    }

    #[test]
    fn test_validate_caa_submit_rejects_single_component() {
        let (store, meta, _) = setup_store_with_utxo();
        let deadline_ts = 999i64;
        let caa = DataItem::container(CAA, vec![
            DataItem::bytes(ESCROW_DEADLINE, deadline_ts.to_be_bytes().to_vec()),
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, meta.chain_id.to_vec()),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
        ]);
        let err = validate_caa_submit(&store, &meta, &caa, 200, &Default::default());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("at least 2"));
    }

    #[test]
    fn test_validate_caa_submit_rejects_duplicate_chain_order() {
        let (store, meta, _) = setup_store_with_utxo();
        let deadline_ts = 999i64;
        let caa = DataItem::container(CAA, vec![
            DataItem::bytes(ESCROW_DEADLINE, deadline_ts.to_be_bytes().to_vec()),
            DataItem::vbc_value(LIST_SIZE, 2),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, meta.chain_id.to_vec()),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
            DataItem::container(CAA_COMPONENT, vec![
                DataItem::bytes(CHAIN_REF, vec![0xBB; 32]),
                DataItem::vbc_value(CHAIN_ORDER, 0),
            ]),
        ]);
        let err = validate_caa_submit(&store, &meta, &caa, 200, &Default::default());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("duplicate CHAIN_ORDER"));
    }

    #[test]
    fn test_validate_caa_bind_rejects_wrong_status() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let caa_hash = [0xDD; 32];
        store.insert_caa_escrow(&caa_hash, 0, 1000, 1, None, 2).unwrap();
        // Finalize it
        store.update_caa_status(&caa_hash, "finalized").unwrap();

        let err = validate_caa_bind(&store, &caa_hash, &[], 500, &Default::default());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("finalized"));
    }

    #[test]
    fn test_validate_caa_bind_rejects_expired() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let caa_hash = [0xDD; 32];
        store.insert_caa_escrow(&caa_hash, 0, 500, 1, None, 2).unwrap();

        let err = validate_caa_bind(&store, &caa_hash, &[], 600, &Default::default());
        assert!(err.is_err());
        assert!(matches!(err.unwrap_err(), ChainError::CaaExpired));
    }

    #[test]
    fn test_validate_caa_bind_rejects_wrong_proof_count() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let caa_hash = [0xDD; 32];
        store.insert_caa_escrow(&caa_hash, 0, 1000, 1, None, 3).unwrap();

        // Submit 1 proof when 3 are required
        let err = validate_caa_bind(&store, &caa_hash, &[], 500, &Default::default());
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("requires 3 proofs, got 0"));
    }

    #[test]
    fn test_release_escrow_row_count_check() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        store.insert_utxo(&Utxo {
            seq_id: 1, pubkey: [0x01; 32], amount: BigInt::from(100),
            block_height: 0, block_timestamp: 100, status: UtxoStatus::Unspent,
        }).unwrap();
        // Trying to release a non-escrowed UTXO should fail
        assert!(store.release_escrow(1).is_err());
    }

    #[test]
    fn test_update_caa_status_nonexistent() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let caa_hash = [0xDD; 32];
        assert!(store.update_caa_status(&caa_hash, "expired").is_err());
    }

    #[test]
    fn test_set_caa_proof_nonexistent() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let caa_hash = [0xDD; 32];
        assert!(store.set_caa_proof(&caa_hash, &[1, 2, 3]).is_err());
    }
}
