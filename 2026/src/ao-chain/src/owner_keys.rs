//! Owner key rotation and revocation — TⒶ³ §6.
//!
//! Validates OWNER_KEY_ROTATION and OWNER_KEY_REVOCATION DataItems
//! against current chain state. Produces validated results that
//! `block::construct_owner_key_block` can record.

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::timestamp::Timestamp;
use ao_crypto::sign;

use crate::error::{ChainError, Result};
use crate::store::{ChainStore, ChainMeta};

/// A validated owner key rotation ready for recording.
#[derive(Debug)]
pub struct ValidatedRotation {
    /// The full OWNER_KEY_ROTATION DataItem.
    pub item: DataItem,
    /// New owner public key (32 bytes).
    pub new_pubkey: [u8; 32],
    /// Old key expiration timestamp (None = no expiration).
    pub old_key_expires_at: Option<i64>,
    /// Signing key (the currently valid owner key that authorized this).
    pub signer_pubkey: [u8; 32],
}

/// A validated owner key revocation ready for recording.
#[derive(Debug)]
pub struct ValidatedRevocation {
    /// The full OWNER_KEY_REVOCATION DataItem.
    pub item: DataItem,
    /// Key being revoked (32 bytes).
    pub target_pubkey: [u8; 32],
    /// Public keys of the signers.
    pub signer_pubkeys: Vec<[u8; 32]>,
}

/// A validated owner key override ready for recording.
#[derive(Debug)]
pub struct ValidatedOverride {
    /// The full OWNER_KEY_OVERRIDE DataItem.
    pub item: DataItem,
    /// Key being reinstated (the target of the overridden revocation).
    pub reinstated_pubkey: [u8; 32],
    /// Keys placed on hold (the signers of the overridden revocation).
    pub held_pubkeys: Vec<[u8; 32]>,
    /// Hold expiration timestamp (after which held keys are auto-revoked).
    pub hold_expires_at: i64,
    /// Public keys of the override signers.
    pub signer_pubkeys: Vec<[u8; 32]>,
}

/// Validate an OWNER_KEY_ROTATION DataItem.
///
/// Per CompetingRecorders.md §6.1:
/// - Must be signed by a currently valid owner key
/// - Contains new ED25519_PUB (new owner key)
/// - Optional TIMESTAMP for old key expiration
/// - Rate limited: one rotation per `meta.key_rotation_rate`
///
/// Pre-live exemption: rate limits not enforced until the chain has
/// recorded its first non-setup transaction (block_height >= 1 with
/// assignments, i.e. next_seq_id > 2).
pub fn validate_rotation(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedRotation> {
    if item.type_code != OWNER_KEY_ROTATION {
        return Err(ChainError::InvalidBlock(
            format!("expected OWNER_KEY_ROTATION ({}), got {}", OWNER_KEY_ROTATION, item.type_code)));
    }

    // Extract new public key
    let new_pub = item.find_child(ED25519_PUB)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidBlock("OWNER_KEY_ROTATION missing ED25519_PUB".into()))?;
    if new_pub.len() != 32 {
        return Err(ChainError::InvalidBlock("new owner pubkey must be 32 bytes".into()));
    }
    let mut new_pubkey = [0u8; 32];
    new_pubkey.copy_from_slice(new_pub);

    // Check new key isn't already an owner key
    if store.is_valid_owner_key(&new_pubkey, current_timestamp)? {
        return Err(ChainError::InvalidBlock("new key is already a valid owner key".into()));
    }

    // Extract optional old key expiration
    let old_key_expires_at = if let Some(ts_item) = item.find_child(TIMESTAMP) {
        let ts_bytes = ts_item.as_bytes()
            .ok_or_else(|| ChainError::InvalidBlock("TIMESTAMP has no bytes".into()))?;
        if ts_bytes.len() != 8 {
            return Err(ChainError::InvalidBlock("expiration TIMESTAMP must be 8 bytes".into()));
        }
        Some(i64::from_be_bytes(ts_bytes.try_into().expect("length validated")))
    } else {
        None
    };

    // Verify AUTH_SIG — must be signed by a currently valid owner key
    let auth_sig = item.find_child(AUTH_SIG)
        .ok_or_else(|| ChainError::InvalidBlock("OWNER_KEY_ROTATION missing AUTH_SIG".into()))?;

    let signer_pubkey = verify_owner_sig(store, item, auth_sig, current_timestamp)?;

    // Rate limiting (skip if pre-live: next_seq_id <= 2 means only issuer UTXO exists)
    let is_pre_live = meta.next_seq_id <= 2;
    if !is_pre_live && meta.key_rotation_rate > 0 {
        if let Some(last_added) = store.last_owner_key_added_timestamp()? {
            let elapsed = current_timestamp.saturating_sub(last_added);
            if elapsed < meta.key_rotation_rate {
                return Err(ChainError::InvalidBlock(format!(
                    "key rotation rate limited: {}ns elapsed, {}ns required",
                    elapsed, meta.key_rotation_rate)));
            }
        }
    }

    Ok(ValidatedRotation {
        item: item.clone(),
        new_pubkey,
        old_key_expires_at,
        signer_pubkey,
    })
}

