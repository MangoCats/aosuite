use sha2::{Sha256, Digest};

/// Compute SHA2-256 hash of input data. Returns 32 bytes.
pub fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Compute BLAKE3 hash of input data. Returns 32 bytes.
pub fn blake3(data: &[u8]) -> [u8; 32] {
    blake3::hash(data).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    fn from_hex(s: &str) -> Vec<u8> {
        (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i+2], 16).unwrap()).collect()
    }

    // Conformance vectors from vectors.json
    #[test]
    fn test_sha256_conformance() {
        // Empty input
        assert_eq!(
            hex(&sha256(&[])),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );

        // "abc" — NIST FIPS 180-4
        assert_eq!(
            hex(&sha256(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );

        // "Assign Onward"
        assert_eq!(
            hex(&sha256(b"Assign Onward")),
            "94305f099f3291fac25073818585021f160a57bb996bd08f4b46cde825c7d53c"
        );
    }

    #[test]
    fn test_sha256_hex_input() {
        // Verify hex input matches
        let input = from_hex("616263");
        assert_eq!(
            hex(&sha256(&input)),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn test_blake3_basic() {
        // BLAKE3 reference: empty input
        let h = blake3(&[]);
        assert_eq!(h.len(), 32);
        // BLAKE3 official test vector for empty input
        assert_eq!(
            hex(&h),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }
}
