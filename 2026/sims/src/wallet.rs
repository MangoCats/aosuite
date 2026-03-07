use ao_crypto::sign::SigningKey;
use num_bigint::BigInt;

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
}

impl KeyEntry {
    pub fn signing_key(&self) -> SigningKey {
        SigningKey::from_seed(&self.seed)
    }
}

/// Per-agent wallet: manages Ed25519 keys and tracks UTXO ownership.
pub struct Wallet {
    pub name: String,
    keys: Vec<KeyEntry>,
}

impl Wallet {
    pub fn new(name: &str) -> Self {
        Wallet {
            name: name.to_string(),
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

    /// Find an unspent UTXO on a given chain.
    pub fn find_unspent(&self, chain_id: &str) -> Option<&KeyEntry> {
        self.keys.iter().find(|k| {
            k.chain_id == chain_id && k.seq_id.is_some() && !k.spent
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

    /// Count of total keys held.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Count of unspent UTXOs.
    pub fn unspent_count(&self) -> usize {
        self.keys.iter().filter(|k| k.seq_id.is_some() && !k.spent).count()
    }
}