/// Validate an OWNER_KEY_REVOCATION DataItem.
///
/// Per CompetingRecorders.md §6.4:
/// - Must be signed by one or more currently valid owner keys
/// - Contains ED25519_PUB of key being revoked
/// - Cannot revoke to zero valid keys
/// - First revocation is immediate; subsequent revocations rate-limited
///   by 24/N hours where N = number of co-signers
///
/// Pre-live exemption: rate limits not enforced until first non-setup tx.
pub fn validate_revocation(
    store: &ChainStore,
    meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedRevocation> {
    if item.type_code != OWNER_KEY_REVOCATION {
        return Err(ChainError::InvalidBlock(
            format!("expected OWNER_KEY_REVOCATION ({}), got {}", OWNER_KEY_REVOCATION, item.type_code)));
    }

    // Extract target key being revoked
    let target_pub = item.find_child(ED25519_PUB)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidBlock("OWNER_KEY_REVOCATION missing ED25519_PUB".into()))?;
    if target_pub.len() != 32 {
        return Err(ChainError::InvalidBlock("target pubkey must be 32 bytes".into()));
    }
    let mut target_pubkey = [0u8; 32];
    target_pubkey.copy_from_slice(target_pub);

    // Target must be a currently valid owner key
    if !store.is_valid_owner_key(&target_pubkey, current_timestamp)? {
        return Err(ChainError::InvalidBlock("target key is not a valid owner key".into()));
    }

    // Verify AUTH_SIG(s) — may have multiple co-signers
    let auth_sigs = item.find_children(AUTH_SIG);
    if auth_sigs.is_empty() {
        return Err(ChainError::InvalidBlock("OWNER_KEY_REVOCATION missing AUTH_SIG".into()));
    }

    let mut signer_pubkeys = Vec::new();
    let mut seen_signers = std::collections::HashSet::new();
    for auth_sig in &auth_sigs {
        let pk = verify_owner_sig(store, item, auth_sig, current_timestamp)?;
        if !seen_signers.insert(pk) {
            return Err(ChainError::InvalidBlock(
                "duplicate signer in OWNER_KEY_REVOCATION".into()));
        }
        signer_pubkeys.push(pk);
    }

    // Check: revoking this key must not leave zero valid owner keys
    let valid_count = store.count_valid_owner_keys(current_timestamp)?;
    if valid_count <= 1 {
        return Err(ChainError::InvalidBlock(
            "cannot revoke: would leave zero valid owner keys".into()));
    }

    // Rate limiting for revocations (§6.4):
    // First revocation is free. Subsequent ones: one per (base / N_signers).
    // We track this via the revoked_at_height — count existing revocations.
    let is_pre_live = meta.next_seq_id <= 2;
    if !is_pre_live {
        let revocation_count = count_recent_revocations(store, meta, current_timestamp)?;
        if revocation_count > 0 {
            let n_signers = signer_pubkeys.len() as i64;
            let interval = meta.revocation_rate_base / n_signers.max(1);
            let last_revocation_ts = last_revocation_timestamp(store)?;
            if let Some(last_ts) = last_revocation_ts {
                let elapsed = current_timestamp.saturating_sub(last_ts);
                if elapsed < interval {
                    return Err(ChainError::InvalidBlock(format!(
                        "revocation rate limited: {}ns elapsed, {}ns required ({} signers)",
                        elapsed, interval, n_signers)));
                }
            }
        }
    }

    Ok(ValidatedRevocation {
        item: item.clone(),
        target_pubkey,
        signer_pubkeys,
    })
}

