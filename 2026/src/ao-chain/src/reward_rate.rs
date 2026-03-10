//! Reward rate change — TⒶ³ §4.3.
//!
//! Validates REWARD_RATE_CHANGE DataItems (type 142) which require
//! dual signatures from both a valid owner key and the current recorder.
//! Produces a validated result that block construction can record.

use num_bigint::BigInt;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign;

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta};

/// A validated reward rate change ready for recording.
#[derive(Debug)]
pub struct ValidatedRateChange {
    /// The full REWARD_RATE_CHANGE DataItem.
    pub item: DataItem,
    /// New reward rate numerator.
    pub new_rate_num: BigInt,
    /// New reward rate denominator.
    pub new_rate_den: BigInt,
}

/// Validate a REWARD_RATE_CHANGE submission.
///
/// Requires exactly 2 AUTH_SIG children: one from a valid owner key,
/// one from the current recorder (identified by `meta.recorder_pubkey`).
pub fn validate_reward_rate_change(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedRateChange> {
    if item.type_code != REWARD_RATE_CHANGE {
        return Err(ChainError::InvalidBlock(
            format!("expected REWARD_RATE_CHANGE ({}), got {}", REWARD_RATE_CHANGE, item.type_code)));
    }

    // Extract new REWARD_RATE
    let rate_item = item.find_child(REWARD_RATE)
        .ok_or_else(|| ChainError::InvalidBlock("missing REWARD_RATE child".into()))?;
    let rate_bytes = rate_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("REWARD_RATE has no bytes".into()))?;
    let (new_rate, _) = bigint::decode_rational(rate_bytes, 0)
        .map_err(|e| ChainError::InvalidBlock(format!("REWARD_RATE decode: {}", e)))?;

    // Reward rate must be non-negative
    if *new_rate.numer() < BigInt::from(0) {
        return Err(ChainError::InvalidBlock("reward rate must be non-negative".into()));
    }
    if *new_rate.denom() <= BigInt::from(0) {
        return Err(ChainError::InvalidBlock("reward rate denominator must be positive".into()));
    }

    // Must have exactly 2 AUTH_SIG children
    let auth_sigs = item.find_children(AUTH_SIG);
    if auth_sigs.len() != 2 {
        return Err(ChainError::InvalidBlock(
            format!("REWARD_RATE_CHANGE requires exactly 2 AUTH_SIG, got {}", auth_sigs.len())));
    }

    // Recorder pubkey must be known
    let recorder_pubkey = meta.recorder_pubkey.ok_or_else(|| {
        ChainError::InvalidBlock("no recorder pubkey set on chain — cannot verify recorder signature".into())
    })?;

    // Build signable content: REWARD_RATE_CHANGE without AUTH_SIG children
    let signable_children: Vec<&DataItem> = item.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .collect();
    let signable = DataItem::container(REWARD_RATE_CHANGE,
        signable_children.into_iter().cloned().collect());

    // Verify both signatures — one must be owner, one must be recorder
    let mut found_owner = false;
    let mut found_recorder = false;

    for auth_sig in &auth_sigs {
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

        // Verify signature
        if !sign::verify_dataitem(&pubkey, &signable, timestamp, &sig) {
            return Err(ChainError::SignatureFailure(
                format!("signature verification failed for pubkey {}", hex::encode(pubkey))));
        }

        // Classify: is this an owner key or the recorder key?
        if pubkey == recorder_pubkey {
            if found_recorder {
                return Err(ChainError::InvalidBlock("duplicate recorder signature".into()));
            }
            found_recorder = true;
        } else if store.is_valid_owner_key(&pubkey, current_timestamp)? {
            if found_owner {
                return Err(ChainError::InvalidBlock("duplicate owner signature".into()));
            }
            found_owner = true;
        } else {
            return Err(ChainError::SignatureFailure(
                format!("signer {} is neither a valid owner key nor the recorder",
                    hex::encode(pubkey))));
        }
    }

    if !found_owner {
        return Err(ChainError::InvalidBlock("missing owner signature on REWARD_RATE_CHANGE".into()));
    }
    if !found_recorder {
        return Err(ChainError::InvalidBlock("missing recorder signature on REWARD_RATE_CHANGE".into()));
    }

    Ok(ValidatedRateChange {
        item: item.clone(),
        new_rate_num: new_rate.numer().clone(),
        new_rate_den: new_rate.denom().clone(),
    })
}

