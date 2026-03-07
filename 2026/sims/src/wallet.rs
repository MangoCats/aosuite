use std::collections::HashMap;

use ao_crypto::sign::SigningKey;
use num_bigint::BigInt;
use num_traits::Zero;
use serde::Serialize;

/// A single key entry in an agent's wallet.
#[derive(Clone)]
pub struct KeyEntry {
    pub seed: [u8; 32],
    pub pubkey: [u8; 32],
    pub chain_id: String,
    /// Sequence ID if this key has a recorded UTXO.
    pub seq_id: Option<u64>,
    /// Share amount held (if UTXO exists).
    pub amount: Option<BigInt>,
    pub spent: bool,
    /// When this key was generated (milliseconds since epoch).
    pub created_ms: u64,
}

/// Per-chain key inventory summary for the viewer.
#[derive(Debug, Clone, Serialize)]
pub struct WalletChainSummary {
    pub chain_id: String,
    pub total_keys: usize,
    pub unspent_keys: usize,
    pub spent_keys: usize,
    pub total_unspent_amount: String,
    pub oldest_unspent_ms: Option<u64>,
}

impl KeyEntry {
    pub fn signing_key(&self) -> SigningKey {
        SigningKey::from_seed(&self.seed)
    }
}

/// A UTXO known to be registered (has seq_id and amount).
pub struct RegisteredUtxo {
    pub seed: [u8; 32],
    pub pubkey: [u8; 32],
    pub seq_id: u64,
    pub amount: BigInt,
}

/// Per-agent wallet: manages Ed25519 keys and tracks UTXO ownership.
pub struct Wallet {
    keys: Vec<KeyEntry>,
}

impl Wallet {
    pub fn new(_name: &str) -> Self {
        Wallet {
            keys: Vec::new(),
        }
    }

    /// Generate a fresh key pair associated with a chain.
    pub fn generate_key(&mut self, chain_id: &str) -> KeyEntry {
        let key = SigningKey::generate();
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(key.public_key_bytes());
        let entry = KeyEntry {
            seed: *key.seed(),
            pubkey,
            chain_id: chain_id.to_string(),
            seq_id: None,
            amount: None,
            spent: false,
            created_ms: now_ms(),
        };
        self.keys.push(entry.clone());
        entry
    }

    /// Import a known key (e.g., issuer key from genesis).
    pub fn import_key(&mut self, seed: [u8; 32], chain_id: &str) -> KeyEntry {
        let key = SigningKey::from_seed(&seed);
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(key.public_key_bytes());
        let entry = KeyEntry {
            seed,
            pubkey,
            chain_id: chain_id.to_string(),
            seq_id: None,
            amount: None,
            spent: false,
            created_ms: now_ms(),
        };
        self.keys.push(entry.clone());
        entry
    }

    /// Register that a key received a UTXO (after a block is recorded).
    pub fn register_utxo(&mut self, pubkey: &[u8; 32], seq_id: u64, amount: BigInt) {
        if let Some(entry) = self.keys.iter_mut().find(|k| &k.pubkey == pubkey) {
            entry.seq_id = Some(seq_id);
            entry.amount = Some(amount);
        }
    }

    /// Find an unspent UTXO on a given chain (with guaranteed seq_id/amount).
    pub fn find_unspent(&self, chain_id: &str) -> Option<RegisteredUtxo> {
        self.keys.iter()
            .filter(|k| k.chain_id == chain_id && !k.spent)
            .find_map(|k| match (&k.seq_id, &k.amount) {
                (Some(seq_id), Some(amount)) => Some(RegisteredUtxo {
                    seed: k.seed,
                    pubkey: k.pubkey,
                    seq_id: *seq_id,
                    amount: amount.clone(),
                }),
                _ => None,
            })
    }

    /// Find all unspent UTXOs on a given chain.
    pub fn find_all_unspent(&self, chain_id: &str) -> Vec<&KeyEntry> {
        self.keys.iter().filter(|k| {
            k.chain_id == chain_id && k.seq_id.is_some() && !k.spent
        }).collect()
    }

    /// Get the signing key for a given public key.
    pub fn get_signing_key(&self, pubkey: &[u8; 32]) -> Option<SigningKey> {
        self.keys.iter()
            .find(|k| &k.pubkey == pubkey)
            .map(|k| k.signing_key())
    }

    /// Mark a key's UTXO as spent.
    pub fn mark_spent(&mut self, pubkey: &[u8; 32]) {
        if let Some(entry) = self.keys.iter_mut().find(|k| &k.pubkey == pubkey) {
            entry.spent = true;
        }
    }

    /// Total unspent balance on a chain (in shares).
    pub fn balance(&self, chain_id: &str) -> BigInt {
        self.find_all_unspent(chain_id).iter()
            .filter_map(|k| k.amount.as_ref())
            .sum()
    }

    /// Per-chain key inventory summary.
    pub fn chain_summaries(&self) -> Vec<WalletChainSummary> {
        let mut by_chain: HashMap<String, Vec<&KeyEntry>> = HashMap::new();
        for k in &self.keys {
            by_chain.entry(k.chain_id.clone()).or_default().push(k);
        }

        let mut result: Vec<WalletChainSummary> = by_chain.into_iter().map(|(chain_id, keys)| {
            let total_keys = keys.len();
            let spent_keys = keys.iter().filter(|k| k.spent).count();
            let unspent: Vec<&&KeyEntry> = keys.iter()
                .filter(|k| !k.spent && k.seq_id.is_some())
                .collect();
            let unspent_keys = unspent.len();
            let total_unspent_amount: BigInt = unspent.iter()
                .filter_map(|k| k.amount.as_ref())
                .fold(BigInt::zero(), |acc, v| acc + v);
            let oldest_unspent_ms = unspent.iter()
                .map(|k| k.created_ms)
                .min();

            WalletChainSummary {
                chain_id,
                total_keys,
                unspent_keys,
                spent_keys,
                total_unspent_amount: total_unspent_amount.to_string(),
                oldest_unspent_ms,
            }
        }).collect();
        result.sort_by(|a, b| a.chain_id.cmp(&b.chain_id));
        result
    }
}

pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