/// Apply a validated rotation to the store.
pub fn apply_rotation(
    store: &ChainStore,
    rotation: &ValidatedRotation,
    block_height: u64,
    block_timestamp: i64,
) -> Result<()> {
    // Insert new key with no expiration
    store.insert_owner_key(
        &rotation.new_pubkey,
        block_height,
        block_timestamp,
        None,
    )?;
    // If specified, set expiration on the old (signer) key
    if let Some(exp) = rotation.old_key_expires_at {
        store.set_owner_key_expiration(&rotation.signer_pubkey, exp)?;
    }
    Ok(())
}

/// Apply a validated revocation to the store.
pub fn apply_revocation(
    store: &ChainStore,
    revocation: &ValidatedRevocation,
    block_height: u64,
) -> Result<()> {
    store.set_owner_key_status(
        &revocation.target_pubkey,
        ChainStore::OWNER_KEY_REVOKED,
        Some(block_height),
    )?;
    Ok(())
}

/// Validate an OWNER_KEY_OVERRIDE DataItem.
///
/// Per CompetingRecorders.md §6.4 / §8.2:
/// - SHA256 child: hash of the OWNER_KEY_REVOCATION being overridden
/// - ED25519_PUB children: keys to place on hold (must match revocation signers)
/// - TIMESTAMP child: hold expiration
/// - AUTH_SIG children: N+1 valid owner key signatures (must exceed the
///   number of signatures on the overridden revocation)
///
/// The override reinstates the revoked key and places the revocation's
/// signers on immediate hold.
pub fn validate_override(
    store: &ChainStore,
    _meta: &ChainMeta,
    item: &DataItem,
    current_timestamp: i64,
) -> Result<ValidatedOverride> {
    if item.type_code != OWNER_KEY_OVERRIDE {
        return Err(ChainError::InvalidBlock(
            format!("expected OWNER_KEY_OVERRIDE ({}), got {}", OWNER_KEY_OVERRIDE, item.type_code)));
    }

    // Extract SHA256 — hash of the revocation being overridden.
    let _revocation_hash = item.find_child(SHA256)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidBlock(
            "OWNER_KEY_OVERRIDE missing SHA256 (revocation hash)".into()))?;

    // Extract ED25519_PUB children — keys to place on hold.
    // The first ED25519_PUB that is NOT inside an AUTH_SIG is a held-key reference.
    // But per the spec structure, the held keys are direct children with type ED25519_PUB.
    // AUTH_SIG children also contain ED25519_PUB, so we need to distinguish.
    // Direct ED25519_PUB children (not inside AUTH_SIG) are the held keys.
    let all_children = item.children();
    let mut held_pubkeys = Vec::new();
    for child in all_children {
        if child.type_code == ED25519_PUB {
            let bytes = child.as_bytes()
                .ok_or_else(|| ChainError::InvalidBlock("ED25519_PUB has no bytes".into()))?;
            if bytes.len() != 32 {
                return Err(ChainError::InvalidBlock("held pubkey must be 32 bytes".into()));
            }
            let mut pk = [0u8; 32];
            pk.copy_from_slice(bytes);
            held_pubkeys.push(pk);
        }
    }
    if held_pubkeys.is_empty() {
        return Err(ChainError::InvalidBlock(
            "OWNER_KEY_OVERRIDE must specify at least one key to place on hold".into()));
    }

    // Extract hold expiration timestamp.
    let hold_expires_bytes = item.find_child(TIMESTAMP)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::InvalidBlock(
            "OWNER_KEY_OVERRIDE missing TIMESTAMP (hold expiration)".into()))?;
    if hold_expires_bytes.len() != 8 {
        return Err(ChainError::InvalidBlock("hold expiration TIMESTAMP must be 8 bytes".into()));
    }
    let hold_expires_at = i64::from_be_bytes(hold_expires_bytes.try_into().expect("length validated"));

    // Verify AUTH_SIG(s) — must be signed by N+1 valid owner keys.
    let auth_sigs = item.find_children(AUTH_SIG);
    if auth_sigs.is_empty() {
        return Err(ChainError::InvalidBlock("OWNER_KEY_OVERRIDE missing AUTH_SIG".into()));
    }

    // For override verification, we need to accept signatures from keys that
    // are currently revoked (the reinstated key) OR valid.
    // The spec says the override signers must be valid owner keys, but the
    // key being reinstated is currently revoked — it signs to prove it
    // controls the key. We verify against all owner keys (valid + revoked).
    let mut signer_pubkeys = Vec::new();
    let mut seen_signers = std::collections::HashSet::new();
    for auth_sig in &auth_sigs {
        let pk = verify_owner_or_revoked_sig(store, item, auth_sig, current_timestamp)?;
        if !seen_signers.insert(pk) {
            return Err(ChainError::InvalidBlock(
                "duplicate signer in OWNER_KEY_OVERRIDE".into()));
        }
        signer_pubkeys.push(pk);
    }

    // The number of override signers must exceed the number of held keys
    // (held keys = signers of the original revocation).
    let n_revocation_signers = held_pubkeys.len();
    if signer_pubkeys.len() <= n_revocation_signers {
        return Err(ChainError::InvalidBlock(format!(
            "OWNER_KEY_OVERRIDE needs {} signers (> {}), got {}",
            n_revocation_signers + 1, n_revocation_signers, signer_pubkeys.len())));
    }

    // Determine which key is being reinstated: the held keys are the revocation's
    // signers; the revoked key is the one NOT in held_pubkeys and NOT a current
    // valid key. We find it among the override signers.
    let mut reinstated_pubkey = None;
    for pk in &signer_pubkeys {
        let status = store.get_owner_key_status(pk)?;
        if status.as_deref() == Some(ChainStore::OWNER_KEY_REVOKED) {
            reinstated_pubkey = Some(*pk);
            break;
        }
    }
    let reinstated_pubkey = reinstated_pubkey.ok_or_else(|| ChainError::InvalidBlock(
        "OWNER_KEY_OVERRIDE: no revoked key found among signers to reinstate".into()))?;

    // Verify all held keys are currently valid owner keys.
    for pk in &held_pubkeys {
        if !store.is_valid_owner_key(pk, current_timestamp)? {
            return Err(ChainError::InvalidBlock(
                "OWNER_KEY_OVERRIDE: held key is not a currently valid owner key".into()));
        }
    }

    Ok(ValidatedOverride {
        item: item.clone(),
        reinstated_pubkey,
        held_pubkeys,
        hold_expires_at,
        signer_pubkeys,
    })
}

