use ao_types::dataitem::DataItem;
use ao_types::timestamp::Timestamp;
use ao_types::typecode::*;
use ao_crypto::sign::{self, SigningKey};

/// Build a signed RECORDER_IDENTITY DataItem per CompetingRecorders.md §10.1.
///
/// Structure:
///   RECORDER_IDENTITY (134, container)
///   ├── ED25519_PUB (1): recorder public key
///   ├── RECORDER_URL (136): service URL
///   ├── DESCRIPTION_INSEP (143): human-readable name/description
///   ├── AUTH_SIG (30): self-signature
///   └── TIMESTAMP (5): publication timestamp
pub fn build_recorder_identity(
    key: &SigningKey,
    url: &str,
    name: &str,
    ts: Timestamp,
) -> DataItem {

    // Build the signable content (everything except AUTH_SIG).
    let signable_children = vec![
        DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
        DataItem::bytes(RECORDER_URL, url.as_bytes().to_vec()),
        DataItem::bytes(DESCRIPTION_INSEP, name.as_bytes().to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
    ];
    let signable = DataItem::container(RECORDER_IDENTITY, signable_children.clone());

    // Sign the container.
    let sig = sign::sign_dataitem(key, &signable, ts);

    // Rebuild with AUTH_SIG included.
    let mut all_children = signable_children;
    all_children.push(DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
    ]));

    DataItem::container(RECORDER_IDENTITY, all_children)
}

/// Verify a RECORDER_IDENTITY DataItem's self-signature.
///
/// Extracts ED25519_PUB and AUTH_SIG, rebuilds the signable content
/// (all children except AUTH_SIG), and verifies the signature.
pub fn verify_recorder_identity(identity: &DataItem) -> bool {
    let children = identity.children();
    if children.is_empty() {
        return false;
    }

    // Extract public key.
    let pubkey = match identity.find_child(ED25519_PUB) {
        Some(item) => match item.as_bytes() {
            Some(b) if b.len() == 32 => b,
            _ => return false,
        },
        None => return false,
    };

    // Extract AUTH_SIG container.
    let auth_sig = match identity.find_child(AUTH_SIG) {
        Some(item) => item,
        None => return false,
    };

    // Extract signature and timestamp from AUTH_SIG.
    let sig_bytes = match auth_sig.find_child(ED25519_SIG) {
        Some(item) => match item.as_bytes() {
            Some(b) if b.len() == 64 => b,
            _ => return false,
        },
        None => return false,
    };
    let ts_bytes = match auth_sig.find_child(TIMESTAMP) {
        Some(item) => match item.as_bytes() {
            Some(b) if b.len() == 8 => b,
            _ => return false,
        },
        None => return false,
    };

    let mut sig = [0u8; 64];
    sig.copy_from_slice(sig_bytes);
    let ts = Timestamp::from_bytes(ts_bytes.try_into().unwrap_or([0u8; 8]));

    // Rebuild the signable content: all children except AUTH_SIG.
    let signable_children: Vec<DataItem> = children
        .iter()
        .filter(|c| c.type_code != AUTH_SIG)
        .cloned()
        .collect();
    let signable = DataItem::container(RECORDER_IDENTITY, signable_children);

    sign::verify_dataitem(pubkey, &signable, ts, &sig)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_and_verify_identity() {
        let key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1_772_611_200);
        let identity = build_recorder_identity(
            &key,
            "https://recorder.example.com",
            "Test Recorder",
            ts,
        );

        // Verify structure.
        assert_eq!(identity.type_code, RECORDER_IDENTITY);
        let children = identity.children();
        assert_eq!(children.len(), 5); // PUB, URL, DESC, TIMESTAMP, AUTH_SIG

        // Check public key matches.
        let pub_child = identity.find_child(ED25519_PUB).unwrap();
        assert_eq!(pub_child.as_bytes().unwrap(), key.public_key_bytes());

        // Check URL.
        let url_child = identity.find_child(RECORDER_URL).unwrap();
        assert_eq!(url_child.as_bytes().unwrap(), b"https://recorder.example.com");

        // Check description.
        let desc_child = identity.find_child(DESCRIPTION_INSEP).unwrap();
        assert_eq!(desc_child.as_bytes().unwrap(), b"Test Recorder");

        // Verify self-signature.
        assert!(verify_recorder_identity(&identity));
    }

    #[test]
    fn test_tampered_identity_fails_verification() {
        let key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1_772_611_200);
        let identity = build_recorder_identity(&key, "https://example.com", "Legit", ts);

        // Rebuild with different URL but same signature — should fail.
        let mut tampered_children: Vec<DataItem> = identity.children().to_vec();
        // Replace URL child (index 1).
        tampered_children[1] = DataItem::bytes(RECORDER_URL, b"https://evil.com".to_vec());
        let tampered = DataItem::container(RECORDER_IDENTITY, tampered_children);

        assert!(!verify_recorder_identity(&tampered));
    }

    #[test]
    fn test_wrong_key_fails_verification() {
        let key1 = SigningKey::generate();
        let key2 = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1_772_611_200);
        let identity = build_recorder_identity(&key1, "https://example.com", "Test", ts);

        // Replace pubkey with key2's but keep key1's signature.
        let mut swapped_children: Vec<DataItem> = identity.children().to_vec();
        swapped_children[0] = DataItem::bytes(ED25519_PUB, key2.public_key_bytes().to_vec());
        let swapped = DataItem::container(RECORDER_IDENTITY, swapped_children);

        assert!(!verify_recorder_identity(&swapped));
    }

    #[test]
    fn test_missing_auth_sig_fails() {
        let key = SigningKey::generate();
        // Build without AUTH_SIG.
        let incomplete = DataItem::container(RECORDER_IDENTITY, vec![
            DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://example.com".to_vec()),
            DataItem::bytes(DESCRIPTION_INSEP, b"No sig".to_vec()),
        ]);

        assert!(!verify_recorder_identity(&incomplete));
    }

    #[test]
    fn test_identity_inseparable() {
        use ao_types::typecode::is_separable;
        // All RECORDER_IDENTITY type codes are in inseparable band 4.
        assert!(!is_separable(RECORDER_IDENTITY));
        assert!(!is_separable(RECORDER_URL));
        assert!(!is_separable(DESCRIPTION_INSEP));
    }
}
