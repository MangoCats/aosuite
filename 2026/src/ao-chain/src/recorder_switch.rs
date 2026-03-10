//! Recorder switch — TⒶ³ §5.
//!
//! Validates RECORDER_CHANGE_PENDING (130) and RECORDER_CHANGE (131) DataItems.
//! Also validates RECORDER_URL_CHANGE (132) dual-signed blocks.

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign;

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta, PendingRecorderChange};

/// Validated RECORDER_CHANGE_PENDING ready for recording.
#[derive(Debug)]
pub struct ValidatedPending {
    /// The full RECORDER_CHANGE_PENDING DataItem.
    pub item: DataItem,
    /// New recorder's public key.
    pub new_recorder_pubkey: [u8; 32],
    /// New recorder's URL hint.
    pub new_recorder_url: String,
    /// Owner's AUTH_SIG bytes — preserved for auto-constructed RECORDER_CHANGE.
    pub owner_auth_sig_bytes: Vec<u8>,
}

/// Validated RECORDER_CHANGE ready for recording.
#[derive(Debug)]
pub struct ValidatedChange {
    /// The full RECORDER_CHANGE DataItem.
    pub item: DataItem,
    /// New recorder's public key.
    pub new_recorder_pubkey: [u8; 32],
    /// New recorder's URL.
    pub new_recorder_url: String,
}

/// Validated RECORDER_URL_CHANGE ready for recording.
#[derive(Debug)]
pub struct ValidatedUrlChange {
    /// The full RECORDER_URL_CHANGE DataItem.
    pub item: DataItem,
    /// New recorder URL.
    pub new_url: String,
}

/// Validate a RECORDER_CHANGE_PENDING submission (§5.1 step 3).
///
/// Requirements:
/// - Must contain ED25519_PUB (new recorder key) and RECORDER_URL (new URL hint)
/// - Must have exactly 1 AUTH_SIG signed by a valid owner key
/// - Chain must not already have a pending recorder change
pub fn validate_pending(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedPending> {
    if item.type_code != RECORDER_CHANGE_PENDING {
        return Err(ChainError::InvalidBlock(
            format!("expected RECORDER_CHANGE_PENDING ({}), got {}", RECORDER_CHANGE_PENDING, item.type_code)));
    }

    // Must not already be in pending state
    if meta.pending_recorder_change.is_some() {
        return Err(ChainError::InvalidBlock(
            "recorder change already pending — complete or cancel before starting another".into()));
    }

    // Extract new recorder pubkey
    let pub_item = item.find_child(ED25519_PUB)
        .ok_or_else(|| ChainError::InvalidBlock("missing ED25519_PUB (new recorder key)".into()))?;
    let pub_bytes = pub_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("ED25519_PUB has no bytes".into()))?;
    if pub_bytes.len() != 32 {
        return Err(ChainError::InvalidBlock("ED25519_PUB must be 32 bytes".into()));
    }
    let mut new_recorder_pubkey = [0u8; 32];
    new_recorder_pubkey.copy_from_slice(pub_bytes);

    // Extract new recorder URL
    let url_item = item.find_child(RECORDER_URL)
        .ok_or_else(|| ChainError::InvalidBlock("missing RECORDER_URL".into()))?;
    let url_bytes = url_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("RECORDER_URL has no bytes".into()))?;
    let new_recorder_url = String::from_utf8(url_bytes.to_vec())
        .map_err(|_| ChainError::InvalidBlock("RECORDER_URL is not valid UTF-8".into()))?;

    // Must have exactly 1 AUTH_SIG
    let auth_sigs = item.find_children(AUTH_SIG);
    if auth_sigs.len() != 1 {
        return Err(ChainError::InvalidBlock(
            format!("RECORDER_CHANGE_PENDING requires exactly 1 AUTH_SIG, got {}", auth_sigs.len())));
    }

    // Build signable content (everything except AUTH_SIG)
    let signable_children: Vec<&DataItem> = item.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .collect();
    let signable = DataItem::container(RECORDER_CHANGE_PENDING,
        signable_children.into_iter().cloned().collect());

    // Verify the signature is from a valid owner key
    verify_owner_sig(store, &auth_sigs[0], &signable, current_timestamp)?;

    // Serialize the owner's AUTH_SIG for embedding in auto-constructed RECORDER_CHANGE
    let owner_auth_sig_bytes = auth_sigs[0].to_bytes();

    Ok(ValidatedPending {
        item: item.clone(),
        new_recorder_pubkey,
        new_recorder_url,
        owner_auth_sig_bytes,
    })
}