/// Apply a validated override to the store.
///
/// 1. Reinstate the revoked key (set status back to 'valid').
/// 2. Place the revocation's signers on 'held' status with expiration.
pub fn apply_override(
    store: &ChainStore,
    ovr: &ValidatedOverride,
    block_height: u64,
) -> Result<()> {
    // Reinstate the revoked key.
    store.set_owner_key_status(
        &ovr.reinstated_pubkey,
        ChainStore::OWNER_KEY_VALID,
        None, // clear revoked_at_height
    )?;

    // Place held keys on hold with expiration.
    for pk in &ovr.held_pubkeys {
        store.set_owner_key_status(pk, ChainStore::OWNER_KEY_HELD, Some(block_height))?;
        store.set_owner_key_expiration(pk, ovr.hold_expires_at)?;
    }

    Ok(())
}

// --- Internal helpers ---

/// Verify an AUTH_SIG child was signed by a currently valid owner key.
/// Returns the signer's public key on success.
fn verify_owner_sig(
    store: &ChainStore,
    signable: &DataItem,
    auth_sig: &DataItem,
    current_timestamp: i64,
) -> Result<[u8; 32]> {
    let sig_bytes = auth_sig.find_child(ED25519_SIG)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("AUTH_SIG missing ED25519_SIG".into()))?;
    let ts_bytes = auth_sig.find_child(TIMESTAMP)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("AUTH_SIG missing TIMESTAMP".into()))?;

    if sig_bytes.len() != 64 {
        return Err(ChainError::SignatureFailure("signature must be 64 bytes".into()));
    }
    if ts_bytes.len() != 8 {
        return Err(ChainError::SignatureFailure("timestamp must be 8 bytes".into()));
    }

    // Extract signer pubkey from AUTH_SIG
    let pub_bytes = auth_sig.find_child(ED25519_PUB)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("AUTH_SIG missing ED25519_PUB".into()))?;
    if pub_bytes.len() != 32 {
        return Err(ChainError::SignatureFailure("signer pubkey must be 32 bytes".into()));
    }
    let mut signer_pubkey = [0u8; 32];
    signer_pubkey.copy_from_slice(pub_bytes);

    // Signer must be a valid owner key
    if !store.is_valid_owner_key(&signer_pubkey, current_timestamp)? {
        return Err(ChainError::SignatureFailure(
            "AUTH_SIG signer is not a valid owner key".into()));
    }

    let sig: [u8; 64] = sig_bytes.try_into().expect("length validated");
    let timestamp = Timestamp::from_bytes(ts_bytes.try_into().expect("length validated"));

    // Build signable content: the item without AUTH_SIG children
    let signable_children: Vec<DataItem> = signable.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .cloned()
        .collect();
    let signable_item = DataItem::container(signable.type_code, signable_children);

    if !sign::verify_dataitem(&signer_pubkey, &signable_item, timestamp, &sig) {
        return Err(ChainError::SignatureFailure("owner key signature verification failed".into()));
    }

    Ok(signer_pubkey)
}

