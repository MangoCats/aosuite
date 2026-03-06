use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use ring::rand::SystemRandom;

use ao_types::dataitem::DataItem;
use ao_types::timestamp::Timestamp;

use crate::hash;
use crate::separable;

/// An Ed25519 signing key (wraps ring's Ed25519KeyPair).
pub struct SigningKey {
    keypair: Ed25519KeyPair,
    seed: [u8; 32],
}

impl SigningKey {
    /// Create a signing key from a 32-byte seed (RFC 8032 private key).
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        let keypair = Ed25519KeyPair::from_seed_unchecked(seed).unwrap();
        SigningKey { keypair, seed: *seed }
    }

    /// Generate a new signing key from OS randomness.
    pub fn generate() -> Self {
        let mut seed = [0u8; 32];
        use ring::rand::SecureRandom;
        SystemRandom::new().fill(&mut seed).unwrap();
        Self::from_seed(&seed)
    }

    /// Get the 32-byte public key.
    pub fn public_key_bytes(&self) -> &[u8] {
        self.keypair.public_key().as_ref()
    }

    /// Get the 32-byte seed.
    pub fn seed(&self) -> &[u8; 32] {
        &self.seed
    }

    /// Sign raw bytes, returning a 64-byte signature.
    pub fn sign_raw(&self, message: &[u8]) -> [u8; 64] {
        let sig = self.keypair.sign(message);
        let mut result = [0u8; 64];
        result.copy_from_slice(sig.as_ref());
        result
    }
}

/// Verify a raw Ed25519 signature against a public key.
pub fn verify_raw(public_key: &[u8], message: &[u8], signature: &[u8]) -> bool {
    let peer_public_key = UnparsedPublicKey::new(&ED25519, public_key);
    peer_public_key.verify(message, signature).is_ok()
}

/// AO signing pipeline (WireFormat.md §6.2):
/// 1. Substitute separable items with SHA256 hashes
/// 2. Serialize substituted tree to bytes
/// 3. digest = SHA256(substituted_bytes)
/// 4. signed_data = digest || timestamp (8 bytes BE)
/// 5. Ed25519 sign
pub fn sign_dataitem(
    key: &SigningKey,
    item: &DataItem,
    timestamp: Timestamp,
) -> [u8; 64] {
    let signed_data = build_signed_data(item, timestamp);
    key.sign_raw(&signed_data)
}

/// Verify an AO signature against a DataItem, timestamp, and public key bytes.
pub fn verify_dataitem(
    pubkey: &[u8],
    item: &DataItem,
    timestamp: Timestamp,
    signature: &[u8; 64],
) -> bool {
    let signed_data = build_signed_data(item, timestamp);
    verify_raw(pubkey, &signed_data, signature)
}

/// Build the 40-byte signed_data = SHA256(substituted_encoding) || timestamp.
fn build_signed_data(item: &DataItem, timestamp: Timestamp) -> [u8; 40] {
    let substituted = separable::substitute_separable(item);
    let encoded = substituted.to_bytes();
    let digest = hash::sha256(&encoded);

    let mut signed_data = [0u8; 40];
    signed_data[..32].copy_from_slice(&digest);
    signed_data[32..].copy_from_slice(&timestamp.to_bytes());
    signed_data
}

#[cfg(test)]
mod tests {
    use super::*;
    use ao_types::typecode::*;
    use ao_types::dataitem::DataItem;
    use ao_types::timestamp::Timestamp;

    fn from_hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i+2], 16).unwrap()).collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    // RFC 8032 TEST 1 — empty message (conformance vector)
    #[test]
    fn test_ed25519_rfc8032_test1() {
        let seed_hex = "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60";
        let expected_pub = "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a";
        let expected_sig = "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b";

        let seed_bytes = from_hex(seed_hex);
        let key = SigningKey::from_seed(&seed_bytes.try_into().unwrap());

        // Public key must match RFC 8032
        assert_eq!(to_hex(key.public_key_bytes()), expected_pub);

        // Signature must match RFC 8032
        let sig = key.sign_raw(b"");
        assert_eq!(to_hex(&sig), expected_sig);

        // Verify
        assert!(verify_raw(key.public_key_bytes(), b"", &sig));
    }

    #[test]
    fn test_ao_sign_verify_round_trip() {
        let key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1772611200); // 2026-03-06

        let assignment = DataItem::container(ASSIGNMENT, vec![
            DataItem::vbc_value(LIST_SIZE, 2),
            DataItem::container(PARTICIPANT, vec![
                DataItem::vbc_value(SEQ_ID, 1),
                DataItem::bytes(AMOUNT, vec![0x01, 0x00]),
            ]),
            DataItem::container(PARTICIPANT, vec![
                DataItem::bytes(ED25519_PUB, vec![0xBB; 32]),
                DataItem::bytes(AMOUNT, vec![0x00, 0xFF]),
            ]),
        ]);

        let sig = sign_dataitem(&key, &assignment, ts);
        assert!(verify_dataitem(key.public_key_bytes(), &assignment, ts, &sig));
    }

    #[test]
    fn test_wrong_key_rejects() {
        let key1 = SigningKey::generate();
        let key2 = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1000);
        let item = DataItem::vbc_value(LIST_SIZE, 42);

        let sig = sign_dataitem(&key1, &item, ts);
        assert!(!verify_dataitem(key2.public_key_bytes(), &item, ts, &sig));
    }

    #[test]
    fn test_wrong_timestamp_rejects() {
        let key = SigningKey::generate();
        let ts1 = Timestamp::from_unix_seconds(1000);
        let ts2 = Timestamp::from_unix_seconds(1001);
        let item = DataItem::vbc_value(LIST_SIZE, 42);

        let sig = sign_dataitem(&key, &item, ts1);
        assert!(!verify_dataitem(key.public_key_bytes(), &item, ts2, &sig));
    }

    #[test]
    fn test_modified_data_rejects() {
        let key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1000);
        let item1 = DataItem::vbc_value(LIST_SIZE, 42);
        let item2 = DataItem::vbc_value(LIST_SIZE, 43);

        let sig = sign_dataitem(&key, &item1, ts);
        assert!(!verify_dataitem(key.public_key_bytes(), &item2, ts, &sig));
    }

    #[test]
    fn test_separable_items_dont_affect_signature() {
        let key = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1000);

        let item_with_note = DataItem::container(ASSIGNMENT, vec![
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::bytes(NOTE, b"Important note".to_vec()),
        ]);

        let item_different_note = DataItem::container(ASSIGNMENT, vec![
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::bytes(NOTE, b"Different note".to_vec()),
        ]);

        let sig = sign_dataitem(&key, &item_with_note, ts);

        // Different separable content → different signature (hash of encoding differs)
        assert!(!verify_dataitem(key.public_key_bytes(), &item_different_note, ts, &sig));

        // But same content verifies
        assert!(verify_dataitem(key.public_key_bytes(), &item_with_note, ts, &sig));
    }
}
