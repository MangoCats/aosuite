use ao_crypto::sign::SigningKey;
use num_bigint::BigInt;

/// A single key entry in the exchange wallet.
#[derive(Clone)]
pub struct KeyEntry {
    pub seed: [u8; 32],
    pub pubkey: [u8; 32],
    pub chain_id: String,
    pub seq_id: Option<u64>,
    pub amount: Option<BigInt>,
    pub spent: bool,
}

/// A UTXO known to be registered.
pub struct RegisteredUtxo {
    pub seed: [u8; 32],
    pub pubkey: [u8; 32],
    pub seq_id: u64,
    pub amount: BigInt,
}

/// Per-chain wallet managing Ed25519 keys and UTXO tracking.
pub struct Wallet {
    keys: Vec<KeyEntry>,
}

impl Default for Wallet {
    fn default() -> Self { Self::new() }
}

impl Wallet {
    pub fn new() -> Self {
        Wallet { keys: Vec::new() }
    }

    /// Generate a fresh key pair for a chain.
    pub fn generate_key(&mut self, chain_id: &str) -> KeyEntry {
        let key = SigningKey::generate();
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(key.public_key_bytes());
        self.push_key(*key.seed(), pubkey, chain_id)
    }

    /// Import a known key (e.g. from config seed).
    pub fn import_key(&mut self, seed: [u8; 32], chain_id: &str) -> KeyEntry {
        let key = SigningKey::from_seed(&seed);
        let mut pubkey = [0u8; 32];
        pubkey.copy_from_slice(key.public_key_bytes());
        self.push_key(seed, pubkey, chain_id)
    }

    fn push_key(&mut self, seed: [u8; 32], pubkey: [u8; 32], chain_id: &str) -> KeyEntry {
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

    /// Register that a key received a UTXO.
    pub fn register_utxo(&mut self, pubkey: &[u8; 32], seq_id: u64, amount: BigInt) {
        if let Some(entry) = self.keys.iter_mut().find(|k| &k.pubkey == pubkey) {
            entry.seq_id = Some(seq_id);
            entry.amount = Some(amount);
        }
    }

    /// Find an unspent UTXO on a given chain.
    pub fn find_unspent(&self, chain_id: &str) -> Option<RegisteredUtxo> {
        self.keys.iter()
            .filter(|k| k.chain_id == chain_id && !k.spent)
            .find_map(|k| {
                let seq_id = k.seq_id?;
                let amount = k.amount.clone()?;
                Some(RegisteredUtxo { seed: k.seed, pubkey: k.pubkey, seq_id, amount })
            })
    }

    /// Find all unspent UTXOs on a chain.
    pub fn find_all_unspent(&self, chain_id: &str) -> Vec<&KeyEntry> {
        self.keys.iter().filter(|k| {
            k.chain_id == chain_id && k.seq_id.is_some() && !k.spent
        }).collect()
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

    /// Check if wallet has a signing key for a pubkey.
    pub fn has_key(&self, pubkey: &[u8; 32]) -> bool {
        self.keys.iter().any(|k| &k.pubkey == pubkey)
    }
}
