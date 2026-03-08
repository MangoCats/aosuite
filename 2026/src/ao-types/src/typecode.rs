/// Type code constants from WireFormat.md §3.
///
/// Core inseparable types (|code| 1–31)
pub const ED25519_PUB: i64 = 1;
pub const ED25519_SIG: i64 = 2;
pub const SHA256: i64 = 3;
pub const BLAKE3: i64 = 4;
pub const TIMESTAMP: i64 = 5;
pub const AMOUNT: i64 = 6;
pub const SEQ_ID: i64 = 7;
pub const ASSIGNMENT: i64 = 8;
pub const AUTHORIZATION: i64 = 9;
pub const PARTICIPANT: i64 = 10;
pub const BLOCK: i64 = 11;
pub const BLOCK_SIGNED: i64 = 12;
pub const BLOCK_CONTENTS: i64 = 13;
pub const PAGE: i64 = 14;
pub const GENESIS: i64 = 15;
pub const RECORDING_BID: i64 = 16;
pub const DEADLINE: i64 = 17;
pub const COIN_COUNT: i64 = 18;
pub const FEE_RATE: i64 = 19;
pub const EXPIRY_PERIOD: i64 = 20;
pub const CHAIN_SYMBOL: i64 = 21;
pub const PROTOCOL_VER: i64 = 22;
pub const SHARES_OUT: i64 = 23;
pub const PREV_HASH: i64 = 24;
pub const FIRST_SEQ: i64 = 25;
pub const SEQ_COUNT: i64 = 26;
pub const LIST_SIZE: i64 = 27;
pub const REFUTATION: i64 = 28;
pub const PAGE_INDEX: i64 = 29;
pub const AUTH_SIG: i64 = 30;

/// Negative type codes
pub const EXPIRY_MODE: i64 = -1;
pub const TAX_PARAMS: i64 = -2;

/// Inseparable types (continued, |code| 31)
pub const REFERRAL_FEE: i64 = 31;

/// Separable types (|code| 32–63)
pub const NOTE: i64 = 32;
pub const DATA_BLOB: i64 = 33;
pub const DESCRIPTION: i64 = 34;
pub const ICON: i64 = 35;
pub const VENDOR_PROFILE: i64 = 36;
pub const EXCHANGE_LISTING: i64 = 37;
pub const CREDENTIAL_REF: i64 = 38;
pub const CREDENTIAL_URL: i64 = 39;

/// Inseparable types, second band (|code| 64–95)
pub const VALIDATOR_ATTESTATION: i64 = 64;
pub const VALIDATED_HEIGHT: i64 = 65;
pub const ROLLED_HASH: i64 = 66;
pub const ANCHOR_REF: i64 = 67;
pub const ANCHOR_TIMESTAMP: i64 = 68;

/// CAA (Conditional Assignment Agreement) types — Phase 6, inseparable band (|code| 69–77)
pub const CAA: i64 = 69;
pub const CAA_COMPONENT: i64 = 70;
pub const CHAIN_REF: i64 = 71;
pub const ESCROW_DEADLINE: i64 = 72;
pub const CHAIN_ORDER: i64 = 73;
pub const RECORDING_PROOF: i64 = 74;
pub const CAA_HASH: i64 = 75;
pub const BLOCK_REF: i64 = 76;
pub const BLOCK_HEIGHT: i64 = 77;
/// Coordinator bond amount (VBC value) — anti-theft protection for earlier chains in ouroboros.
pub const COORDINATOR_BOND: i64 = 78;

/// How the data portion of a DataItem is sized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeCategory {
    /// Fixed N bytes, no size field.
    Fixed(usize),
    /// Unsigned VBC size prefix, then that many bytes.
    Variable,
    /// Data is itself a single unsigned VBC value, self-delimiting.
    VbcValue,
    /// Unsigned VBC size prefix, contents are child DataItems.
    Container,
}

