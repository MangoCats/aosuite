use anyhow::{Result, bail};

use ao_types::dataitem::DataItem;
use ao_types::json as ao_json;
use ao_types::typecode;
use ao_crypto::hash;

/// Result of verifying a batch of blocks.
#[derive(Debug)]
pub struct VerificationResult {
    /// New rolled hash after processing all blocks.
    pub rolled_hash: [u8; 32],
    /// Height of the last verified block.
    pub last_height: u64,
    /// Number of blocks verified.
    pub count: u64,
}

/// Compute the rolled-up hash: SHA-256(prev_rolled_hash || block_hash).
/// This is the core accumulator that makes chain history tamper-evident.
pub fn update_rolled_hash(prev_rolled: &[u8; 32], block_hash: &[u8; 32]) -> [u8; 32] {
    let mut data = [0u8; 64];
    data[..32].copy_from_slice(prev_rolled);
    data[32..].copy_from_slice(block_hash);
    hash::sha256(&data)
}

/// Initialize the rolled hash from a genesis block (height 0).
/// The initial rolled hash is SHA-256 of the genesis block's hash.
pub fn genesis_rolled_hash(genesis_block_hash: &[u8; 32]) -> [u8; 32] {
    // For height 0, prev_rolled is all zeros
    update_rolled_hash(&[0u8; 32], genesis_block_hash)
}

/// Extract the block hash from a BLOCK DataItem.
///
/// Block structure: BLOCK { SHA256(block_hash), BLOCK_SIGNED { ... } }
/// The validator recomputes SHA-256 of the BLOCK_SIGNED encoding and
/// compares it against the embedded SHA256 item.
fn extract_and_verify_block_hash(block: &DataItem) -> Result<[u8; 32]> {
    if block.type_code != typecode::BLOCK {
        bail!("expected BLOCK ({}), got type code {}", typecode::BLOCK, block.type_code);
    }

    let children = block.children();
    if children.is_empty() {
        bail!("BLOCK has no children");
    }

    // Find SHA256 child (the claimed block hash)
    let claimed_hash_item = block.find_child(typecode::SHA256)
        .ok_or_else(|| anyhow::anyhow!("BLOCK missing SHA256 hash item"))?;
    let claimed_hash = claimed_hash_item.as_bytes()
        .ok_or_else(|| anyhow::anyhow!("SHA256 item has no data"))?;
    if claimed_hash.len() != 32 {
        bail!("SHA256 item has wrong length: {}", claimed_hash.len());
    }

    // Find BLOCK_SIGNED child
    let block_signed = block.find_child(typecode::BLOCK_SIGNED)
        .ok_or_else(|| anyhow::anyhow!("BLOCK missing BLOCK_SIGNED child"))?;

    // Recompute hash of BLOCK_SIGNED encoding
    let signed_bytes = block_signed.to_bytes();
    let computed_hash = hash::sha256(&signed_bytes);

    // Compare
    if computed_hash != claimed_hash {
        bail!(
            "block hash mismatch: claimed {} != computed {}",
            hex::encode(claimed_hash),
            hex::encode(computed_hash)
        );
    }

    let mut hash_arr = [0u8; 32];
    hash_arr.copy_from_slice(claimed_hash);
    Ok(hash_arr)
}

