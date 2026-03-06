use crate::typecode::{self, SizeCategory};
use crate::vbc;

/// Error type for DataItem encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataItemError {
    Vbc(vbc::VbcError),
    UnexpectedEnd,
    UnknownTypeCode(i64),
    /// Fixed-size item has wrong data length.
    WrongFixedSize { code: i64, expected: usize, got: usize },
    /// Container has leftover bytes after parsing children.
    TrailingBytes { code: i64, leftover: usize },
}

impl core::fmt::Display for DataItemError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DataItemError::Vbc(e) => write!(f, "VBC error: {}", e),
            DataItemError::UnexpectedEnd => write!(f, "unexpected end of data"),
            DataItemError::UnknownTypeCode(c) => write!(f, "unknown type code {}", c),
            DataItemError::WrongFixedSize { code, expected, got } =>
                write!(f, "type code {} expects {} bytes, got {}", code, expected, got),
            DataItemError::TrailingBytes { code, leftover } =>
                write!(f, "container {} has {} trailing bytes", code, leftover),
        }
    }
}

impl From<vbc::VbcError> for DataItemError {
    fn from(e: vbc::VbcError) -> Self {
        DataItemError::Vbc(e)
    }
}

/// The data payload of a DataItem, determined by its type code's size category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataValue {
    /// Fixed-size or variable-size raw bytes.
    Bytes(Vec<u8>),
    /// Self-delimiting unsigned VBC value.
    VbcValue(u64),
    /// Container holding child DataItems.
    Container(Vec<DataItem>),
}

/// A single DataItem: type code + payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataItem {
    pub type_code: i64,
    pub value: DataValue,
}

impl DataItem {
    /// Create a fixed-size or variable-size bytes item.
    pub fn bytes(type_code: i64, data: Vec<u8>) -> Self {
        DataItem { type_code, value: DataValue::Bytes(data) }
    }

    /// Create a VBC-value item.
    pub fn vbc_value(type_code: i64, value: u64) -> Self {
        DataItem { type_code, value: DataValue::VbcValue(value) }
    }

    /// Create a container item.
    pub fn container(type_code: i64, children: Vec<DataItem>) -> Self {
        DataItem { type_code, value: DataValue::Container(children) }
    }

    /// Encode this DataItem to binary, appending to `out`.
    pub fn encode(&self, out: &mut Vec<u8>) {
        vbc::encode_signed(self.type_code, out);

        match &self.value {
            DataValue::Bytes(data) => {
                let cat = typecode::size_category(self.type_code);
                match cat {
                    Some(SizeCategory::Fixed(_)) => {
                        // No size prefix for fixed
                        out.extend_from_slice(data);
                    }
                    _ => {
                        // Variable: size prefix + data
                        vbc::encode_unsigned(data.len() as u64, out);
                        out.extend_from_slice(data);
                    }
                }
            }
            DataValue::VbcValue(v) => {
                vbc::encode_unsigned(*v, out);
            }
            DataValue::Container(children) => {
                // Encode children to a temp buffer to get size
                let mut child_buf = Vec::new();
                for child in children {
                    child.encode(&mut child_buf);
                }
                vbc::encode_unsigned(child_buf.len() as u64, out);
                out.extend_from_slice(&child_buf);
            }
        }
    }

