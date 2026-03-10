//! Chain migration — TⒶ³ §7.
//!
//! Validates CHAIN_MIGRATION DataItems (type 133) which freeze an old chain
//! and point to a new chain. Also validates SURROGATE_PROOF items (type 135)
//! for surrogate-tier migration trust.

use num_bigint::BigInt;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign;

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta};

/// A validated chain migration (freeze) ready for recording.
#[derive(Debug)]
pub struct ValidatedMigration {
    /// The full CHAIN_MIGRATION DataItem.
    pub item: DataItem,
    /// New chain ID (SHA2-256 of new genesis).
    pub new_chain_id: [u8; 32],
    /// Whether an owner signature was present (Full tier).
    pub has_owner_sig: bool,
    /// Whether a recorder signature was present.
    pub has_recorder_sig: bool,
}

/// Validate a CHAIN_MIGRATION submission.
///
/// Checks:
/// - Type code is CHAIN_MIGRATION (133)
/// - Contains CHAIN_REF child (32-byte new chain ID)
/// - Chain is not already frozen
/// - No active CAA escrows
/// - AUTH_SIG children are valid (owner recommended, recorder optional)
pub fn validate_chain_migration(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedMigration> {
    if item.type_code != CHAIN_MIGRATION {
        return Err(ChainError::InvalidBlock(
            format!("expected CHAIN_MIGRATION ({}), got {}", CHAIN_MIGRATION, item.type_code)));
    }

    // Chain must not already be frozen
    if meta.frozen {
        return Err(ChainError::InvalidBlock("chain is already frozen".into()));
    }

    // No active escrows allowed
    let active_escrows = store.count_active_escrows()?;
    if active_escrows > 0 {
        return Err(ChainError::InvalidBlock(
            format!("cannot migrate with {} active escrows", active_escrows)));
    }

    // No pending recorder change allowed
    if meta.pending_recorder_change.is_some() {
        return Err(ChainError::InvalidBlock(
            "cannot migrate during pending recorder change".into()));
    }

    // Extract CHAIN_REF (new chain ID)
    let chain_ref = item.find_child(CHAIN_REF)
        .ok_or_else(|| ChainError::InvalidBlock("missing CHAIN_REF child".into()))?;
    let ref_bytes = chain_ref.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("CHAIN_REF has no bytes".into()))?;
    if ref_bytes.len() != 32 {
        return Err(ChainError::InvalidBlock("CHAIN_REF must be 32 bytes".into()));
    }
    let mut new_chain_id = [0u8; 32];
    new_chain_id.copy_from_slice(ref_bytes);

    // New chain ID must differ from current
    if new_chain_id == meta.chain_id {
        return Err(ChainError::InvalidBlock("CHAIN_REF cannot be the same chain".into()));
    }

    // Build signable content: CHAIN_MIGRATION without AUTH_SIG children
    let signable_children: Vec<&DataItem> = item.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .collect();
    let signable = DataItem::container(CHAIN_MIGRATION,
        signable_children.into_iter().cloned().collect());

    // Verify AUTH_SIG children (0, 1, or 2)
    let auth_sigs = item.find_children(AUTH_SIG);
    if auth_sigs.len() > 2 {
        return Err(ChainError::InvalidBlock(
            format!("CHAIN_MIGRATION allows at most 2 AUTH_SIG, got {}", auth_sigs.len())));
    }

    let mut has_owner_sig = false;
    let mut has_recorder_sig = false;

    for auth_sig in &auth_sigs {
        let (pubkey, sig, timestamp) = extract_auth_sig(auth_sig)?;

        // Verify signature
        if !sign::verify_dataitem(&pubkey, &signable, timestamp, &sig) {
            return Err(ChainError::SignatureFailure(
                format!("signature verification failed for pubkey {}", hex::encode(pubkey))));
        }

        // Classify signer
        if let Some(recorder_pk) = &meta.recorder_pubkey {
            if pubkey == *recorder_pk {
                if has_recorder_sig {
                    return Err(ChainError::InvalidBlock("duplicate recorder signature".into()));
                }
                has_recorder_sig = true;
                continue;
            }
        }

        if store.is_valid_owner_key(&pubkey, current_timestamp)? {
            if has_owner_sig {
                return Err(ChainError::InvalidBlock("duplicate owner signature".into()));
            }
            has_owner_sig = true;
        } else {
            return Err(ChainError::SignatureFailure(
                format!("signer {} is neither a valid owner key nor the recorder",
                    hex::encode(pubkey))));
        }
    }

    Ok(ValidatedMigration {
        item: item.clone(),
        new_chain_id,
        has_owner_sig,
        has_recorder_sig,
    })
}

