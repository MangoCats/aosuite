use crate::dataitem::{DataItem, DataValue};
use crate::typecode;

/// Serialize a DataItem to its canonical JSON representation (WireFormat.md §7).
///
/// Rules:
/// - Type name and numeric code are both present.
/// - Fixed-size items use `"value"` (hex string for bytes).
/// - Variable-size items use `"value"` (hex string for bytes).
/// - VBC-value items use `"value"` (decimal integer).
/// - Containers use `"items"` array preserving child order.
pub fn to_json(item: &DataItem) -> serde_json::Value {
    let code = item.type_code;
    let name = typecode::type_name(code).unwrap_or("UNKNOWN");

    let mut obj = serde_json::Map::new();
    obj.insert("type".to_string(), serde_json::Value::String(name.to_string()));
    obj.insert("code".to_string(), serde_json::Value::Number(code.into()));

    match &item.value {
        DataValue::Bytes(data) => {
            let hex: String = data.iter().map(|b| format!("{:02x}", b)).collect();
            obj.insert("value".to_string(), serde_json::Value::String(hex));
        }
        DataValue::VbcValue(v) => {
            obj.insert(
                "value".to_string(),
                serde_json::Value::Number((*v).into()),
            );
        }
        DataValue::Container(children) => {
            let items: Vec<serde_json::Value> = children.iter().map(to_json).collect();
            obj.insert("items".to_string(), serde_json::Value::Array(items));
        }
    }

    serde_json::Value::Object(obj)
}

/// Deserialize a DataItem from its canonical JSON representation.
pub fn from_json(json: &serde_json::Value) -> Result<DataItem, String> {
    let obj = json.as_object().ok_or("expected JSON object")?;

    let code = obj
        .get("code")
        .and_then(|v| v.as_i64())
        .ok_or("missing or invalid 'code'")?;

    let cat = typecode::size_category(code)
        .ok_or_else(|| format!("unknown type code {}", code))?;

    match cat {
        typecode::SizeCategory::Fixed(_) | typecode::SizeCategory::Variable => {
            let hex = obj
                .get("value")
                .and_then(|v| v.as_str())
                .ok_or("missing 'value' for bytes item")?;
            let bytes = hex_decode(hex).map_err(|e| format!("invalid hex: {}", e))?;
            Ok(DataItem::bytes(code, bytes))
        }
        typecode::SizeCategory::VbcValue => {
            let value = obj
                .get("value")
                .and_then(|v| v.as_u64())
                .ok_or("missing or invalid 'value' for VBC item")?;
            Ok(DataItem::vbc_value(code, value))
        }
        typecode::SizeCategory::Container => {
            let items_json = obj
                .get("items")
                .and_then(|v| v.as_array())
                .ok_or("missing 'items' for container")?;
            let children: Result<Vec<DataItem>, String> =
                items_json.iter().map(from_json).collect();
            Ok(DataItem::container(code, children?))
        }
    }
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("odd-length hex string".to_string());
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typecode::*;

    #[test]
    fn test_json_round_trip_bytes() {
        let key = DataItem::bytes(ED25519_PUB, vec![0xAA; 32]);
        let json = to_json(&key);
        let decoded = from_json(&json).unwrap();
        assert_eq!(decoded, key);

        // Check JSON structure
        assert_eq!(json["type"], "ED25519_PUB");
        assert_eq!(json["code"], 1);
        assert!(json["value"].as_str().unwrap().len() == 64); // 32 bytes = 64 hex chars
    }

    #[test]
    fn test_json_round_trip_vbc() {
        let seq = DataItem::vbc_value(SEQ_ID, 42);
        let json = to_json(&seq);
        let decoded = from_json(&json).unwrap();
        assert_eq!(decoded, seq);
        assert_eq!(json["value"], 42);
    }

    #[test]
    fn test_json_round_trip_container() {
        let participant = DataItem::container(PARTICIPANT, vec![
            DataItem::vbc_value(SEQ_ID, 1),
            DataItem::bytes(AMOUNT, vec![0x01, 0xFF]),
        ]);
        let json = to_json(&participant);
        let decoded = from_json(&json).unwrap();
        assert_eq!(decoded, participant);

        assert_eq!(json["type"], "PARTICIPANT");
        assert!(json["items"].as_array().unwrap().len() == 2);
    }

    #[test]
    fn test_json_binary_round_trip() {
        // JSON → binary → JSON must produce identical bytes
        let original = DataItem::container(ASSIGNMENT, vec![
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

        // Original → binary → decoded
        let binary = original.to_bytes();
        let from_binary = DataItem::from_bytes(&binary).unwrap();
        assert_eq!(from_binary, original);

        // Original → JSON → decoded
        let json = to_json(&original);
        let from_json_item = from_json(&json).unwrap();
        assert_eq!(from_json_item, original);

        // JSON-decoded → binary must match original binary
        let binary2 = from_json_item.to_bytes();
        assert_eq!(binary2, binary);
    }
}