    /// Encode this DataItem and return the bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.encode(&mut buf);
        buf
    }

    /// Decode a DataItem from `data` starting at `pos`.
    /// Returns `(item, bytes_consumed)`.
    pub fn decode(data: &[u8], pos: usize) -> Result<(Self, usize), DataItemError> {
        let (type_code, tc_len) = vbc::decode_signed(data, pos)?;
        let mut offset = pos + tc_len;

        let cat = typecode::size_category(type_code)
            .ok_or(DataItemError::UnknownTypeCode(type_code))?;

        let item = match cat {
            SizeCategory::Fixed(n) => {
                if offset + n > data.len() {
                    return Err(DataItemError::UnexpectedEnd);
                }
                let bytes = data[offset..offset + n].to_vec();
                offset += n;
                DataItem::bytes(type_code, bytes)
            }
            SizeCategory::Variable => {
                let (size, size_len) = vbc::decode_unsigned(data, offset)?;
                offset += size_len;
                let size = size as usize;
                if offset + size > data.len() {
                    return Err(DataItemError::UnexpectedEnd);
                }
                let bytes = data[offset..offset + size].to_vec();
                offset += size;
                DataItem::bytes(type_code, bytes)
            }
            SizeCategory::VbcValue => {
                let (value, vbc_len) = vbc::decode_unsigned(data, offset)?;
                offset += vbc_len;
                DataItem::vbc_value(type_code, value)
            }
            SizeCategory::Container => {
                let (size, size_len) = vbc::decode_unsigned(data, offset)?;
                offset += size_len;
                let size = size as usize;
                if offset + size > data.len() {
                    return Err(DataItemError::UnexpectedEnd);
                }
                let container_end = offset + size;
                let mut children = Vec::new();
                while offset < container_end {
                    let (child, child_len) = DataItem::decode(data, offset)?;
                    offset += child_len;
                    children.push(child);
                }
                if offset != container_end {
                    return Err(DataItemError::TrailingBytes {
                        code: type_code,
                        leftover: offset - container_end,
                    });
                }
                DataItem::container(type_code, children)
            }
        };

        Ok((item, offset - pos))
    }

    /// Get children if this is a container, or empty slice otherwise.
    pub fn children(&self) -> &[DataItem] {
        match &self.value {
            DataValue::Container(children) => children,
            _ => &[],
        }
    }

    /// Find the first child with the given type code.
    pub fn find_child(&self, type_code: i64) -> Option<&DataItem> {
        self.children().iter().find(|c| c.type_code == type_code)
    }

    /// Find all children with the given type code.
    pub fn find_children(&self, type_code: i64) -> Vec<&DataItem> {
        self.children().iter().filter(|c| c.type_code == type_code).collect()
    }

    /// Get the VBC value if this is a VbcValue item.
    pub fn as_vbc_value(&self) -> Option<u64> {
        match &self.value {
            DataValue::VbcValue(v) => Some(*v),
            _ => None,
        }
    }

    /// Get the bytes if this is a Bytes item.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match &self.value {
            DataValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    /// Decode a DataItem from a byte slice (convenience wrapper).
    pub fn from_bytes(data: &[u8]) -> Result<Self, DataItemError> {
        let (item, consumed) = Self::decode(data, 0)?;
        if consumed != data.len() {
            return Err(DataItemError::TrailingBytes {
                code: item.type_code,
                leftover: data.len() - consumed,
            });
        }
        Ok(item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typecode::*;

    #[test]
    fn test_fixed_size_round_trip() {
        let key = DataItem::bytes(ED25519_PUB, vec![0xAA; 32]);
        let encoded = key.to_bytes();
        // Type code 1 = VBC 0x02, then 32 raw bytes, no size prefix
        assert_eq!(encoded.len(), 1 + 32); // type code (1 byte for small value) + 32 data
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, key);
    }

    #[test]
    fn test_variable_size_round_trip() {
        let note = DataItem::bytes(NOTE, b"Hello, AO!".to_vec());
        let encoded = note.to_bytes();
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, note);
    }

    #[test]
    fn test_vbc_value_round_trip() {
        let seq = DataItem::vbc_value(SEQ_ID, 42);
        let encoded = seq.to_bytes();
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, seq);
    }

    #[test]
    fn test_container_round_trip() {
        let participant = DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, vec![0x11; 32]),
            DataItem::bytes(AMOUNT, vec![0x01, 0x00]),  // BigInt: 256
        ]);
        let encoded = participant.to_bytes();
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, participant);
    }

    #[test]
    fn test_nested_container_round_trip() {
        let assignment = DataItem::container(ASSIGNMENT, vec![
            DataItem::vbc_value(LIST_SIZE, 2),
            DataItem::container(PARTICIPANT, vec![
                DataItem::vbc_value(SEQ_ID, 1),
                DataItem::bytes(AMOUNT, vec![0x01, 0xFF]),
            ]),
            DataItem::container(PARTICIPANT, vec![
                DataItem::bytes(ED25519_PUB, vec![0xBB; 32]),
                DataItem::bytes(AMOUNT, vec![0x01, 0xFE]),
            ]),
        ]);
        let encoded = assignment.to_bytes();
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, assignment);
    }

    #[test]
    fn test_timestamp_item() {
        let ts = DataItem::bytes(TIMESTAMP, 189000000i64.to_be_bytes().to_vec());
        let encoded = ts.to_bytes();
        // Type code 5 = signed VBC wire 10 = 0x0a, then 8 raw bytes
        assert_eq!(encoded.len(), 1 + 8);
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, ts);
    }

    #[test]
    fn test_negative_type_code() {
        let mode = DataItem::vbc_value(EXPIRY_MODE, 1);
        let encoded = mode.to_bytes();
        // EXPIRY_MODE = -1, wire = 3, hex 03
        assert_eq!(encoded[0], 0x03);
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, mode);
    }

    #[test]
    fn test_empty_container() {
        let empty = DataItem::container(TAX_PARAMS, vec![]);
        let encoded = empty.to_bytes();
        let decoded = DataItem::from_bytes(&encoded).unwrap();
        assert_eq!(decoded, empty);
    }

    #[test]
    fn test_unknown_type_code_rejected() {
        // Type code 99 is unknown
        let mut data = Vec::new();
        vbc::encode_signed(99, &mut data);
        data.push(0x00); // some trailing byte
        let result = DataItem::decode(&data, 0);
        assert!(matches!(result, Err(DataItemError::UnknownTypeCode(99))));
    }
}
