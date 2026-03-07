use num_bigint::BigInt;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_crypto::hash;
use ao_crypto::sign;

use crate::error::{ChainError, Result};
use crate::store::{ChainMeta, ChainStore, Utxo, UtxoStatus};

/// Extract chain ID (SHA256 hash) from a genesis DataItem without touching the store.
/// Returns the 32-byte chain ID, or an error if the genesis is malformed.
pub fn compute_chain_id(genesis: &DataItem) -> Result<[u8; 32]> {
    if genesis.type_code != GENESIS {
        return Err(ChainError::InvalidGenesis(
            format!("expected GENESIS ({}), got {}", GENESIS, genesis.type_code)));
    }
    let chain_hash_item = genesis.find_child(SHA256)
        .ok_or_else(|| ChainError::InvalidGenesis("missing SHA256 (chain ID)".into()))?;
    let chain_hash_bytes = chain_hash_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidGenesis("SHA256 has no bytes".into()))?;
    if chain_hash_bytes.len() != 32 {
        return Err(ChainError::InvalidGenesis("chain ID hash must be 32 bytes".into()));
    }
    let mut chain_id = [0u8; 32];
    chain_id.copy_from_slice(chain_hash_bytes);
    Ok(chain_id)
}

/// Parse a genesis block DataItem and initialize the chain store.
pub fn load_genesis(store: &ChainStore, genesis: &DataItem) -> Result<ChainMeta> {
    if genesis.type_code != GENESIS {
        return Err(ChainError::InvalidGenesis(
            format!("expected GENESIS ({}), got {}", GENESIS, genesis.type_code)));
    }

    let children = genesis.children();
    if children.is_empty() {
        return Err(ChainError::InvalidGenesis("empty genesis block".into()));
    }

    // Extract required fields
    let protocol_ver = find_vbc(genesis, PROTOCOL_VER, "PROTOCOL_VER")?;
    if protocol_ver != 1 {
        return Err(ChainError::InvalidGenesis(
            format!("unsupported protocol version {}", protocol_ver)));
    }

    let symbol = find_string(genesis, CHAIN_SYMBOL, "CHAIN_SYMBOL")?;
    let coin_count = find_bigint(genesis, COIN_COUNT, "COIN_COUNT")?;
    let shares_out = find_bigint(genesis, SHARES_OUT, "SHARES_OUT")?;
    let fee_rate = find_rational(genesis, FEE_RATE, "FEE_RATE")?;
    let expiry_period_bytes = find_bytes(genesis, EXPIRY_PERIOD, "EXPIRY_PERIOD")?;
    let expiry_mode = find_vbc(genesis, EXPIRY_MODE, "EXPIRY_MODE")?;

    if expiry_period_bytes.len() != 8 {
        return Err(ChainError::InvalidGenesis("EXPIRY_PERIOD must be 8 bytes".into()));
    }
    let expiry_period = i64::from_be_bytes(expiry_period_bytes.try_into().expect("length validated above"));

    // Tax params (optional, required for mode 2)
    let (tax_start_age, tax_doubling_period) = if expiry_mode == 2 {
        let tax_params = genesis.find_child(TAX_PARAMS)
            .ok_or_else(|| ChainError::InvalidGenesis("mode 2 requires TAX_PARAMS".into()))?;
        let timestamps = tax_params.find_children(TIMESTAMP);
        if timestamps.len() != 2 {
            return Err(ChainError::InvalidGenesis(
                "TAX_PARAMS must contain exactly 2 TIMESTAMP items".into()));
        }
        let start = parse_timestamp_bytes(timestamps[0])?;
        let doubling = parse_timestamp_bytes(timestamps[1])?;
        (Some(start), Some(doubling))
    } else {
        (None, None)
    };

    // Find issuer participant
    let participant = genesis.find_child(PARTICIPANT)
        .ok_or_else(|| ChainError::InvalidGenesis("missing PARTICIPANT".into()))?;
    let issuer_pub = find_bytes(participant, ED25519_PUB, "issuer ED25519_PUB")?;
    let issuer_amount = find_bigint(participant, AMOUNT, "issuer AMOUNT")?;

    if issuer_pub.len() != 32 {
        return Err(ChainError::InvalidGenesis("issuer public key must be 32 bytes".into()));
    }
    if issuer_amount != shares_out {
        return Err(ChainError::InvalidGenesis(
            "issuer amount must equal SHARES_OUT".into()));
    }

    // Find and verify AUTH_SIG
    let auth_sig = genesis.find_child(AUTH_SIG)
        .ok_or_else(|| ChainError::InvalidGenesis("missing AUTH_SIG".into()))?;
    let sig_bytes = find_bytes(auth_sig, ED25519_SIG, "AUTH_SIG ED25519_SIG")?;
    let sig_ts_bytes = find_bytes(auth_sig, TIMESTAMP, "AUTH_SIG TIMESTAMP")?;

    if sig_bytes.len() != 64 {
        return Err(ChainError::InvalidGenesis("signature must be 64 bytes".into()));
    }
    if sig_ts_bytes.len() != 8 {
        return Err(ChainError::InvalidGenesis("signature timestamp must be 8 bytes".into()));
    }

    let sig_timestamp = Timestamp::from_bytes(sig_ts_bytes.try_into().expect("length validated above"));

    // The signed content is everything in the genesis except AUTH_SIG and SHA256
    // Per WireFormat.md §6.1, the issuer signs the genesis content
    // We need to reconstruct the signable content (the GENESIS minus AUTH_SIG and SHA256)
    // Actually, per §6.2, the signature is over the content with separable substitution.
    // For genesis, the "assignment" equivalent is the genesis content before AUTH_SIG.
    // Let's build the signable items:
    let signable_children: Vec<DataItem> = children.iter()
        .filter(|c| c.type_code != AUTH_SIG && c.type_code != SHA256)
        .cloned()
        .collect();
    let signable = DataItem::container(GENESIS, signable_children);

    let sig_array: [u8; 64] = sig_bytes.try_into().expect("length validated above");
    let pubkey_array: [u8; 32] = issuer_pub.clone().try_into().expect("length validated above");
    if !sign::verify_dataitem(&pubkey_array, &signable, sig_timestamp, &sig_array) {
        return Err(ChainError::InvalidGenesis("genesis signature verification failed".into()));
    }

    // Verify chain ID hash
    let chain_hash_item = genesis.find_child(SHA256)
        .ok_or_else(|| ChainError::InvalidGenesis("missing SHA256 (chain ID)".into()))?;
    let chain_hash_bytes = chain_hash_item.as_bytes()
        .ok_or_else(|| ChainError::InvalidGenesis("SHA256 has no bytes".into()))?;
    if chain_hash_bytes.len() != 32 {
        return Err(ChainError::InvalidGenesis("chain ID hash must be 32 bytes".into()));
    }

    // Per WireFormat.md §6.1: chain ID = SHA256 of all child encodings except SHA256 itself.
    let mut content_bytes = Vec::new();
    for child in children {
        if child.type_code != SHA256 {
            child.encode(&mut content_bytes);
        }
    }
    let computed_hash = hash::sha256(&content_bytes);
    if computed_hash != chain_hash_bytes {
        return Err(ChainError::InvalidGenesis("chain ID hash mismatch".into()));
    }

    let mut chain_id = [0u8; 32];
    chain_id.copy_from_slice(chain_hash_bytes);

    let genesis_timestamp = sig_timestamp.raw();

    // Build chain metadata
    let meta = ChainMeta {
        chain_id,
        symbol,
        coin_count,
        shares_out: shares_out.clone(),
        fee_rate_num: fee_rate.0,
        fee_rate_den: fee_rate.1,
        expiry_period,
        expiry_mode,
        tax_start_age,
        tax_doubling_period,
        block_height: 0,
        next_seq_id: 2, // seq 1 is assigned to the issuer
        last_block_timestamp: genesis_timestamp,
        prev_hash: chain_id, // genesis hash is the "prev_hash" for block 1
    };

    // Initialize store
    store.init_schema()?;
    store.store_chain_meta(&meta)?;

    // Store genesis block data
    let genesis_bytes = genesis.to_bytes();
    store.store_block(0, genesis_timestamp, &chain_id, &genesis_bytes)?;

    // Create issuer UTXO with seq_id = 1
    let mut pubkey_arr = [0u8; 32];
    pubkey_arr.copy_from_slice(&issuer_pub);
    store.insert_utxo(&Utxo {
        seq_id: 1,
        pubkey: pubkey_arr,
        amount: shares_out,
        block_height: 0,
        block_timestamp: genesis_timestamp,
        status: UtxoStatus::Unspent,
    })?;

    // Mark issuer key as used
    store.mark_key_used(&pubkey_arr)?;

    Ok(meta)
}