/// Apply a validated reward rate change to the store.
pub fn apply_reward_rate_change(
    store: &ChainStore,
    vrc: &ValidatedRateChange,
) -> Result<()> {
    store.update_reward_rate(&vrc.new_rate_num, &vrc.new_rate_den)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ao_crypto::sign::SigningKey;
    use crate::store::ChainStore;

    fn make_meta(recorder_pk: [u8; 32]) -> ChainMeta {
        let default_24h: i64 = 24 * 3600 * 189_000_000;
        ChainMeta {
            chain_id: [1; 32],
            symbol: "TST".into(),
            coin_count: BigInt::from(1_000_000),
            shares_out: BigInt::from(1_000_000),
            fee_rate_num: BigInt::from(1),
            fee_rate_den: BigInt::from(1000),
            expiry_period: 0,
            expiry_mode: 1,
            tax_start_age: None,
            tax_doubling_period: None,
            reward_rate_num: BigInt::from(1),
            reward_rate_den: BigInt::from(100),
            key_rotation_rate: default_24h,
            revocation_rate_base: default_24h,
            recorder_pubkey: Some(recorder_pk),
            pending_recorder_change: None,
            frozen: false,
            block_height: 5,
            next_seq_id: 10,
            last_block_timestamp: 1000,
            prev_hash: [0; 32],
        }
    }

    fn sign_rate_change(
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
    fn test_reward_rate_change_valid() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(recorder_pk);
        store.store_chain_meta(&meta).unwrap();

        // Register owner key
        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        // Build new rate: 2/100
        let new_rate = num_rational::BigRational::new(BigInt::from(2), BigInt::from(100));
        let mut rate_bytes = Vec::new();
        bigint::encode_rational(&new_rate, &mut rate_bytes);

        let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
        ]);

        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_rate_change(&owner_key, &signable, ts);
        let recorder_sig = sign_rate_change(&recorder_key, &signable, ts);

        let item = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes),
            owner_sig,
            recorder_sig,
        ]);

        let result = validate_reward_rate_change(&store, &meta, &item, 3000);
        assert!(result.is_ok(), "Valid rate change failed: {:?}", result.err());
        let vrc = result.unwrap();
        // BigRational normalizes 2/100 → 1/50
        assert_eq!(vrc.new_rate_num, BigInt::from(1));
        assert_eq!(vrc.new_rate_den, BigInt::from(50));
    }

    #[test]
    fn test_reward_rate_change_missing_recorder_sig() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let other_key = SigningKey::generate();
        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(recorder_pk);
        store.store_chain_meta(&meta).unwrap();

        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        // Register other_key as owner too
        let mut other_pk = [0u8; 32];
        other_pk.copy_from_slice(other_key.public_key_bytes());
        store.insert_owner_key(&other_pk, 0, 100, None).unwrap();

        let new_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(50));
        let mut rate_bytes = Vec::new();
        bigint::encode_rational(&new_rate, &mut rate_bytes);

        let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
        ]);

        let ts = Timestamp::from_raw(2000);
        // Two owner sigs, no recorder sig
        let owner_sig = sign_rate_change(&owner_key, &signable, ts);
        let other_sig = sign_rate_change(&other_key, &signable, ts);

        let item = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes),
            owner_sig,
            other_sig,
        ]);

        let result = validate_reward_rate_change(&store, &meta, &item, 3000);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("recorder") || err.contains("duplicate owner"),
            "Error should mention missing recorder: {}", err);
    }

    #[test]
    fn test_reward_rate_change_only_one_sig() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(recorder_pk);
        store.store_chain_meta(&meta).unwrap();

        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        let new_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(200));
        let mut rate_bytes = Vec::new();
        bigint::encode_rational(&new_rate, &mut rate_bytes);

        let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
        ]);

        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_rate_change(&owner_key, &signable, ts);

        // Only owner sig — should require 2
        let item = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes),
            owner_sig,
        ]);

        let result = validate_reward_rate_change(&store, &meta, &item, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("exactly 2"));
    }

    #[test]
    fn test_reward_rate_change_negative_rate_rejected() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(recorder_pk);
        store.store_chain_meta(&meta).unwrap();

        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        // Negative rate: -1/100
        let neg_rate = num_rational::BigRational::new(BigInt::from(-1), BigInt::from(100));
        let mut rate_bytes = Vec::new();
        bigint::encode_rational(&neg_rate, &mut rate_bytes);

        let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
        ]);

        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_rate_change(&owner_key, &signable, ts);
        let recorder_sig = sign_rate_change(&recorder_key, &signable, ts);

        let item = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes),
            owner_sig,
            recorder_sig,
        ]);

        let result = validate_reward_rate_change(&store, &meta, &item, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("non-negative"),
            "Expected non-negative error");
    }
}
