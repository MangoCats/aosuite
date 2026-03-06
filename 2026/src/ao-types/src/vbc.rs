/// Maximum bytes a VBC encoding can occupy.
const MAX_VBC_BYTES: usize = 10;

/// Error type for VBC encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VbcError {
    /// Input buffer was empty or ended mid-value.
    UnexpectedEnd,
    /// Encoding exceeds the 10-byte maximum.
    Overflow,
    /// Negative zero (wire value 1) in signed VBC.
    NegativeZero,
}

impl core::fmt::Display for VbcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VbcError::UnexpectedEnd => write!(f, "unexpected end of VBC data"),
            VbcError::Overflow => write!(f, "VBC value overflow (>10 bytes)"),
            VbcError::NegativeZero => write!(f, "negative zero is invalid in signed VBC"),
        }
    }
}

// --- Unsigned VBC ---

/// Encode an unsigned 64-bit value as VBC bytes, appended to `out`.
/// Returns the number of bytes written.
pub fn encode_unsigned(value: u64, out: &mut Vec<u8>) -> usize {
    let mut v = value;
    let start = out.len();
    loop {
        let mut byte = (v & 0x7F) as u8;
        v >>= 7;
        if v > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
    out.len() - start
}

/// Decode an unsigned VBC value from `data` starting at `pos`.
/// Returns `(value, bytes_consumed)`.
pub fn decode_unsigned(data: &[u8], pos: usize) -> Result<(u64, usize), VbcError> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut i = pos;
    loop {
        if i >= data.len() {
            return Err(VbcError::UnexpectedEnd);
        }
        if i - pos >= MAX_VBC_BYTES {
            return Err(VbcError::Overflow);
        }
        let byte = data[i];
        let bits = (byte & 0x7F) as u64;
        // Check for overflow before shifting
        if shift >= 64 || (shift > 0 && bits > (u64::MAX >> shift)) {
            return Err(VbcError::Overflow);
        }
        value |= bits << shift;
        i += 1;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    Ok((value, i - pos))
}

// --- Signed VBC ---

/// Encode a signed 64-bit value as VBC bytes, appended to `out`.
/// Returns the number of bytes written.
///
/// Mapping: n >= 0 → wire = n << 1; n < 0 → wire = ((-n) << 1) | 1.
pub fn encode_signed(value: i64, out: &mut Vec<u8>) -> usize {
    let wire: u64 = if value >= 0 {
        (value as u64) << 1
    } else {
        // -i64::MIN would overflow; handle by computing magnitude directly
        let mag = (value as i128).unsigned_abs() as u64;
        (mag << 1) | 1
    };
    encode_unsigned(wire, out)
}

/// Decode a signed VBC value from `data` starting at `pos`.
/// Returns `(value, bytes_consumed)`.
pub fn decode_signed(data: &[u8], pos: usize) -> Result<(i64, usize), VbcError> {
    let (wire, consumed) = decode_unsigned(data, pos)?;
    if wire == 1 {
        return Err(VbcError::NegativeZero);
    }
    let magnitude = wire >> 1;
    if magnitude > i64::MAX as u64 {
        return Err(VbcError::Overflow);
    }
    let value = if wire & 1 == 0 {
        magnitude as i64
    } else {
        -(magnitude as i64)
    };
    Ok((value, consumed))
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

    // Conformance test vectors from vectors.json
    #[test]
    fn test_signed_vbc_conformance() {
        let vectors: &[(i64, u64, &str)] = &[
            (0, 0, "00"),
            (1, 2, "02"),
            (-1, 3, "03"),
            (2, 4, "04"),
            (-2, 5, "05"),
            (3, 6, "06"),
            (-3, 7, "07"),
            (10, 20, "14"),
            (-10, 21, "15"),
            (31, 62, "3e"),
            (-31, 63, "3f"),
            (32, 64, "40"),
            (-32, 65, "41"),
            (63, 126, "7e"),
            (-63, 127, "7f"),
            (64, 128, "8001"),
            (-64, 129, "8101"),
            (-65, 131, "8301"),
            (100, 200, "c801"),
            (-100, 201, "c901"),
            (127, 254, "fe01"),
            (128, 256, "8002"),
            (-128, 257, "8102"),
            (255, 510, "fe03"),
            (-255, 511, "ff03"),
            (256, 512, "8004"),
            (1000, 2000, "d00f"),
            (-1000, 2001, "d10f"),
            (8191, 16382, "fe7f"),
            (-8191, 16383, "ff7f"),
            (8192, 16384, "808001"),
            (-8192, 16385, "818001"),
            (1000000, 2000000, "80897a"),
            (-1000000, 2000001, "81897a"),
        ];

        for &(value, wire, expected_hex) in vectors {
            // Test encoding
            let mut buf = Vec::new();
            encode_signed(value, &mut buf);
            let got_hex = bytes_to_hex(&buf);
            assert_eq!(
                got_hex, expected_hex,
                "encode_signed({}) = {} (wire {}), expected {}",
                value, got_hex, wire, expected_hex
            );

            // Verify wire value
            let mut wire_buf = Vec::new();
            encode_unsigned(wire, &mut wire_buf);
            assert_eq!(
                wire_buf, buf,
                "wire value mismatch for signed {}",
                value
            );

            // Test decoding
            let expected_bytes = hex_to_bytes(expected_hex);
            let (decoded, consumed) = decode_signed(&expected_bytes, 0).unwrap();
            assert_eq!(decoded, value, "decode_signed({}) = {}", expected_hex, decoded);
            assert_eq!(consumed, expected_bytes.len());
        }
    }

    #[test]
    fn test_unsigned_vbc_conformance() {
        let vectors: &[(u64, &str)] = &[
            (0, "00"),
            (1, "01"),
            (2, "02"),
            (63, "3f"),
            (64, "40"),
            (127, "7f"),
            (128, "8001"),
            (255, "ff01"),
            (256, "8002"),
            (1000, "e807"),
            (16383, "ff7f"),
            (16384, "808001"),
        ];

        for &(value, expected_hex) in vectors {
            let mut buf = Vec::new();
            encode_unsigned(value, &mut buf);
            let got_hex = bytes_to_hex(&buf);
            assert_eq!(
                got_hex, expected_hex,
                "encode_unsigned({}) = {}, expected {}",
                value, got_hex, expected_hex
            );

            let expected_bytes = hex_to_bytes(expected_hex);
            let (decoded, consumed) = decode_unsigned(&expected_bytes, 0).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(consumed, expected_bytes.len());
        }
    }

    #[test]
    fn test_negative_zero_rejected() {
        // Wire value 1 = negative zero, must be rejected
        let data = hex_to_bytes("01");
        let result = decode_signed(&data, 0);
        assert_eq!(result, Err(VbcError::NegativeZero));
    }

    #[test]
    fn test_signed_round_trip_boundaries() {
        let boundary_values = [
            0i64,
            1, -1,
            63, -63,
            64, -64,
            127, -127, 128, -128,
            255, -255, 256, -256,
            8191, -8191, 8192, -8192,
            i64::MAX, -(i64::MAX),
            i32::MAX as i64, i32::MIN as i64,
        ];
        for &value in &boundary_values {
            let mut buf = Vec::new();
            encode_signed(value, &mut buf);
            let (decoded, consumed) = decode_signed(&buf, 0).unwrap();
            assert_eq!(decoded, value, "round-trip failed for {}", value);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn test_unsigned_round_trip_boundaries() {
        let boundary_values = [
            0u64, 1, 63, 64, 127, 128, 255, 256,
            16383, 16384, u32::MAX as u64, u64::MAX >> 1,
        ];
        for &value in &boundary_values {
            let mut buf = Vec::new();
            encode_unsigned(value, &mut buf);
            let (decoded, consumed) = decode_unsigned(&buf, 0).unwrap();
            assert_eq!(decoded, value, "round-trip failed for {}", value);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn test_decode_at_offset() {
        // Encode two values and decode from offset
        let mut buf = Vec::new();
        let len1 = encode_signed(42, &mut buf);
        encode_signed(-99, &mut buf);

        let (v1, c1) = decode_signed(&buf, 0).unwrap();
        assert_eq!(v1, 42);
        assert_eq!(c1, len1);

        let (v2, _c2) = decode_signed(&buf, len1).unwrap();
        assert_eq!(v2, -99);
    }

    #[test]
    fn test_unexpected_end() {
        // A byte with continuation bit set, but no following byte
        let data = vec![0x80];
        assert_eq!(decode_unsigned(&data, 0), Err(VbcError::UnexpectedEnd));
        assert_eq!(decode_signed(&data, 0), Err(VbcError::UnexpectedEnd));
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(decode_unsigned(&[], 0), Err(VbcError::UnexpectedEnd));
        assert_eq!(decode_signed(&[], 0), Err(VbcError::UnexpectedEnd));
    }
}
