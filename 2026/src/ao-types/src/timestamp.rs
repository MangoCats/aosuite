/// AO timestamp multiplier: Unix seconds × 189,000,000 (~5.29 ns resolution).
pub const AO_MULTIPLIER: i64 = 189_000_000;

/// Maximum Unix seconds representable without i64 overflow: ~year 3517.
/// Well beyond the 2126 design horizon.
pub const MAX_UNIX_SECONDS: i64 = i64::MAX / AO_MULTIPLIER; // 48,802,498,078

/// Minimum Unix seconds representable (negative = pre-epoch, ~year -1577).
/// Supports birthdates, historical dates, and pre-1970 phenomena.
pub const MIN_UNIX_SECONDS: i64 = i64::MIN / AO_MULTIPLIER; // -48,802,498,078

/// AO timestamp: Unix seconds × 189,000,000, stored as 8-byte big-endian signed integer.
///
/// Signed i64 is used deliberately:
/// - Supports pre-epoch dates (birthdates, historical events)
/// - Overflows at ~year 3517, well beyond the 2126 design horizon
/// - Matches the wire format spec: "8-byte big-endian signed integer"
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(pub i64);

impl Timestamp {
    /// Create a timestamp from Unix seconds.
    ///
    /// Panics if seconds is outside [`MIN_UNIX_SECONDS`]..=[`MAX_UNIX_SECONDS`] (~year -1577 to ~3517).
    /// For user-supplied input, prefer [`try_from_unix_seconds`].
    pub fn from_unix_seconds(seconds: i64) -> Self {
        Timestamp(seconds.checked_mul(AO_MULTIPLIER).expect("timestamp overflow"))
    }

    /// Fallible version of [`from_unix_seconds`]. Returns `None` on overflow.
    pub fn try_from_unix_seconds(seconds: i64) -> Option<Self> {
        seconds.checked_mul(AO_MULTIPLIER).map(Timestamp)
    }

    /// Create a timestamp from a raw AO timestamp value.
    pub fn from_raw(raw: i64) -> Self {
        Timestamp(raw)
    }

    /// Get the raw AO timestamp value.
    pub fn raw(self) -> i64 {
        self.0
    }

    /// Encode as 8-byte big-endian.
    pub fn to_bytes(self) -> [u8; 8] {
        self.0.to_be_bytes()
    }

    /// Decode from 8-byte big-endian.
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Timestamp(i64::from_be_bytes(bytes))
    }

    /// Decode from a slice at `pos`, consuming 8 bytes.
    pub fn decode(data: &[u8], pos: usize) -> Result<(Self, usize), TimestampError> {
        if pos + 8 > data.len() {
            return Err(TimestampError::UnexpectedEnd);
        }
        let bytes: [u8; 8] = data[pos..pos + 8].try_into().unwrap();
        Ok((Self::from_bytes(bytes), 8))
    }

    /// Encode to `out`, appending 8 bytes.
    pub fn encode(self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_bytes());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimestampError {
    UnexpectedEnd,
}

impl core::fmt::Display for TimestampError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TimestampError::UnexpectedEnd => write!(f, "unexpected end of timestamp data"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bytes(hex: &str) -> Vec<u8> {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect()
    }

    fn bytes_to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }

    #[test]
    fn test_timestamp_conformance() {
        let vectors: &[(i64, i64, &str)] = &[
            // (unix_seconds, ao_timestamp, hex)
            (0, 0, "0000000000000000"),
            (1704067200, 322068700800000000, "047837c6e874a000"),  // 2024-01-01
            (1767225600, 334005638400000000, "04a2a05bc5cf4000"),  // 2026-01-01
            (1772611200, 335023516800000000, "04a63e1d0e442000"),  // 2026-03-06
            (1, 189000000, "000000000b43e940"),                     // 1 second
        ];

        for &(unix_seconds, ao_ts, expected_hex) in vectors {
            let ts = Timestamp::from_unix_seconds(unix_seconds);
            assert_eq!(ts.raw(), ao_ts, "AO timestamp for unix {} = {}, expected {}", unix_seconds, ts.raw(), ao_ts);

            let encoded = ts.to_bytes();
            let got_hex = bytes_to_hex(&encoded);
            assert_eq!(got_hex, expected_hex, "hex for unix {} = {}, expected {}", unix_seconds, got_hex, expected_hex);

            // Round-trip
            let decoded_bytes = hex_to_bytes(expected_hex);
            let (decoded, consumed) = Timestamp::decode(&decoded_bytes, 0).unwrap();
            assert_eq!(decoded, ts);
            assert_eq!(consumed, 8);
        }
    }

    #[test]
    fn test_timestamp_ordering() {
        let t1 = Timestamp::from_unix_seconds(1000);
        let t2 = Timestamp::from_unix_seconds(1001);
        assert!(t1 < t2);
    }

    #[test]
    fn test_timestamp_decode_short() {
        let data = [0u8; 7];
        assert_eq!(Timestamp::decode(&data, 0), Err(TimestampError::UnexpectedEnd));
    }

    #[test]
    fn test_year_2126_is_representable() {
        // 2126-01-01T00:00:00Z = Unix 4924300800
        let unix_2126 = 4_924_300_800i64;
        let ts = Timestamp::from_unix_seconds(unix_2126);
        assert!(ts.raw() > 0, "year 2126 must be representable as positive i64");
        // Round-trip through bytes
        let decoded = Timestamp::from_bytes(ts.to_bytes());
        assert_eq!(decoded, ts);
    }

    #[test]
    fn test_pre_epoch_negative_timestamps() {
        // 1960-01-01 = Unix -315619200
        let ts = Timestamp::from_unix_seconds(-315_619_200);
        assert!(ts.raw() < 0, "pre-epoch timestamps must be negative");
        let decoded = Timestamp::from_bytes(ts.to_bytes());
        assert_eq!(decoded, ts);
    }

    #[test]
    fn test_try_from_unix_seconds_overflow() {
        assert!(Timestamp::try_from_unix_seconds(MAX_UNIX_SECONDS).is_some());
        assert!(Timestamp::try_from_unix_seconds(MAX_UNIX_SECONDS + 1).is_none());
        assert!(Timestamp::try_from_unix_seconds(MIN_UNIX_SECONDS).is_some());
        assert!(Timestamp::try_from_unix_seconds(MIN_UNIX_SECONDS - 1).is_none());
    }
}
