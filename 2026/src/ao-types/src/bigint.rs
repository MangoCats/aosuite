use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Zero;

use crate::vbc;

/// Error type for BigInt/Rational encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BigIntError {
    Vbc(vbc::VbcError),
    /// Not enough bytes for the declared magnitude.
    UnexpectedEnd,
    /// Redundant leading bytes in magnitude.
    NonMinimal,
    /// Rational denominator is zero or negative.
    InvalidDenominator,
    /// Rational total size doesn't match actual content.
    SizeMismatch,
}

impl core::fmt::Display for BigIntError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            BigIntError::Vbc(e) => write!(f, "VBC error in bigint: {}", e),
            BigIntError::UnexpectedEnd => write!(f, "unexpected end of bigint data"),
            BigIntError::NonMinimal => write!(f, "non-minimal bigint encoding"),
            BigIntError::InvalidDenominator => write!(f, "rational denominator must be positive"),
            BigIntError::SizeMismatch => write!(f, "rational size mismatch"),
        }
    }
}

impl From<vbc::VbcError> for BigIntError {
    fn from(e: vbc::VbcError) -> Self {
        BigIntError::Vbc(e)
    }
}

// --- BigInt encoding ---

/// Encode a BigInt as VBC byte count + two's-complement big-endian bytes.
pub fn encode_bigint(value: &BigInt, out: &mut Vec<u8>) {
    if value.is_zero() {
        vbc::encode_unsigned(0, out);
        return;
    }
    let bytes = value.to_signed_bytes_be();
    vbc::encode_unsigned(bytes.len() as u64, out);
    out.extend_from_slice(&bytes);
}

/// Decode a BigInt from `data` starting at `pos`.
/// Returns `(value, bytes_consumed)`.
pub fn decode_bigint(data: &[u8], pos: usize) -> Result<(BigInt, usize), BigIntError> {
    let (byte_count, vbc_len) = vbc::decode_unsigned(data, pos)?;
    let byte_count = byte_count as usize;
    let mag_start = pos + vbc_len;
    let mag_end = mag_start + byte_count;

    if mag_end > data.len() {
        return Err(BigIntError::UnexpectedEnd);
    }

    if byte_count == 0 {
        return Ok((BigInt::zero(), vbc_len));
    }

    let magnitude = &data[mag_start..mag_end];

    // Check minimality: no redundant leading bytes
    if byte_count > 1 {
        let first = magnitude[0];
        let second = magnitude[1];
        // Redundant 0x00 prefix (positive): first=0x00 and second < 0x80
        if first == 0x00 && second < 0x80 {
            return Err(BigIntError::NonMinimal);
        }
        // Redundant 0xFF prefix (negative): first=0xFF and second >= 0x80
        if first == 0xFF && second >= 0x80 {
            return Err(BigIntError::NonMinimal);
        }
    }

    let value = BigInt::from_signed_bytes_be(magnitude);
    Ok((value, vbc_len + byte_count))
}

// --- Rational encoding ---

/// Encode a BigRational as TotalSize + NumSize + Numerator + Denominator.
pub fn encode_rational(value: &BigRational, out: &mut Vec<u8>) {
    let num_bytes = if value.numer().is_zero() {
        vec![]
    } else {
        value.numer().to_signed_bytes_be()
    };
    let denom_bytes = value.denom().to_signed_bytes_be();

    // NumSize as unsigned VBC
    let mut num_size_vbc = Vec::new();
    vbc::encode_unsigned(num_bytes.len() as u64, &mut num_size_vbc);

    let total = num_size_vbc.len() + num_bytes.len() + denom_bytes.len();
    vbc::encode_unsigned(total as u64, out);
    out.extend_from_slice(&num_size_vbc);
    out.extend_from_slice(&num_bytes);
    out.extend_from_slice(&denom_bytes);
}