/// Apply a validated RECORDER_CHANGE_PENDING to the store.
pub fn apply_pending(
    store: &ChainStore,
    vp: &ValidatedPending,
    block_height: u64,
) -> Result<()> {
    store.set_pending_recorder_change(&PendingRecorderChange {
        new_recorder_pubkey: vp.new_recorder_pubkey,
        new_recorder_url: vp.new_recorder_url.clone(),
        pending_height: block_height,
        owner_auth_sig_bytes: vp.owner_auth_sig_bytes.clone(),
    })
}

/// Validate a RECORDER_CHANGE submission (§5.1 step 4).
///
/// Requirements:
/// - Must contain ED25519_PUB (new recorder key) and RECORDER_URL (new URL)
/// - Must have at least 1 AUTH_SIG from a valid owner key (REQUIRED)
/// - May have a second AUTH_SIG from the outgoing recorder (OPTIONAL)
/// - Chain must have a pending recorder change
/// - All active CAA escrows must be resolved (count == 0)
pub fn validate_change(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedChange> {
    if item.type_code != RECORDER_CHANGE {
        return Err(ChainError::InvalidBlock(
            format!("expected RECORDER_CHANGE ({}), got {}", RECORDER_CHANGE, item.type_code)));
    }

    // Must have a pending recorder change
    let pending = meta.pending_recorder_change.as_ref().ok_or_else(|| {
        ChainError::InvalidBlock("no RECORDER_CHANGE_PENDING recorded — must initiate pending first".into())
    })?;

    // All active escrows must be resolved
    let active = store.count_active_escrows()?;
    if active > 0 {
        return Err(ChainError::InvalidBlock(
            format!("{} active CAA escrow(s) remain — recorder change blocked until all resolve", active)));
    }

    // Extract new recorder pubkey
    let pub_item = item.find_child(ED25519_PUB)
        .ok_or_else(|| ChainError::InvalidBlock("missing ED25519_PUB (new recorder key)".into()))?;
    let pub_bytes = pub_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("ED25519_PUB has no bytes".into()))?;
    if pub_bytes.len() != 32 {
        return Err(ChainError::InvalidBlock("ED25519_PUB must be 32 bytes".into()));
    }
    let mut new_recorder_pubkey = [0u8; 32];
    new_recorder_pubkey.copy_from_slice(pub_bytes);

    // New key must match the pending key
    if new_recorder_pubkey != pending.new_recorder_pubkey {
        return Err(ChainError::InvalidBlock(
            "RECORDER_CHANGE pubkey does not match pending change".into()));
    }

    // Extract new recorder URL
    let url_item = item.find_child(RECORDER_URL)
        .ok_or_else(|| ChainError::InvalidBlock("missing RECORDER_URL".into()))?;
    let url_bytes = url_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("RECORDER_URL has no bytes".into()))?;
    let new_recorder_url = String::from_utf8(url_bytes.to_vec())
        .map_err(|_| ChainError::InvalidBlock("RECORDER_URL is not valid UTF-8".into()))?;

    // Must have 1 or 2 AUTH_SIG children
    let auth_sigs = item.find_children(AUTH_SIG);
    if auth_sigs.is_empty() || auth_sigs.len() > 2 {
        return Err(ChainError::InvalidBlock(
            format!("RECORDER_CHANGE requires 1-2 AUTH_SIG, got {}", auth_sigs.len())));
    }

    // Build signable content
    let signable_children: Vec<&DataItem> = item.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .collect();
    let signable = DataItem::container(RECORDER_CHANGE,
        signable_children.into_iter().cloned().collect());

    // At least one must be a valid owner key
    let mut found_owner = false;
    for auth_sig in &auth_sigs {
        let pubkey = extract_sig_pubkey(auth_sig)?;
        verify_sig(auth_sig, &signable)?;

        if store.is_valid_owner_key(&pubkey, current_timestamp)? {
            found_owner = true;
        }
        // Recorder sig is optional — we just verify it's valid
    }

    if !found_owner {
        return Err(ChainError::InvalidBlock(
            "RECORDER_CHANGE requires at least one owner signature".into()));
    }

    Ok(ValidatedChange {
        item: item.clone(),
        new_recorder_pubkey,
        new_recorder_url,
    })
}

/// Apply a validated RECORDER_CHANGE to the store.
pub fn apply_change(
    store: &ChainStore,
    vc: &ValidatedChange,
) -> Result<()> {
    store.set_recorder_pubkey(&vc.new_recorder_pubkey)?;
    store.clear_pending_recorder_change()
}

