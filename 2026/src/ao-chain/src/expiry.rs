use num_bigint::BigInt;
use num_traits::Zero;

use crate::error::Result;
use crate::store::{ChainStore, ChainMeta};

/// Run expiration sweep and return total expired shares.
/// Currently implements Mode 1 (hard cutoff) only.
/// Mode 2 (age tax) is deferred per EconomicRules.md §4.2.
pub fn run_expiry_sweep(
    store: &ChainStore,
    meta: &ChainMeta,
    current_timestamp: i64,
) -> Result<BigInt> {
    match meta.expiry_mode {
        1 => sweep_hard_cutoff(store, meta.expiry_period, current_timestamp),
        2 => {
            // Age tax mode — deferred, treat as no expiration for now
            Ok(BigInt::zero())
        }
        _ => Ok(BigInt::zero()),
    }
}

/// Mode 1: Hard cutoff. Expire all UTXOs whose receipt timestamp + expiry_period < current.
fn sweep_hard_cutoff(
    store: &ChainStore,
    expiry_period: i64,
    current_timestamp: i64,
) -> Result<BigInt> {
    let expired_utxos = store.find_expired_utxos(current_timestamp, expiry_period)?;
    let mut total = BigInt::zero();

    for utxo in &expired_utxos {
        store.mark_expired(utxo.seq_id)?;
        total += &utxo.amount;
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{ChainStore, Utxo, UtxoStatus};

    fn test_meta(expiry_period: i64) -> ChainMeta {
        ChainMeta {
            chain_id: [0; 32],
            symbol: "TST".into(),
            coin_count: BigInt::from(1_000_000u64),
            shares_out: BigInt::from(1_000_000u64),
            fee_rate_num: BigInt::from(1),
            fee_rate_den: BigInt::from(1000),
            expiry_period,
            expiry_mode: 1,
            tax_start_age: None,
            tax_doubling_period: None,
            block_height: 0,
            next_seq_id: 3,
            last_block_timestamp: 0,
            prev_hash: [0; 32],
        }
    }

    #[test]
    fn test_hard_cutoff_expires_old_utxos() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        store.insert_utxo(&Utxo {
            seq_id: 1, pubkey: [0x01; 32], amount: BigInt::from(500),
            block_height: 0, block_timestamp: 100, status: UtxoStatus::Unspent,
        }).unwrap();

        store.insert_utxo(&Utxo {
            seq_id: 2, pubkey: [0x02; 32], amount: BigInt::from(300),
            block_height: 0, block_timestamp: 250, status: UtxoStatus::Unspent,
        }).unwrap();

        let meta = test_meta(200);

        // At time 350: seq 1 (100+200=300 < 350) expired, seq 2 (250+200=450 > 350) still valid
        let expired = run_expiry_sweep(&store, &meta, 350).unwrap();
        assert_eq!(expired, BigInt::from(500));

        assert_eq!(store.get_utxo(1).unwrap().unwrap().status, UtxoStatus::Expired);
        assert_eq!(store.get_utxo(2).unwrap().unwrap().status, UtxoStatus::Unspent);
    }

    #[test]
    fn test_no_expiry_when_all_fresh() {
        let store = ChainStore::open_memory().unwrap();
        store.init_schema().unwrap();

        store.insert_utxo(&Utxo {
            seq_id: 1, pubkey: [0x01; 32], amount: BigInt::from(500),
            block_height: 0, block_timestamp: 100, status: UtxoStatus::Unspent,
        }).unwrap();

        let meta = test_meta(200);
        let expired = run_expiry_sweep(&store, &meta, 200).unwrap();
        assert_eq!(expired, BigInt::zero());
    }
}