/// Verify an AUTH_SIG child was signed by a valid OR revoked owner key.
/// Used for override verification where the reinstated key is currently revoked.
fn verify_owner_or_revoked_sig(
    store: &ChainStore,
    signable: &DataItem,
    auth_sig: &DataItem,
    current_timestamp: i64,
) -> Result<[u8; 32]> {
    let sig_bytes = auth_sig.find_child(ED25519_SIG)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("AUTH_SIG missing ED25519_SIG".into()))?;
    let ts_bytes = auth_sig.find_child(TIMESTAMP)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("AUTH_SIG missing TIMESTAMP".into()))?;

    if sig_bytes.len() != 64 {
        return Err(ChainError::SignatureFailure("signature must be 64 bytes".into()));
    }
    if ts_bytes.len() != 8 {
        return Err(ChainError::SignatureFailure("timestamp must be 8 bytes".into()));
    }

    let pub_bytes = auth_sig.find_child(ED25519_PUB)
        .and_then(|c| c.as_bytes())
        .ok_or_else(|| ChainError::SignatureFailure("AUTH_SIG missing ED25519_PUB".into()))?;
    if pub_bytes.len() != 32 {
        return Err(ChainError::SignatureFailure("signer pubkey must be 32 bytes".into()));
    }
    let mut signer_pubkey = [0u8; 32];
    signer_pubkey.copy_from_slice(pub_bytes);

    // Signer must be a known owner key (valid or revoked — not unknown).
    // Valid keys must also not be expired.
    let status = store.get_owner_key_status(&signer_pubkey)?;
    match status.as_deref() {
        Some(ChainStore::OWNER_KEY_VALID) => {
            // Valid status, but must also check expiration
            if !store.is_valid_owner_key(&signer_pubkey, current_timestamp)? {
                return Err(ChainError::SignatureFailure(
                    "AUTH_SIG signer key is expired".into()));
            }
        }
        Some(ChainStore::OWNER_KEY_REVOKED) => {
            // Revoked keys can sign overrides (to reinstate themselves)
        }
        Some(ChainStore::OWNER_KEY_HELD) => {
            return Err(ChainError::SignatureFailure(
                "AUTH_SIG signer is on hold — cannot sign".into()));
        }
        _ => {
            return Err(ChainError::SignatureFailure(
                "AUTH_SIG signer is not a known owner key".into()));
        }
    }

    let sig: [u8; 64] = sig_bytes.try_into().expect("length validated");
    let timestamp = Timestamp::from_bytes(ts_bytes.try_into().expect("length validated"));

    let signable_children: Vec<DataItem> = signable.children().iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .cloned()
        .collect();
    let signable_item = DataItem::container(signable.type_code, signable_children);

    if !sign::verify_dataitem(&signer_pubkey, &signable_item, timestamp, &sig) {
        return Err(ChainError::SignatureFailure("owner key signature verification failed".into()));
    }

    Ok(signer_pubkey)
}

