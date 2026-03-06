use ao_types::dataitem::{DataItem, DataValue};
use ao_types::typecode;

use crate::hash;

/// Walk a DataItem tree and replace each separable item with a SHA256 item
/// containing the hash of that item's complete binary encoding.
///
/// Non-separable containers are recursed into. Non-separable leaf items are
/// returned unchanged.
pub fn substitute_separable(item: &DataItem) -> DataItem {
    if typecode::is_separable(item.type_code) {
        // Replace entire item with SHA256 hash of its encoding
        let encoded = item.to_bytes();
        let digest = hash::sha256(&encoded);
        DataItem::bytes(typecode::SHA256, digest.to_vec())
    } else {
        match &item.value {
            DataValue::Container(children) => {
                let new_children: Vec<DataItem> =
                    children.iter().map(substitute_separable).collect();
                DataItem::container(item.type_code, new_children)
            }
            _ => item.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ao_types::typecode::*;

    #[test]
    fn test_separable_substitution() {
        let note = DataItem::bytes(NOTE, b"Hello".to_vec());
        let note_encoded = note.to_bytes();
        let expected_hash = hash::sha256(&note_encoded);

        let result = substitute_separable(&note);
        assert_eq!(result.type_code, SHA256);
        if let DataValue::Bytes(data) = &result.value {
            assert_eq!(data.as_slice(), &expected_hash);
        } else {
            panic!("expected Bytes");
        }
    }

    #[test]
    fn test_inseparable_unchanged() {
        let key = DataItem::bytes(ED25519_PUB, vec![0xAA; 32]);
        let result = substitute_separable(&key);
        assert_eq!(result, key);
    }

    #[test]
    fn test_container_recurse() {
        let assignment = DataItem::container(ASSIGNMENT, vec![
            DataItem::vbc_value(LIST_SIZE, 1),
            DataItem::bytes(NOTE, b"test note".to_vec()), // separable
            DataItem::container(PARTICIPANT, vec![
                DataItem::bytes(ED25519_PUB, vec![0xBB; 32]),
            ]),
        ]);

        let result = substitute_separable(&assignment);
        if let DataValue::Container(children) = &result.value {
            assert_eq!(children.len(), 3);
            // First child unchanged (LIST_SIZE is inseparable)
            assert_eq!(children[0].type_code, LIST_SIZE);
            // Second child replaced with SHA256
            assert_eq!(children[1].type_code, SHA256);
            // Third child unchanged (PARTICIPANT container, recursed but no separable inside)
            assert_eq!(children[2].type_code, PARTICIPANT);
        } else {
            panic!("expected container");
        }
    }

    #[test]
    fn test_nested_separable_in_separable_container() {
        // VENDOR_PROFILE (36, separable) containing NOTE (32, separable)
        // The whole VENDOR_PROFILE gets replaced, not its children individually
        let vp = DataItem::container(VENDOR_PROFILE, vec![
            DataItem::bytes(NOTE, b"name".to_vec()),
        ]);
        let result = substitute_separable(&vp);
        // Should be replaced with a single SHA256
        assert_eq!(result.type_code, SHA256);
    }
}