/// Look up the size category for a known type code.
/// Returns `None` for unknown type codes.
pub fn size_category(code: i64) -> Option<SizeCategory> {
    match code {
        ED25519_PUB => Some(SizeCategory::Fixed(32)),
        ED25519_SIG => Some(SizeCategory::Fixed(64)),
        SHA256 => Some(SizeCategory::Fixed(32)),
        BLAKE3 => Some(SizeCategory::Fixed(32)),
        TIMESTAMP => Some(SizeCategory::Fixed(8)),
        DEADLINE => Some(SizeCategory::Fixed(8)),
        EXPIRY_PERIOD => Some(SizeCategory::Fixed(8)),
        PREV_HASH => Some(SizeCategory::Fixed(32)),

        ROLLED_HASH => Some(SizeCategory::Fixed(32)),
        ANCHOR_TIMESTAMP => Some(SizeCategory::Fixed(8)),
        CHAIN_REF => Some(SizeCategory::Fixed(32)),
        ESCROW_DEADLINE => Some(SizeCategory::Fixed(8)),
        CAA_HASH => Some(SizeCategory::Fixed(32)),

        AMOUNT | RECORDING_BID | COIN_COUNT | FEE_RATE |
        CHAIN_SYMBOL | SHARES_OUT | REFERRAL_FEE |
        NOTE | DATA_BLOB | DESCRIPTION | ICON |
        CREDENTIAL_URL | ANCHOR_REF |
        COORDINATOR_BOND => Some(SizeCategory::Variable),

        SEQ_ID | PROTOCOL_VER | FIRST_SEQ | SEQ_COUNT |
        LIST_SIZE | PAGE_INDEX | EXPIRY_MODE |
        VALIDATED_HEIGHT |
        CHAIN_ORDER | BLOCK_HEIGHT => Some(SizeCategory::VbcValue),

        ASSIGNMENT | AUTHORIZATION | PARTICIPANT |
        BLOCK | BLOCK_SIGNED | BLOCK_CONTENTS |
        PAGE | GENESIS | REFUTATION | AUTH_SIG |
        TAX_PARAMS | VENDOR_PROFILE | EXCHANGE_LISTING |
        CREDENTIAL_REF | VALIDATOR_ATTESTATION |
        CAA | CAA_COMPONENT | RECORDING_PROOF | BLOCK_REF => Some(SizeCategory::Container),

        _ => None,
    }
}

/// Check if a type code's item is separable: `|code| & 0x20 != 0`.
pub fn is_separable(code: i64) -> bool {
    (code.unsigned_abs() & 0x20) != 0
}