/// Apply a validated chain migration: mark chain as frozen.
pub fn apply_chain_migration(store: &ChainStore) -> Result<()> {
    store.set_chain_frozen()
}

/// A validated surrogate proof for migration trust.
#[derive(Debug)]
pub struct ValidatedSurrogateProof {
    /// The UTXO sequence ID on the old chain.
    pub seq_id: u64,
    /// Share amount of this UTXO.
    pub amount: BigInt,
    /// New chain genesis hash this proof is bound to.
    pub new_chain_id: [u8; 32],
}

/// Validate a SURROGATE_PROOF against an old chain's UTXO set.
///
/// The proof must:
/// - Be validated against a frozen (migrated) chain
/// - Reference an existing unspent UTXO on the old chain
/// - Have the correct AMOUNT matching the UTXO
/// - Be signed by the UTXO's secret key
/// - Bind to a specific new chain via CHAIN_REF
pub fn validate_surrogate_proof(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    new_chain_id: &[u8; 32],
) -> Result<ValidatedSurrogateProof> {
    if item.type_code != SURROGATE_PROOF {
        return Err(ChainError::InvalidBlock(
            format!("expected SURROGATE_PROOF ({}), got {}", SURROGATE_PROOF, item.type_code)));
    }

    // Old chain must be frozen for surrogate proofs to be meaningful
    if !meta.frozen {
        return Err(ChainError::InvalidBlock(
            "SURROGATE_PROOF requires the old chain to be frozen".into()));
    }

    // Extract SEQ_ID
    let seq_id = item.find_child(SEQ_ID)
        .and_then(|c| c.as_vbc_value())
        .ok_or_else(|| ChainError::InvalidBlock("missing SEQ_ID in SURROGATE_PROOF".into()))?;

    // Extract AMOUNT
    let amount_item = item.find_child(AMOUNT)
        .ok_or_else(|| ChainError::InvalidBlock("missing AMOUNT in SURROGATE_PROOF".into()))?;
    let amount_bytes = amount_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("AMOUNT has no bytes".into()))?;
    let (amount, _) = bigint::decode_bigint(amount_bytes, 0)
        .map_err(|e| ChainError::InvalidBlock(format!("AMOUNT decode: {}", e)))?;

    // Extract CHAIN_REF
    let chain_ref = item.find_child(CHAIN_REF)
        .ok_or_else(|| ChainError::InvalidBlock("missing CHAIN_REF in SURROGATE_PROOF".into()))?;
    let ref_bytes = chain_ref.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("CHAIN_REF has no bytes".into()))?;
    if ref_bytes.len() != 32 {
        return Err(ChainError::InvalidBlock("CHAIN_REF must be 32 bytes".into()));
    }
    let mut proof_chain_id = [0u8; 32];
    proof_chain_id.copy_from_slice(ref_bytes);

    // CHAIN_REF must match the expected new chain
    if proof_chain_id != *new_chain_id {
        return Err(ChainError::InvalidBlock(
            "SURROGATE_PROOF CHAIN_REF does not match new chain ID".into()));
    }

    // Look up the UTXO on the old chain
    let utxo = store.get_utxo(seq_id)?
        .ok_or_else(|| ChainError::InvalidBlock(
            format!("UTXO seq {} not found on old chain", seq_id)))?;

    // UTXO must be unspent (chain should be frozen, so this is the final state)
    if utxo.status != crate::store::UtxoStatus::Unspent {
        return Err(ChainError::InvalidBlock(
            format!("UTXO seq {} is not unspent (status: {:?})", seq_id, utxo.status)));
    }

    // Amount must match
    if amount != utxo.amount {
        return Err(ChainError::InvalidBlock(
            format!("SURROGATE_PROOF amount mismatch for seq {}: claimed {}, actual {}",
                seq_id, amount, utxo.amount)));
    }

    // Verify AUTH_SIG signed by the UTXO's key
    let auth_sig = item.find_child(AUTH_SIG)
        .ok_or_else(|| ChainError::InvalidBlock("missing AUTH_SIG in SURROGATE_PROOF".into()))?;
    let (pubkey, sig, timestamp) = extract_auth_sig(auth_sig)?;

    if pubkey != utxo.pubkey {
        return Err(ChainError::SignatureFailure(
            format!("SURROGATE_PROOF signer {} does not match UTXO pubkey {}",
                hex::encode(pubkey), hex::encode(utxo.pubkey))));
    }

    // Build signable content: SURROGATE_PROOF without AUTH_SIG
    let signable_children: Vec<&DataItem> = item.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .collect();
    let signable = DataItem::container(SURROGATE_PROOF,
        signable_children.into_iter().cloned().collect());

    if !sign::verify_dataitem(&pubkey, &signable, timestamp, &sig) {
        return Err(ChainError::SignatureFailure(
            "SURROGATE_PROOF signature verification failed".into()));
    }

    Ok(ValidatedSurrogateProof {
        seq_id,
        amount,
        new_chain_id: proof_chain_id,
    })
}