/// Validate a RECORDER_URL_CHANGE submission (§5.5).
///
/// Requirements:
/// - Must contain RECORDER_URL (new URL)
/// - Must have exactly 2 AUTH_SIG (one recorder, one owner) — dual-signed
pub fn validate_url_change(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedUrlChange> {
    if item.type_code != RECORDER_URL_CHANGE {
        return Err(ChainError::InvalidBlock(
            format!("expected RECORDER_URL_CHANGE ({}), got {}", RECORDER_URL_CHANGE, item.type_code)));
    }

    // Extract new URL
    let url_item = item.find_child(RECORDER_URL)
        .ok_or_else(|| ChainError::InvalidBlock("missing RECORDER_URL".into()))?;
    let url_bytes = url_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidBlock("RECORDER_URL has no bytes".into()))?;
    let new_url = String::from_utf8(url_bytes.to_vec())
        .map_err(|_| ChainError::InvalidBlock("RECORDER_URL is not valid UTF-8".into()))?;

    // Must have exactly 2 AUTH_SIG (recorder + owner)
    let auth_sigs = item.find_children(AUTH_SIG);
    if auth_sigs.len() != 2 {
        return Err(ChainError::InvalidBlock(
            format!("RECORDER_URL_CHANGE requires exactly 2 AUTH_SIG, got {}", auth_sigs.len())));
    }

    let recorder_pubkey = meta.recorder_pubkey.ok_or_else(|| {
        ChainError::InvalidBlock("no recorder pubkey set — cannot verify recorder signature".into())
    })?;

    // Build signable content
    let signable_children: Vec<&DataItem> = item.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .collect();
    let signable = DataItem::container(RECORDER_URL_CHANGE,
        signable_children.into_iter().cloned().collect());

    let mut found_owner = false;
    let mut found_recorder = false;

    for auth_sig in &auth_sigs {
        let pubkey = extract_sig_pubkey(auth_sig)?;
        verify_sig(auth_sig, &signable)?;

        if pubkey == recorder_pubkey {
            found_recorder = true;
        } else if store.is_valid_owner_key(&pubkey, current_timestamp)? {
            found_owner = true;
        } else {
            return Err(ChainError::SignatureFailure(
                format!("signer {} is neither a valid owner key nor the recorder",
                    hex::encode(pubkey))));
        }
    }

    if !found_owner {
        return Err(ChainError::InvalidBlock("missing owner signature on RECORDER_URL_CHANGE".into()));
    }
    if !found_recorder {
        return Err(ChainError::InvalidBlock("missing recorder signature on RECORDER_URL_CHANGE".into()));
    }

    Ok(ValidatedUrlChange {
        item: item.clone(),
        new_url,
    })
}

// ── Internal helpers ─────────────────────────────────────────────────

/// Extract the 32-byte public key from an AUTH_SIG.
fn extract_sig_pubkey(auth_sig: &DataItem) -> Result<[u8; 32]> {
    let pub_bytes = auth_sig.find_child(ED25519_PUB)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("missing ED25519_PUB in AUTH_SIG".into()))?;
    if pub_bytes.len() != 32 {
        return Err(ChainError::SignatureFailure("pubkey must be 32 bytes".into()));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(pub_bytes);
    Ok(pk)
}

/// Verify a signature in an AUTH_SIG against signable content.
fn verify_sig(auth_sig: &DataItem, signable: &DataItem) -> Result<()> {
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

    if !sign::verify_dataitem(&pubkey, signable, timestamp, &sig) {
        return Err(ChainError::SignatureFailure(
            format!("signature verification failed for pubkey {}", hex::encode(pubkey))));
    }
    Ok(())
}