// --- Helper functions for extracting typed values from DataItem ---

fn find_vbc(parent: &DataItem, code: i64, name: &str) -> Result<u64> {
    parent.find_child(code)
        .and_then(|c| c.as_vbc_value())
        .ok_or_else(|| ChainError::InvalidGenesis(format!("missing or invalid {}", name)))
}

fn find_bytes(parent: &DataItem, code: i64, name: &str) -> Result<Vec<u8>> {
    parent.find_child(code)
        .and_then(|c| c.as_bytes().map(|b| b.to_vec()))
        .ok_or_else(|| ChainError::InvalidGenesis(format!("missing or invalid {}", name)))
}

fn find_string(parent: &DataItem, code: i64, name: &str) -> Result<String> {
    let bytes = find_bytes(parent, code, name)?;
    String::from_utf8(bytes)
        .map_err(|_| ChainError::InvalidGenesis(format!("{} is not valid UTF-8", name)))
}

fn find_bigint(parent: &DataItem, code: i64, name: &str) -> Result<BigInt> {
    let bytes = find_bytes(parent, code, name)?;
    let (value, _) = bigint::decode_bigint(&bytes, 0)
        .map_err(|e| ChainError::InvalidGenesis(format!("{}: {}", name, e)))?;
    Ok(value)
}