/// Verify a batch of blocks fetched as JSON from a recorder.
///
/// For each block:
/// 1. Deserialize JSON → DataItem
/// 2. Extract block hash, verify it matches SHA-256(BLOCK_SIGNED encoding)
/// 3. Update rolled hash
///
/// Returns the new rolled hash after all blocks, or an error describing
/// the first tampered block.
pub fn verify_block_batch(
    blocks_json: &[serde_json::Value],
    expected_start_height: u64,
    prev_rolled_hash: &[u8; 32],
) -> Result<VerificationResult> {
    if blocks_json.is_empty() {
        return Ok(VerificationResult {
            rolled_hash: *prev_rolled_hash,
            last_height: expected_start_height.saturating_sub(1),
            count: 0,
        });
    }

    let mut rolled = *prev_rolled_hash;
    let mut last_height = expected_start_height.saturating_sub(1);

    for (i, block_json) in blocks_json.iter().enumerate() {
        let height = expected_start_height + i as u64;

        let block = ao_json::from_json(block_json)
            .map_err(|e| anyhow::anyhow!("block {} JSON decode error: {}", height, e))?;

        let block_hash = extract_and_verify_block_hash(&block)
            .map_err(|e| anyhow::anyhow!("block {} verification failed: {}", height, e))?;

        rolled = update_rolled_hash(&rolled, &block_hash);
        last_height = height;
    }

    Ok(VerificationResult {
        rolled_hash: rolled,
        last_height,
        count: blocks_json.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rolled_hash_deterministic() {
        let h1 = [0x11; 32];
        let h2 = [0x22; 32];

        let r1 = update_rolled_hash(&[0u8; 32], &h1);
        let r2 = update_rolled_hash(&r1, &h2);

        // Same inputs produce same outputs
        let r1b = update_rolled_hash(&[0u8; 32], &h1);
        let r2b = update_rolled_hash(&r1b, &h2);
        assert_eq!(r1, r1b);
        assert_eq!(r2, r2b);

        // Different inputs produce different outputs
        let r_alt = update_rolled_hash(&[0u8; 32], &h2);
        assert_ne!(r1, r_alt);
    }

    #[test]
    fn test_genesis_rolled_hash() {
        let genesis_hash = [0xAA; 32];
        let rolled = genesis_rolled_hash(&genesis_hash);
        // Should be SHA-256([0; 32] || genesis_hash)
        let expected = update_rolled_hash(&[0u8; 32], &genesis_hash);
        assert_eq!(rolled, expected);
    }

    #[test]
    fn test_verify_empty_batch() {
        let prev = [0xCC; 32];
        let result = verify_block_batch(&[], 5, &prev).unwrap();
        assert_eq!(result.rolled_hash, prev);
        assert_eq!(result.count, 0);
    }

    /// Build a BLOCK DataItem with correct SHA256 of BLOCK_SIGNED encoding.
    fn make_test_block(block_contents_children: Vec<DataItem>) -> DataItem {
        let block_contents = DataItem::container(
            typecode::BLOCK_CONTENTS, block_contents_children,
        );
        let block_signed = DataItem::container(
            typecode::BLOCK_SIGNED, vec![block_contents],
        );

        let signed_bytes = block_signed.to_bytes();
        let block_hash = hash::sha256(&signed_bytes);

        DataItem::container(typecode::BLOCK, vec![
            DataItem::bytes(typecode::SHA256, block_hash.to_vec()),
            block_signed,
        ])
    }

    #[test]
    fn test_verify_single_block_roundtrip() {
        let ts = DataItem::bytes(typecode::TIMESTAMP, vec![0u8; 8]);
        let block = make_test_block(vec![ts]);

        let block_json = ao_json::to_json(&block);
        let prev_rolled = [0u8; 32];

        let result = verify_block_batch(&[block_json], 0, &prev_rolled).unwrap();
        assert_eq!(result.count, 1);
        assert_eq!(result.last_height, 0);
        assert_ne!(result.rolled_hash, prev_rolled);

        // Determinism: same block produces same rolled hash
        let block2 = make_test_block(vec![DataItem::bytes(typecode::TIMESTAMP, vec![0u8; 8])]);
        let block_json2 = ao_json::to_json(&block2);
        let result2 = verify_block_batch(&[block_json2], 0, &prev_rolled).unwrap();
        assert_eq!(result.rolled_hash, result2.rolled_hash);
    }

    #[test]
    fn test_verify_multi_block_chain() {
        let block0 = make_test_block(vec![
            DataItem::bytes(typecode::TIMESTAMP, vec![0u8; 8]),
        ]);
        let block1 = make_test_block(vec![
            DataItem::bytes(typecode::TIMESTAMP, vec![1u8; 8]),
        ]);
        let block2 = make_test_block(vec![
            DataItem::bytes(typecode::TIMESTAMP, vec![2u8; 8]),
        ]);

        let jsons: Vec<_> = [&block0, &block1, &block2]
            .iter().map(|b| ao_json::to_json(b)).collect();

        let prev = [0u8; 32];
        let result = verify_block_batch(&jsons, 0, &prev).unwrap();
        assert_eq!(result.count, 3);
        assert_eq!(result.last_height, 2);

        // Incremental verification matches batch
        let r0 = verify_block_batch(&jsons[..1], 0, &prev).unwrap();
        let r1 = verify_block_batch(&jsons[1..2], 1, &r0.rolled_hash).unwrap();
        let r2 = verify_block_batch(&jsons[2..3], 2, &r1.rolled_hash).unwrap();
        assert_eq!(result.rolled_hash, r2.rolled_hash);
    }

    #[test]
    fn test_verify_detects_tampered_hash() {
        let block = make_test_block(vec![
            DataItem::bytes(typecode::TIMESTAMP, vec![0u8; 8]),
        ]);

        // Tamper with the SHA256 hash inside the block JSON
        let mut json = ao_json::to_json(&block);
        if let Some(items) = json.get_mut("items").and_then(|v| v.as_array_mut()) {
            if let Some(hash_item) = items.first_mut() {
                hash_item.as_object_mut().unwrap().insert(
                    "value".to_string(),
                    serde_json::Value::String("ff".repeat(32)),
                );
            }
        }

        let prev = [0u8; 32];
        let err = verify_block_batch(&[json], 0, &prev);
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("hash mismatch"));
    }
}