/// Verify an AUTH_SIG is signed by a valid owner key.
fn verify_owner_sig(
    store: &ChainStore,
    auth_sig: &DataItem,
    signable: &DataItem,
    current_timestamp: i64,
) -> Result<()> {
    let pubkey = extract_sig_pubkey(auth_sig)?;
    verify_sig(auth_sig, signable)?;

    if !store.is_valid_owner_key(&pubkey, current_timestamp)? {
        return Err(ChainError::SignatureFailure(
            format!("signer {} is not a valid owner key", hex::encode(pubkey))));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
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
            reward_rate_num: BigInt::from(0),
            reward_rate_den: BigInt::from(1),
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

    fn sign_item(key: &SigningKey, signable: &DataItem, ts: Timestamp) -> DataItem {
        let sig = sign::sign_dataitem(key, signable, ts);
        DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
        ])
    }

    #[test]
    fn test_pending_valid() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let new_recorder = SigningKey::generate();

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

        let signable = DataItem::container(RECORDER_CHANGE_PENDING, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        ]);

        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_item(&owner_key, &signable, ts);

        let item = DataItem::container(RECORDER_CHANGE_PENDING, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
            owner_sig,
        ]);

        let result = validate_pending(&store, &meta, &item, 3000);
        assert!(result.is_ok(), "Valid pending failed: {:?}", result.err());
        let vp = result.unwrap();
        assert_eq!(vp.new_recorder_url, "https://new-recorder.example.com");
    }

    #[test]
    fn test_pending_rejected_when_already_pending() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let new_recorder = SigningKey::generate();

        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let mut meta = make_meta(recorder_pk);

        // Set pending state
        meta.pending_recorder_change = Some(PendingRecorderChange {
            new_recorder_pubkey: [2; 32],
            new_recorder_url: "https://old-pending.example.com".into(),
            pending_height: 5,
            owner_auth_sig_bytes: Vec::new(),
        });
        store.store_chain_meta(&meta).unwrap();

        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        let signable = DataItem::container(RECORDER_CHANGE_PENDING, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        ]);
        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_item(&owner_key, &signable, ts);

        let item = DataItem::container(RECORDER_CHANGE_PENDING, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
            owner_sig,
        ]);

        let result = validate_pending(&store, &meta, &item, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already pending"));
    }

    #[test]
    fn test_change_valid_no_escrows() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let new_recorder = SigningKey::generate();

        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());
        let mut new_pk = [0u8; 32];
        new_pk.copy_from_slice(new_recorder.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let mut meta = make_meta(recorder_pk);
        meta.pending_recorder_change = Some(PendingRecorderChange {
            new_recorder_pubkey: new_pk,
            new_recorder_url: "https://new-recorder.example.com".into(),
            pending_height: 5,
            owner_auth_sig_bytes: Vec::new(),
        });
        store.store_chain_meta(&meta).unwrap();

        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        let signable = DataItem::container(RECORDER_CHANGE, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        ]);
        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_item(&owner_key, &signable, ts);

        let item = DataItem::container(RECORDER_CHANGE, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
            owner_sig,
        ]);

        let result = validate_change(&store, &meta, &item, 3000);
        assert!(result.is_ok(), "Valid change failed: {:?}", result.err());
    }

    #[test]
    fn test_change_blocked_without_pending() {
        let owner_key = SigningKey::generate();
        let recorder_key = SigningKey::generate();
        let new_recorder = SigningKey::generate();

        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());

        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        let meta = make_meta(recorder_pk); // no pending
        store.store_chain_meta(&meta).unwrap();

        let mut owner_pk = [0u8; 32];
        owner_pk.copy_from_slice(owner_key.public_key_bytes());
        store.insert_owner_key(&owner_pk, 0, 100, None).unwrap();

        let signable = DataItem::container(RECORDER_CHANGE, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
        ]);
        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_item(&owner_key, &signable, ts);

        let item = DataItem::container(RECORDER_CHANGE, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new-recorder.example.com".to_vec()),
            owner_sig,
        ]);

        let result = validate_change(&store, &meta, &item, 3000);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("pending"));
    }

    #[test]
    fn test_url_change_valid() {
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

        let signable = DataItem::container(RECORDER_URL_CHANGE, vec![
            DataItem::bytes(RECORDER_URL, b"https://moved.example.com".to_vec()),
        ]);
        let ts = Timestamp::from_raw(2000);
        let owner_sig = sign_item(&owner_key, &signable, ts);
        let recorder_sig = sign_item(&recorder_key, &signable, ts);

        let item = DataItem::container(RECORDER_URL_CHANGE, vec![
            DataItem::bytes(RECORDER_URL, b"https://moved.example.com".to_vec()),
            owner_sig,
            recorder_sig,
        ]);

        let result = validate_url_change(&store, &meta, &item, 3000);
        assert!(result.is_ok(), "Valid URL change failed: {:?}", result.err());
        assert_eq!(result.unwrap().new_url, "https://moved.example.com");
    }

    #[test]
    fn test_url_change_missing_recorder_sig() {
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

        let signable = DataItem::container(RECORDER_URL_CHANGE, vec![
            DataItem::bytes(RECORDER_URL, b"https://moved.example.com".to_vec()),
        ]);
        let ts = Timestamp::from_raw(2000);
        // Two owner sigs, no recorder sig
        let sig1 = sign_item(&owner_key, &signable, ts);
        let sig2 = sign_item(&other_key, &signable, ts);

        let item = DataItem::container(RECORDER_URL_CHANGE, vec![
            DataItem::bytes(RECORDER_URL, b"https://moved.example.com".to_vec()),
            sig1,
            sig2,
        ]);

        let result = validate_url_change(&store, &meta, &item, 3000);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("recorder") || err.contains("duplicate"),
            "Expected recorder-related error, got: {}", err);
    }
}