fn find_rational(parent: &DataItem, code: i64, name: &str) -> Result<(BigInt, BigInt)> {
    let bytes = find_bytes(parent, code, name)?;
    let (rational, _) = bigint::decode_rational(&bytes, 0)
        .map_err(|e| ChainError::InvalidGenesis(format!("{}: {}", name, e)))?;
    Ok((rational.numer().clone(), rational.denom().clone()))
}

fn parse_timestamp_bytes(item: &DataItem) -> Result<i64> {
    let bytes = item.as_bytes()
        .ok_or_else(|| ChainError::InvalidGenesis("TIMESTAMP has no bytes".into()))?;
    if bytes.len() != 8 {
        return Err(ChainError::InvalidGenesis("TIMESTAMP must be 8 bytes".into()));
    }
    Ok(i64::from_be_bytes(bytes.try_into().expect("length validated above")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ao_crypto::sign::SigningKey;

    fn build_test_genesis() -> (DataItem, SigningKey) {
        // Build a minimal genesis block
        let seed = [0x42u8; 32];
        let key = SigningKey::from_seed(&seed);
        let pubkey = key.public_key_bytes().to_vec();

        let shares_out_val = BigInt::from(1u64 << 20); // 2^20 for testing
        let mut shares_bytes = Vec::new();
        bigint::encode_bigint(&shares_out_val, &mut shares_bytes);

        let coin_count_val = BigInt::from(1_000_000u64);
        let mut coin_bytes = Vec::new();
        bigint::encode_bigint(&coin_count_val, &mut coin_bytes);

        // Fee rate 1/1000
        let fee_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1000));
        let mut fee_bytes = Vec::new();
        bigint::encode_rational(&fee_rate, &mut fee_bytes);

        let expiry_ts = Timestamp::from_unix_seconds(31_536_000); // 1 year
        let expiry_bytes = expiry_ts.to_bytes().to_vec();

        let ts = Timestamp::from_unix_seconds(1_772_611_200); // 2026-03-06

        // Build signable content (everything except AUTH_SIG and SHA256)
        let signable_children = vec![
            DataItem::vbc_value(PROTOCOL_VER, 1),
            DataItem::bytes(CHAIN_SYMBOL, b"TST".to_vec()),
            DataItem::bytes(DESCRIPTION, b"Test chain".to_vec()),
            DataItem::bytes(COIN_COUNT, coin_bytes.clone()),
            DataItem::bytes(SHARES_OUT, shares_bytes.clone()),
            DataItem::bytes(FEE_RATE, fee_bytes.clone()),
            DataItem::bytes(EXPIRY_PERIOD, expiry_bytes.clone()),
            DataItem::vbc_value(EXPIRY_MODE, 1),
            DataItem::container(PARTICIPANT, vec![
                DataItem::bytes(ED25519_PUB, pubkey.to_vec()),
                DataItem::bytes(AMOUNT, shares_bytes.clone()),
            ]),
        ];
        let signable = DataItem::container(GENESIS, signable_children.clone());

        let sig = sign::sign_dataitem(&key, &signable, ts);

        // Build full genesis
        let mut all_children = signable_children;
        all_children.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        ]));

        // Compute chain ID hash
        let mut content_bytes = Vec::new();
        for child in &all_children {
            child.encode(&mut content_bytes);
        }
        let chain_hash = hash::sha256(&content_bytes);
        all_children.push(DataItem::bytes(SHA256, chain_hash.to_vec()));

        let genesis = DataItem::container(GENESIS, all_children);
        (genesis, key)
    }

    #[test]
    fn test_load_genesis() {
        let (genesis, _key) = build_test_genesis();
        let store = ChainStore::open_memory().unwrap();
        let meta = load_genesis(&store, &genesis).unwrap();

        assert_eq!(meta.symbol, "TST");
        assert_eq!(meta.expiry_mode, 1);
        assert_eq!(meta.block_height, 0);
        assert_eq!(meta.next_seq_id, 2);

        // Verify issuer UTXO was created
        let utxo = store.get_utxo(1).unwrap().unwrap();
        assert_eq!(utxo.amount, BigInt::from(1u64 << 20));
        assert_eq!(utxo.status, UtxoStatus::Unspent);
    }

    #[test]
    fn test_genesis_round_trip() {
        let (genesis, _key) = build_test_genesis();

        // Binary round-trip
        let bytes = genesis.to_bytes();
        let decoded = DataItem::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, genesis);

        // Load from decoded
        let store = ChainStore::open_memory().unwrap();
        let meta = load_genesis(&store, &decoded).unwrap();
        assert_eq!(meta.symbol, "TST");
    }

    #[test]
    fn test_wrong_type_code_rejected() {
        let item = DataItem::container(BLOCK, vec![]);
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();
        assert!(load_genesis(&store, &item).is_err());
    }
}