/// Decode a BigRational from `data` starting at `pos`.
/// Returns `(value, bytes_consumed)`.
pub fn decode_rational(data: &[u8], pos: usize) -> Result<(BigRational, usize), BigIntError> {
    let (total_size, total_vbc_len) = vbc::decode_unsigned(data, pos)?;
    let total_size = total_size as usize;
    let content_start = pos + total_vbc_len;

    if content_start + total_size > data.len() {
        return Err(BigIntError::UnexpectedEnd);
    }

    let (num_size, num_size_vbc_len) = vbc::decode_unsigned(data, content_start)?;
    let num_size = num_size as usize;

    let num_start = content_start + num_size_vbc_len;
    let num_end = num_start + num_size;
    let denom_size = total_size - num_size_vbc_len - num_size;

    if denom_size == 0 {
        return Err(BigIntError::InvalidDenominator);
    }

    let denom_start = num_end;
    let denom_end = denom_start + denom_size;

    if denom_end > data.len() {
        return Err(BigIntError::UnexpectedEnd);
    }

    let numer = if num_size == 0 {
        BigInt::zero()
    } else {
        BigInt::from_signed_bytes_be(&data[num_start..num_end])
    };

    let denom = BigInt::from_signed_bytes_be(&data[denom_start..denom_end]);
    if denom <= BigInt::zero() {
        return Err(BigIntError::InvalidDenominator);
    }

    // Verify total size consistency
    if num_size_vbc_len + num_size + denom_size != total_size {
        return Err(BigIntError::SizeMismatch);
    }

    Ok((BigRational::new(numer, denom), total_vbc_len + total_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
    use num_rational::BigRational;

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
    fn test_bigint_conformance() {
        let vectors: &[(&str, &str)] = &[
            ("0", "00"),
            ("1", "0101"),
            ("-1", "01ff"),
            ("127", "017f"),
            ("-127", "0181"),
            ("128", "020080"),
            ("-128", "0180"),
            ("-129", "02ff7f"),
            ("255", "0200ff"),
            ("-255", "02ff01"),
            ("256", "020100"),
            ("-256", "02ff00"),
            ("32767", "027fff"),
            ("-32768", "028000"),
            ("65535", "0300ffff"),
            ("18446744073709551616", "09010000000000000000"),        // 2^64
            ("77371252455336267181195264", "0b4000000000000000000000"), // ~2^86
        ];

        for &(value_str, expected_hex) in vectors {
            let value: BigInt = value_str.parse().unwrap();

            // Test encoding
            let mut buf = Vec::new();
            encode_bigint(&value, &mut buf);
            let got_hex = bytes_to_hex(&buf);
            assert_eq!(
                got_hex, expected_hex,
                "encode_bigint({}) = {}, expected {}",
                value_str, got_hex, expected_hex
            );

            // Test decoding
            let expected_bytes = hex_to_bytes(expected_hex);
            let (decoded, consumed) = decode_bigint(&expected_bytes, 0).unwrap();
            assert_eq!(
                decoded, value,
                "decode_bigint({}) = {}, expected {}",
                expected_hex, decoded, value
            );
            assert_eq!(consumed, expected_bytes.len());
        }
    }

    #[test]
    fn test_bigint_round_trip() {
        let values = [
            "0", "1", "-1", "255", "-256", "1000000",
            "-999999999999999999999999999",
            "77371252455336267181195264",
        ];
        for val_str in &values {
            let value: BigInt = val_str.parse().unwrap();
            let mut buf = Vec::new();
            encode_bigint(&value, &mut buf);
            let (decoded, consumed) = decode_bigint(&buf, 0).unwrap();
            assert_eq!(decoded, value, "round-trip failed for {}", val_str);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn test_rational_conformance() {
        let vectors: &[(&str, &str, &str)] = &[
            // (num, denom, expected_hex)
            ("1", "2", "03010102"),
            ("-3", "7", "0301fd07"),
            ("1", "1000000", "0501010f4240"),
            ("3", "10000000000", "07010302540be400"),
        ];

        for &(num_str, denom_str, expected_hex) in vectors {
            let numer: BigInt = num_str.parse().unwrap();
            let denom: BigInt = denom_str.parse().unwrap();
            let value = BigRational::new(numer.clone(), denom.clone());

            // Test encoding
            let mut buf = Vec::new();
            encode_rational(&value, &mut buf);
            let got_hex = bytes_to_hex(&buf);
            assert_eq!(
                got_hex, expected_hex,
                "encode_rational({}/{}) = {}, expected {}",
                num_str, denom_str, got_hex, expected_hex
            );

            // Test decoding
            let expected_bytes = hex_to_bytes(expected_hex);
            let (decoded, consumed) = decode_rational(&expected_bytes, 0).unwrap();
            assert_eq!(
                decoded, value,
                "decode_rational({}) = {}, expected {}/{}",
                expected_hex, decoded, num_str, denom_str
            );
            assert_eq!(consumed, expected_bytes.len());
        }
    }

    #[test]
    fn test_rational_round_trip() {
        let cases = [("1", "2"), ("-3", "7"), ("1", "1000000"), ("0", "1")];
        for (n, d) in &cases {
            let value = BigRational::new(n.parse().unwrap(), d.parse().unwrap());
            let mut buf = Vec::new();
            encode_rational(&value, &mut buf);
            let (decoded, consumed) = decode_rational(&buf, 0).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn test_rational_zero_denom_rejected() {
        // Manually craft bytes with zero-length denom
        // TotalSize=1, NumSize=0, no num bytes, denom would be 1 byte but...
        // Actually, zero denom can't happen with BigRational::new (it panics).
        // Test at decode level with crafted bytes: TotalSize=2, NumSize=1, num=[01], denom=[]
        // That's total=2, num_size_vbc=1byte, num=1byte, denom=0bytes → denom_size=0
        let data = hex_to_bytes("020101"); // total=2, num_size=1, num=[01], denom=nothing
        let result = decode_rational(&data, 0);
        assert_eq!(result, Err(BigIntError::InvalidDenominator));
    }
}