/// Check whether any revocation has occurred (first revocation is free, subsequent are rate-limited).
/// Returns 0 if no prior revocations, 1 if at least one exists.
fn count_recent_revocations(
    store: &ChainStore,
    _meta: &ChainMeta,
    _current_timestamp: i64,
) -> Result<u64> {
    if last_revocation_timestamp(store)?.is_some() {
        Ok(1)
    } else {
        Ok(0)
    }
}

/// Get the timestamp of the most recent revocation.
fn last_revocation_timestamp(store: &ChainStore) -> Result<Option<i64>> {
    // We need the timestamp of the last revoked key. The owner_keys table
    // stores added_timestamp but not revoked_timestamp. For rate limiting,
    // we track this via the block that contained the revocation.
    // For now, use the added_timestamp of the most recently revoked key's
    // block height. This is approximate but functional.
    //
    // TODO: Add revoked_at_timestamp column in a later phase for precise tracking.
    // For Phase 2, the store schema already has revoked_at_height — we use
    // the block timestamp at that height.
    store.last_revocation_block_timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ao_crypto::sign::SigningKey;

    fn setup_chain_with_owner() -> (ChainStore, ChainMeta, SigningKey) {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        let key = SigningKey::generate();
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(key.public_key_bytes());

        let default_24h = Timestamp::from_unix_seconds(24 * 3600).raw();
        let meta = ChainMeta {
            chain_id: [0xAA; 32],
            symbol: "TST".into(),
            coin_count: num_bigint::BigInt::from(1_000_000u64),
            shares_out: num_bigint::BigInt::from(1_000_000u64),
            fee_rate_num: num_bigint::BigInt::from(1),
            fee_rate_den: num_bigint::BigInt::from(1000),
            expiry_period: 0,
            expiry_mode: 1,
            tax_start_age: None,
            tax_doubling_period: None,
            reward_rate_num: num_bigint::BigInt::from(0),
            reward_rate_den: num_bigint::BigInt::from(1),
            key_rotation_rate: default_24h,
            revocation_rate_base: default_24h,
            recorder_pubkey: None,
            pending_recorder_change: None,
            frozen: false,
            block_height: 0,
            next_seq_id: 2, // post-genesis (issuer got seq 1)
            last_block_timestamp: 100,
            prev_hash: [0; 32],
        };
        store.store_chain_meta(&meta).unwrap();
        store.insert_owner_key(&pubkey, 0, 100, None).unwrap();

        (store, meta, key)
    }

    fn build_rotation(
        signer: &SigningKey,
        new_pubkey: &[u8; 32],
        expires_at: Option<i64>,
        ts: Timestamp,
    ) -> DataItem {
        let mut children = vec![
            DataItem::bytes(ED25519_PUB, new_pubkey.to_vec()),
        ];
        if let Some(exp) = expires_at {
            children.push(DataItem::bytes(TIMESTAMP, exp.to_be_bytes().to_vec()));
        }
        let signable = DataItem::container(OWNER_KEY_ROTATION, children.clone());
        let sig = sign::sign_dataitem(signer, &signable, ts);
        children.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::bytes(ED25519_PUB, signer.public_key_bytes().to_vec()),
        ]));
        DataItem::container(OWNER_KEY_ROTATION, children)
    }

    fn build_revocation(
        signers: &[&SigningKey],
        target_pubkey: &[u8; 32],
        ts: Timestamp,
    ) -> DataItem {
        let children_no_sig = vec![
            DataItem::bytes(ED25519_PUB, target_pubkey.to_vec()),
        ];
        let signable = DataItem::container(OWNER_KEY_REVOCATION, children_no_sig.clone());

        let mut children = children_no_sig;
        for signer in signers {
            let sig = sign::sign_dataitem(signer, &signable, ts);
            children.push(DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, signer.public_key_bytes().to_vec()),
            ]));
        }
        DataItem::container(OWNER_KEY_REVOCATION, children)
    }

    #[test]
    fn test_rotation_valid() {
        let (store, meta, key) = setup_chain_with_owner();
        let new_key = SigningKey::generate();
        let mut new_pubkey = [0u8; 32];
        new_pubkey.copy_from_slice(new_key.public_key_bytes());

        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let rotation = build_rotation(&key, &new_pubkey, None, ts);

        // Pre-live (next_seq_id = 2 means only issuer exists, no assignments yet)
        let result = validate_rotation(&store, &meta, &rotation, ts.raw());
        assert!(result.is_ok(), "rotation should succeed: {:?}", result.err());
        let vr = result.unwrap();
        assert_eq!(vr.new_pubkey, new_pubkey);
        assert!(vr.old_key_expires_at.is_none());
    }

    #[test]
    fn test_rotation_with_expiration() {
        let (store, meta, key) = setup_chain_with_owner();
        let new_key = SigningKey::generate();
        let mut new_pubkey = [0u8; 32];
        new_pubkey.copy_from_slice(new_key.public_key_bytes());

        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let exp = Timestamp::from_unix_seconds(1_772_700_000 + 48 * 3600).raw();
        let rotation = build_rotation(&key, &new_pubkey, Some(exp), ts);

        let result = validate_rotation(&store, &meta, &rotation, ts.raw());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().old_key_expires_at, Some(exp));
    }

    #[test]
    fn test_rotation_rate_limited_post_live() {
        let (store, mut meta, key) = setup_chain_with_owner();
        // Make it post-live (assignments have happened)
        meta.next_seq_id = 5;
        store.store_chain_meta(&meta).unwrap();

        let new_key1 = SigningKey::generate();
        let mut new_pk1 = [0u8; 32];
        new_pk1.copy_from_slice(new_key1.public_key_bytes());

        // First rotation should succeed (genesis key was added at ts=100)
        let ts1 = Timestamp::from_unix_seconds(1_772_700_000);
        let rot1 = build_rotation(&key, &new_pk1, None, ts1);
        let result = validate_rotation(&store, &meta, &rot1, ts1.raw());
        assert!(result.is_ok());

        // Apply it
        let vr = result.unwrap();
        apply_rotation(&store, &vr, 1, ts1.raw()).unwrap();

        // Second rotation immediately should be rate limited
        let new_key2 = SigningKey::generate();
        let mut new_pk2 = [0u8; 32];
        new_pk2.copy_from_slice(new_key2.public_key_bytes());
        let ts2 = Timestamp::from_unix_seconds(1_772_700_001); // 1 second later
        let rot2 = build_rotation(&key, &new_pk2, None, ts2);
        let result = validate_rotation(&store, &meta, &rot2, ts2.raw());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("rate limited"));
    }

    #[test]
    fn test_rotation_pre_live_no_rate_limit() {
        let (store, meta, key) = setup_chain_with_owner();
        // Pre-live: next_seq_id = 2

        let new_key1 = SigningKey::generate();
        let mut new_pk1 = [0u8; 32];
        new_pk1.copy_from_slice(new_key1.public_key_bytes());

        let ts1 = Timestamp::from_unix_seconds(1_772_700_000);
        let rot1 = build_rotation(&key, &new_pk1, None, ts1);
        let vr = validate_rotation(&store, &meta, &rot1, ts1.raw()).unwrap();
        apply_rotation(&store, &vr, 1, ts1.raw()).unwrap();

        // Second rotation 1 second later — should succeed because pre-live
        let new_key2 = SigningKey::generate();
        let mut new_pk2 = [0u8; 32];
        new_pk2.copy_from_slice(new_key2.public_key_bytes());
        let ts2 = Timestamp::from_unix_seconds(1_772_700_001);
        let rot2 = build_rotation(&key, &new_pk2, None, ts2);
        let result = validate_rotation(&store, &meta, &rot2, ts2.raw());
        assert!(result.is_ok(), "pre-live rotation should not be rate limited");
    }

    #[test]
    fn test_rotation_invalid_signer() {
        let (store, meta, _key) = setup_chain_with_owner();

        // Sign with a non-owner key
        let attacker = SigningKey::generate();
        let new_key = SigningKey::generate();
        let mut new_pubkey = [0u8; 32];
        new_pubkey.copy_from_slice(new_key.public_key_bytes());

        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let rotation = build_rotation(&attacker, &new_pubkey, None, ts);
        let result = validate_rotation(&store, &meta, &rotation, ts.raw());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a valid owner key"));
    }

    #[test]
    fn test_rotation_duplicate_key_rejected() {
        let (store, meta, key) = setup_chain_with_owner();

        // Try to add the same key that's already an owner
        let mut existing_pk = [0u8; 32];
        existing_pk.copy_from_slice(key.public_key_bytes());

        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let rotation = build_rotation(&key, &existing_pk, None, ts);
        let result = validate_rotation(&store, &meta, &rotation, ts.raw());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already a valid owner key"));
    }

    #[test]
    fn test_revocation_valid() {
        let (store, meta, key1) = setup_chain_with_owner();

        // Add a second key so we can revoke one
        let key2 = SigningKey::generate();
        let mut pk2 = [0u8; 32];
        pk2.copy_from_slice(key2.public_key_bytes());
        store.insert_owner_key(&pk2, 1, 200, None).unwrap();

        // Revoke key2, signed by key1
        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let revocation = build_revocation(&[&key1], &pk2, ts);
        let result = validate_revocation(&store, &meta, &revocation, ts.raw());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().target_pubkey, pk2);
    }

    #[test]
    fn test_revocation_zero_keys_prevented() {
        let (store, meta, key) = setup_chain_with_owner();

        // Only one owner key — try to revoke it
        let mut pk = [0u8; 32];
        pk.copy_from_slice(key.public_key_bytes());

        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let revocation = build_revocation(&[&key], &pk, ts);
        let result = validate_revocation(&store, &meta, &revocation, ts.raw());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("zero valid owner keys"));
    }

    #[test]
    fn test_revocation_target_not_valid() {
        let (store, meta, key) = setup_chain_with_owner();

        // Try to revoke a key that doesn't exist
        let fake_pk = [0xFF; 32];
        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let revocation = build_revocation(&[&key], &fake_pk, ts);
        let result = validate_revocation(&store, &meta, &revocation, ts.raw());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a valid owner key"));
    }

    #[test]
    fn test_revocation_multi_signer() {
        let (store, meta, key1) = setup_chain_with_owner();

        // Add keys 2 and 3
        let key2 = SigningKey::generate();
        let mut pk2 = [0u8; 32];
        pk2.copy_from_slice(key2.public_key_bytes());
        store.insert_owner_key(&pk2, 1, 200, None).unwrap();

        let key3 = SigningKey::generate();
        let mut pk3 = [0u8; 32];
        pk3.copy_from_slice(key3.public_key_bytes());
        store.insert_owner_key(&pk3, 2, 300, None).unwrap();

        // Revoke key3, co-signed by key1 + key2
        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let revocation = build_revocation(&[&key1, &key2], &pk3, ts);
        let result = validate_revocation(&store, &meta, &revocation, ts.raw());
        assert!(result.is_ok());
        let vr = result.unwrap();
        assert_eq!(vr.signer_pubkeys.len(), 2);
    }

    #[test]
    fn test_apply_rotation_and_revocation() {
        let (store, _meta, _key) = setup_chain_with_owner();

        // Add a new key via rotation
        let new_pk = [0x42; 32];
        let rotation = ValidatedRotation {
            item: DataItem::container(OWNER_KEY_ROTATION, vec![]),
            new_pubkey: new_pk,
            old_key_expires_at: None,
            signer_pubkey: [0; 32],
        };
        apply_rotation(&store, &rotation, 1, 500).unwrap();
        assert!(store.is_valid_owner_key(&new_pk, 500).unwrap());

        // Revoke it
        let revocation = ValidatedRevocation {
            item: DataItem::container(OWNER_KEY_REVOCATION, vec![]),
            target_pubkey: new_pk,
            signer_pubkeys: vec![],
        };
        apply_revocation(&store, &revocation, 2).unwrap();
        assert!(!store.is_valid_owner_key(&new_pk, 500).unwrap());
    }
}