/// Check if a set of surrogate proofs meets the >50% threshold.
///
/// Deduplicates by seq_id (each UTXO counted at most once).
/// Returns `true` if the sum of proven amounts exceeds half of `shares_out`.
pub fn check_surrogate_majority(
    proofs: &[ValidatedSurrogateProof],
    shares_out: &BigInt,
) -> bool {
    let mut seen = std::collections::HashSet::new();
    let mut total = BigInt::from(0);
    for p in proofs {
        if seen.insert(p.seq_id) {
            total += &p.amount;
        }
    }
    // Strict majority: total > shares_out / 2
    // Equivalent: 2 * total > shares_out (avoids fractional arithmetic)
    &total * BigInt::from(2) > *shares_out
}

// --- Helpers ---

/// Extract pubkey, signature, and timestamp from an AUTH_SIG container.
fn extract_auth_sig(auth_sig: &DataItem) -> Result<([u8; 32], [u8; 64], Timestamp)> {
    let sig_bytes = auth_sig.find_child(ED25519_SIG)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("missing ED25519_SIG in AUTH_SIG".into()))?;
    let ts_bytes = auth_sig.find_child(TIMESTAMP)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("missing TIMESTAMP in AUTH_SIG".into()))?;
    let pub_bytes = auth_sig.find_child(ED25519_PUB)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("missing ED25519_PUB in AUTH_SIG".into()))?;

    if sig_bytes.len() != 64 {
        return Err(ChainError::SignatureFailure("signature must be 64 bytes".into()));
    }
    if ts_bytes.len() != 8 {
        return Err(ChainError::SignatureFailure("timestamp must be 8 bytes".into()));
    }
    if pub_bytes.len() != 32 {
        return Err(ChainError::SignatureFailure("pubkey must be 32 bytes".into()));
    }

    let sig: [u8; 64] = sig_bytes.try_into().expect("validated above");
    let timestamp = Timestamp::from_bytes(ts_bytes.try_into().expect("validated above"));
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(pub_bytes);

    Ok((pubkey, sig, timestamp))
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_traits::Zero;
    use ao_crypto::sign::SigningKey;
    use crate::store::ChainStore;

    fn make_meta(chain_id: [u8; 32], recorder_pk: Option<[u8; 32]>) -> ChainMeta {
        let default_24h: i64 = 24 * 3600 * 189_000_000;
        ChainMeta {
            chain_id,
            symbol: "TST".into(),
            coin_count: BigInt::from(1_000_000),
            shares_out: BigInt::from(1_000_000),
            fee_rate_num: BigInt::from(1),
            fee_rate_den: BigInt::from(1000),
            expiry_period: 0,
            expiry_mode: 1,
            tax_start_age: None,
            tax_doubling_period: None,
            reward_rate_num: BigInt::from(0),
            reward_rate_den: BigInt::from(1),
            key_rotation_rate: default_24h,
            revocation_rate_base: default_24h,
            recorder_pubkey: recorder_pk,
            pending_recorder_change: None,
            frozen: false,
            block_height: 5,
            next_seq_id: 10,
            last_block_timestamp: 1000,
            prev_hash: [0; 32],
        }
    }

    fn sign_migration(
        key: &SigningKey,
        signable: &DataItem,
        ts: Timestamp,
    ) -> DataItem {
        let sig = sign::sign_dataitem(key, signable, ts);
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
        ])
    }

    #[test]
    fn test_chain_migration_with_owner_sig() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let chain_id = [1u8; 32];
        let new_chain_id = [2u8; 32];

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(chain_id, Some(recorder_pk));
        store.store_chain_meta(&meta).unwrap();

        // Register owner key
        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        let signable = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);

        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_migration(&owner_key, &signable, ts);

        let item = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
            owner_sig,
        ]);

        let result = validate_chain_migration(&store, &meta, &item, 3000);
        assert!(result.is_ok(), "Migration failed: {:?}", result.err());
        let vm = result.unwrap();
        assert_eq!(vm.new_chain_id, new_chain_id);
        assert!(vm.has_owner_sig);
        assert!(!vm.has_recorder_sig);
    }

    #[test]
    fn test_chain_migration_no_sigs_social_tier() {
        let chain_id = [1u8; 32];
        let new_chain_id = [2u8; 32];

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(chain_id, None);
        store.store_chain_meta(&meta).unwrap();

        // No AUTH_SIG — social tier
        let item = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);

        let result = validate_chain_migration(&store, &meta, &item, 3000);
        assert!(result.is_ok());
        let vm = result.unwrap();
        assert!(!vm.has_owner_sig);
        assert!(!vm.has_recorder_sig);
    }

    #[test]
    fn test_chain_migration_already_frozen() {
        let chain_id = [1u8; 32];
        let new_chain_id = [2u8; 32];

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let mut meta = make_meta(chain_id, None);
        meta.frozen = true;
        store.store_chain_meta(&meta).unwrap();

        let item = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);

        let result = validate_chain_migration(&store, &meta, &item, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already frozen"));
    }

    #[test]
    fn test_chain_migration_blocked_by_escrows() {
        let chain_id = [1u8; 32];
        let new_chain_id = [2u8; 32];

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(chain_id, None);
        store.store_chain_meta(&meta).unwrap();

        // Insert an active escrow
        store.insert_caa_escrow(&[0xAA; 32], 1, 9999, 1, None, 2, &BigInt::zero()).unwrap();

        let item = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);

        let result = validate_chain_migration(&store, &meta, &item, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("active escrows"));
    }

    #[test]
    fn test_surrogate_proof_valid() {
        let utxo_key = SigningKey::generate();
        let mut utxo_pk = [0u8; 32];
        utxo_pk.copy_from_slice(utxo_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        // Chain must be frozen for surrogate proofs
        let mut meta = make_meta([1; 32], None);
        meta.frozen = true;
        store.store_chain_meta(&meta).unwrap();

        // Insert a UTXO
        let amount = BigInt::from(600_000);
        store.insert_utxo(&crate::store::Utxo {
            seq_id: 1,
            pubkey: utxo_pk,
            amount: amount.clone(),
            block_height: 0,
            block_timestamp: 100,
            status: crate::store::UtxoStatus::Unspent,
        }).unwrap();

        let new_chain_id = [2u8; 32];

        // Encode amount
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(&amount, &mut amount_bytes);

        let signable = DataItem::container(SURROGATE_PROOF, vec![
            DataItem::vbc_value(SEQ_ID, 1),
            DataItem::bytes(AMOUNT, amount_bytes.clone()),
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);

        let ts = Timestamp::from_raw(2000);
        let sig = sign::sign_dataitem(&utxo_key, &signable, ts);
        let auth = DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::bytes(ED25519_PUB, utxo_key.public_key_bytes().to_vec()),
        ]);

        let item = DataItem::container(SURROGATE_PROOF, vec![
            DataItem::vbc_value(SEQ_ID, 1),
            DataItem::bytes(AMOUNT, amount_bytes),
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
            auth,
        ]);

        let result = validate_surrogate_proof(&store, &meta, &item, &new_chain_id);
        assert!(result.is_ok(), "Surrogate proof failed: {:?}", result.err());
        let vsp = result.unwrap();
        assert_eq!(vsp.seq_id, 1);
        assert_eq!(vsp.amount, BigInt::from(600_000));
    }

    #[test]
    fn test_surrogate_majority_check() {
        let shares_out = BigInt::from(1_000_000);

        // 600k > 500k = majority
        let proofs = vec![
            ValidatedSurrogateProof {
                seq_id: 1,
                amount: BigInt::from(600_000),
                new_chain_id: [2; 32],
            },
        ];
        assert!(check_surrogate_majority(&proofs, &shares_out));

        // 500k == 500k = NOT majority (strict >50%)
        let proofs_equal = vec![
            ValidatedSurrogateProof {
                seq_id: 1,
                amount: BigInt::from(500_000),
                new_chain_id: [2; 32],
            },
        ];
        assert!(!check_surrogate_majority(&proofs_equal, &shares_out));

        // 400k < 500k = not majority
        let proofs_less = vec![
            ValidatedSurrogateProof {
                seq_id: 1,
                amount: BigInt::from(400_000),
                new_chain_id: [2; 32],
            },
        ];
        assert!(!check_surrogate_majority(&proofs_less, &shares_out));

        // Duplicate seq_id should be deduplicated
        let proofs_dup = vec![
            ValidatedSurrogateProof {
                seq_id: 1,
                amount: BigInt::from(400_000),
                new_chain_id: [2; 32],
            },
            ValidatedSurrogateProof {
                seq_id: 1, // duplicate
                amount: BigInt::from(400_000),
                new_chain_id: [2; 32],
            },
        ];
        // Without dedup this would be 800k > 500k, but with dedup it's 400k
        assert!(!check_surrogate_majority(&proofs_dup, &shares_out));
    }

    #[test]
    fn test_chain_migration_blocked_by_pending_recorder_change() {
        let chain_id = [1u8; 32];
        let new_chain_id = [2u8; 32];

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let mut meta = make_meta(chain_id, None);
        meta.pending_recorder_change = Some(crate::store::PendingRecorderChange {
            new_recorder_pubkey: [0xCC; 32],
            new_recorder_url: "https://new.example.com".into(),
            pending_height: 3,
            owner_auth_sig_bytes: vec![],
        });
        store.store_chain_meta(&meta).unwrap();

        let item = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);

        let result = validate_chain_migration(&store, &meta, &item, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("pending recorder change"));
    }

    #[test]
    fn test_chain_migration_with_both_sigs() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let chain_id = [1u8; 32];
        let new_chain_id = [2u8; 32];

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(chain_id, Some(recorder_pk));
        store.store_chain_meta(&meta).unwrap();

        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        let signable = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);

        let ts = Timestamp::from_raw(2000);
        let owner_auth = sign_migration(&owner_key, &signable, ts);
        let recorder_auth = sign_migration(&recorder_key, &signable, ts);

        let item = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
            owner_auth,
            recorder_auth,
        ]);

        let result = validate_chain_migration(&store, &meta, &item, 3000);
        assert!(result.is_ok());
        let vm = result.unwrap();
        assert!(vm.has_owner_sig);
        assert!(vm.has_recorder_sig);
    }

    #[test]
    fn test_surrogate_proof_requires_frozen_chain() {
        let utxo_key = SigningKey::generate();
        let mut utxo_pk = [0u8; 32];
        utxo_pk.copy_from_slice(utxo_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta([1; 32], None); // NOT frozen

        store.insert_utxo(&crate::store::Utxo {
            seq_id: 1, pubkey: utxo_pk,
            amount: BigInt::from(600_000),
            block_height: 0, block_timestamp: 100,
            status: crate::store::UtxoStatus::Unspent,
        }).unwrap();

        let new_chain_id = [2u8; 32];
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(&BigInt::from(600_000), &mut amount_bytes);

        let item = DataItem::container(SURROGATE_PROOF, vec![
            DataItem::vbc_value(SEQ_ID, 1),
            DataItem::bytes(AMOUNT, amount_bytes),
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
            // AUTH_SIG omitted — won't get that far
        ]);

        let result = validate_surrogate_proof(&store, &meta, &item, &new_chain_id);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("frozen"));
    }
}