/// Get the human-readable name for a type code, or `None` if unknown.
pub fn type_name(code: i64) -> Option<&'static str> {
    match code {
        ED25519_PUB => Some("ED25519_PUB"),
        ED25519_SIG => Some("ED25519_SIG"),
        SHA256 => Some("SHA256"),
        BLAKE3 => Some("BLAKE3"),
        TIMESTAMP => Some("TIMESTAMP"),
        AMOUNT => Some("AMOUNT"),
        SEQ_ID => Some("SEQ_ID"),
        ASSIGNMENT => Some("ASSIGNMENT"),
        AUTHORIZATION => Some("AUTHORIZATION"),
        PARTICIPANT => Some("PARTICIPANT"),
        BLOCK => Some("BLOCK"),
        BLOCK_SIGNED => Some("BLOCK_SIGNED"),
        BLOCK_CONTENTS => Some("BLOCK_CONTENTS"),
        PAGE => Some("PAGE"),
        GENESIS => Some("GENESIS"),
        RECORDING_BID => Some("RECORDING_BID"),
        DEADLINE => Some("DEADLINE"),
        COIN_COUNT => Some("COIN_COUNT"),
        FEE_RATE => Some("FEE_RATE"),
        EXPIRY_PERIOD => Some("EXPIRY_PERIOD"),
        CHAIN_SYMBOL => Some("CHAIN_SYMBOL"),
        PROTOCOL_VER => Some("PROTOCOL_VER"),
        SHARES_OUT => Some("SHARES_OUT"),
        PREV_HASH => Some("PREV_HASH"),
        FIRST_SEQ => Some("FIRST_SEQ"),
        SEQ_COUNT => Some("SEQ_COUNT"),
        LIST_SIZE => Some("LIST_SIZE"),
        REFUTATION => Some("REFUTATION"),
        PAGE_INDEX => Some("PAGE_INDEX"),
        AUTH_SIG => Some("AUTH_SIG"),
        REFERRAL_FEE => Some("REFERRAL_FEE"),
        EXPIRY_MODE => Some("EXPIRY_MODE"),
        TAX_PARAMS => Some("TAX_PARAMS"),
        NOTE => Some("NOTE"),
        DATA_BLOB => Some("DATA_BLOB"),
        DESCRIPTION => Some("DESCRIPTION"),
        ICON => Some("ICON"),
        VENDOR_PROFILE => Some("VENDOR_PROFILE"),
        EXCHANGE_LISTING => Some("EXCHANGE_LISTING"),
        CREDENTIAL_REF => Some("CREDENTIAL_REF"),
        CREDENTIAL_URL => Some("CREDENTIAL_URL"),
        VALIDATOR_ATTESTATION => Some("VALIDATOR_ATTESTATION"),
        VALIDATED_HEIGHT => Some("VALIDATED_HEIGHT"),
        ROLLED_HASH => Some("ROLLED_HASH"),
        ANCHOR_REF => Some("ANCHOR_REF"),
        ANCHOR_TIMESTAMP => Some("ANCHOR_TIMESTAMP"),
        CAA => Some("CAA"),
        CAA_COMPONENT => Some("CAA_COMPONENT"),
        CHAIN_REF => Some("CHAIN_REF"),
        ESCROW_DEADLINE => Some("ESCROW_DEADLINE"),
        CHAIN_ORDER => Some("CHAIN_ORDER"),
        RECORDING_PROOF => Some("RECORDING_PROOF"),
        CAA_HASH => Some("CAA_HASH"),
        BLOCK_REF => Some("BLOCK_REF"),
        BLOCK_HEIGHT => Some("BLOCK_HEIGHT"),
        COORDINATOR_BOND => Some("COORDINATOR_BOND"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_separability() {
        // Inseparable: |code| 1-31
        assert!(!is_separable(1));   // ED25519_PUB
        assert!(!is_separable(8));   // ASSIGNMENT
        assert!(!is_separable(31));  // last inseparable
        assert!(!is_separable(-1));  // EXPIRY_MODE
        assert!(!is_separable(-2));  // TAX_PARAMS

        // Separable: |code| 32-63
        assert!(is_separable(32));   // NOTE
        assert!(is_separable(33));   // DATA_BLOB
        assert!(is_separable(34));   // DESCRIPTION
        assert!(is_separable(35));   // ICON
        assert!(is_separable(36));   // VENDOR_PROFILE
        assert!(is_separable(63));   // end of first separable band

        // Credential refs are separable
        assert!(is_separable(CREDENTIAL_REF)); // 38
        assert!(is_separable(CREDENTIAL_URL)); // 39

        // Next inseparable band: 64-95 (validator + CAA types)
        assert!(!is_separable(VALIDATOR_ATTESTATION)); // 64
        assert!(!is_separable(VALIDATED_HEIGHT));       // 65
        assert!(!is_separable(ROLLED_HASH));             // 66
        assert!(!is_separable(ANCHOR_REF));              // 67
        assert!(!is_separable(ANCHOR_TIMESTAMP));        // 68
        assert!(!is_separable(CAA));                     // 69
        assert!(!is_separable(CAA_COMPONENT));           // 70
        assert!(!is_separable(CHAIN_REF));               // 71
        assert!(!is_separable(ESCROW_DEADLINE));         // 72
        assert!(!is_separable(CHAIN_ORDER));             // 73
        assert!(!is_separable(RECORDING_PROOF));         // 74
        assert!(!is_separable(CAA_HASH));                // 75
        assert!(!is_separable(BLOCK_REF));               // 76
        assert!(!is_separable(BLOCK_HEIGHT));            // 77
        assert!(!is_separable(95));

        // Next separable band: 96-127
        assert!(is_separable(96));
        assert!(is_separable(127));
    }

    #[test]
    fn test_all_codes_have_categories() {
        let all_codes = [
            ED25519_PUB, ED25519_SIG, SHA256, BLAKE3, TIMESTAMP,
            AMOUNT, SEQ_ID, ASSIGNMENT, AUTHORIZATION, PARTICIPANT,
            BLOCK, BLOCK_SIGNED, BLOCK_CONTENTS, PAGE, GENESIS,
            RECORDING_BID, DEADLINE, COIN_COUNT, FEE_RATE, EXPIRY_PERIOD,
            CHAIN_SYMBOL, PROTOCOL_VER, SHARES_OUT, PREV_HASH,
            FIRST_SEQ, SEQ_COUNT, LIST_SIZE, REFUTATION, PAGE_INDEX,
            AUTH_SIG, REFERRAL_FEE, EXPIRY_MODE, TAX_PARAMS,
            NOTE, DATA_BLOB, DESCRIPTION, ICON, VENDOR_PROFILE,
            EXCHANGE_LISTING, CREDENTIAL_REF, CREDENTIAL_URL,
            VALIDATOR_ATTESTATION, VALIDATED_HEIGHT, ROLLED_HASH,
            ANCHOR_REF, ANCHOR_TIMESTAMP,
            CAA, CAA_COMPONENT, CHAIN_REF, ESCROW_DEADLINE,
            CHAIN_ORDER, RECORDING_PROOF, CAA_HASH, BLOCK_REF, BLOCK_HEIGHT,
            COORDINATOR_BOND,
        ];
        for code in all_codes {
            assert!(
                size_category(code).is_some(),
                "missing size category for code {}",
                code
            );
            assert!(
                type_name(code).is_some(),
                "missing type name for code {}",
                code
            );
        }
    }
}
