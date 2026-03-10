//! Chain audit — semantic validation for TⒶ³ block types.
//!
//! The `ChainAuditor` replays chain state (owner keys, recorder, pending
//! changes, frozen status) from genesis forward, validating every TⒶ³
//! block against the rules in CompetingRecorders.md. It also enforces the
//! fee ceiling: `RECORDING_FEE_ACTUAL ≤ genesis FEE_RATE`.

use anyhow::{Result, bail};
use num_rational::BigRational;

use ao_chain::genesis;
use ao_chain::migration;
use ao_chain::owner_keys;
use ao_chain::recorder_switch;
use ao_chain::reward_rate;
use ao_chain::store::{ChainMeta, ChainStore, PendingRecorderChange};
use ao_types::bigint;
use ao_types::dataitem::DataItem;
use ao_types::json as ao_json;
use ao_types::typecode::*;

/// Semantic auditor for a single chain.
///
/// Maintains a `ChainStore` replica with just enough state for TⒶ³
/// validation: owner keys, recorder pubkey, pending recorder change,
/// frozen status, and genesis parameters (fee rate, reward rate, etc.).
pub struct ChainAuditor {
    store: ChainStore,
    meta: Option<ChainMeta>,
}

/// An audit finding — a semantic violation detected during block audit.
#[derive(Debug, Clone)]
pub struct AuditFinding {
    pub height: u64,
    pub severity: Severity,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Chain rule violation — triggers an alteration alert.
    Error,
}

impl ChainAuditor {
    /// Open or create an auditor backed by a per-chain SQLite database.
    pub fn open(db_path: &str) -> Result<Self> {
        let store = ChainStore::open(db_path)
            .map_err(|e| anyhow::anyhow!("chain audit store: {}", e))?;
        store.init_schema()
            .map_err(|e| anyhow::anyhow!("chain audit schema: {}", e))?;
        let meta = store.load_chain_meta()
            .map_err(|e| anyhow::anyhow!("chain audit meta: {}", e))?;
        Ok(ChainAuditor { store, meta })
    }

    /// Open an in-memory auditor (for tests).
    #[cfg(test)]
    pub fn open_memory() -> Result<Self> {
        let store = ChainStore::open_memory()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        store.init_schema()
            .map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok(ChainAuditor { store, meta: None })
    }

    /// Whether the auditor has processed the genesis block.
    pub fn is_initialized(&self) -> bool {
        self.meta.is_some()
    }

    /// The last block height processed by this auditor.
    pub fn audited_height(&self) -> u64 {
        self.meta.as_ref().map(|m| m.block_height).unwrap_or(0)
    }

    /// Initialize from the genesis block (block 0). Must be called once
    /// before `audit_block`. The genesis_json is the raw JSON for block 0
    /// as returned by the recorder's blocks endpoint.
    pub fn process_genesis(&mut self, genesis_json: &serde_json::Value) -> Result<()> {
        if self.meta.is_some() {
            bail!("genesis already processed");
        }
        let genesis_item = ao_json::from_json(genesis_json)
            .map_err(|e| anyhow::anyhow!("genesis JSON decode: {}", e))?;
        let meta = genesis::load_genesis(&self.store, &genesis_item)
            .map_err(|e| anyhow::anyhow!("genesis validation: {}", e))?;
        self.meta = Some(meta);
        Ok(())
    }

    /// Audit a non-genesis block. Returns a list of findings (empty = valid).
    ///
    /// The caller is responsible for feeding blocks in order (height 1, 2, …).
    /// Hash integrity is verified separately by the verifier; this function
    /// only performs semantic/authority-chain validation.
    pub fn audit_block(
        &mut self,
        block_json: &serde_json::Value,
        height: u64,
    ) -> Result<Vec<AuditFinding>> {
        let meta = self.meta.clone()
            .ok_or_else(|| anyhow::anyhow!("genesis not processed"))?;

        let block = ao_json::from_json(block_json)
            .map_err(|e| anyhow::anyhow!("block {} JSON decode: {}", height, e))?;
        let mut findings = Vec::new();

        // Extract BLOCK_SIGNED → BLOCK_CONTENTS
        let block_signed = block.find_child(BLOCK_SIGNED)
            .ok_or_else(|| anyhow::anyhow!("block {} missing BLOCK_SIGNED", height))?;
        let block_contents = block_signed.find_child(BLOCK_CONTENTS)
            .ok_or_else(|| anyhow::anyhow!("block {} missing BLOCK_CONTENTS", height))?;

        // Extract block timestamp from blockmaker's AUTH_SIG
        let block_timestamp = extract_block_timestamp(block_signed)?;

        // Frozen chain check: no blocks should exist after CHAIN_MIGRATION
        if meta.frozen {
            findings.push(AuditFinding {
                height,
                severity: Severity::Error,
                message: "block recorded on frozen (migrated) chain".into(),
            });
        }

        // Fee ceiling enforcement: RECORDING_FEE_ACTUAL ≤ genesis FEE_RATE
        if let Some(fee_item) = block_contents.find_child(RECORDING_FEE_ACTUAL) {
            check_fee_ceiling(&meta, fee_item, height, &mut findings);
        }

        // Scan PAGE children for TⒶ³ items
        for child in block_contents.children() {
            if child.type_code == PAGE {
                for page_child in child.children() {
                    if page_child.type_code != PAGE_INDEX {
                        self.audit_content_item(
                            &meta, page_child, height, block_timestamp, &mut findings,
                        )?;
                    }
                }
            }
        }

        // Advance chain metadata
        self.advance_meta(height, block_timestamp, &block)?;

        Ok(findings)
    }

    /// Validate a TⒶ³ content item and apply state changes on success.
    fn audit_content_item(
        &mut self,
        meta: &ChainMeta,
        item: &DataItem,
        height: u64,
        block_timestamp: i64,
        findings: &mut Vec<AuditFinding>,
    ) -> Result<()> {
        match item.type_code {
            OWNER_KEY_ROTATION => {
                match owner_keys::validate_rotation(&self.store, meta, item, block_timestamp) {
                    Ok(vr) => {
                        owner_keys::apply_rotation(&self.store, &vr, height, block_timestamp)
                            .map_err(|e| anyhow::anyhow!("apply rotation: {}", e))?;
                    }
                    Err(e) => {
                        findings.push(AuditFinding {
                            height,
                            severity: Severity::Error,
                            message: format!("invalid OWNER_KEY_ROTATION: {}", e),
                        });
                    }
                }
            }
            OWNER_KEY_REVOCATION => {
                match owner_keys::validate_revocation(&self.store, meta, item, block_timestamp) {
                    Ok(vr) => {
                        owner_keys::apply_revocation(&self.store, &vr, height)
                            .map_err(|e| anyhow::anyhow!("apply revocation: {}", e))?;
                    }
                    Err(e) => {
                        findings.push(AuditFinding {
                            height,
                            severity: Severity::Error,
                            message: format!("invalid OWNER_KEY_REVOCATION: {}", e),
                        });
                    }
                }
            }
            RECORDER_CHANGE_PENDING => {
                match recorder_switch::validate_pending(&self.store, meta, item, block_timestamp) {
                    Ok(vp) => {
                        recorder_switch::apply_pending(&self.store, &vp, height)
                            .map_err(|e| anyhow::anyhow!("apply pending: {}", e))?;
                        if let Some(ref mut m) = self.meta {
                            m.pending_recorder_change = Some(PendingRecorderChange {
                                new_recorder_pubkey: vp.new_recorder_pubkey,
                                new_recorder_url: vp.new_recorder_url.clone(),
                                pending_height: height,
                                owner_auth_sig_bytes: vp.owner_auth_sig_bytes.clone(),
                            });
                        }
                    }
                    Err(e) => {
                        findings.push(AuditFinding {
                            height,
                            severity: Severity::Error,
                            message: format!("invalid RECORDER_CHANGE_PENDING: {}", e),
                        });
                    }
                }
            }
            RECORDER_CHANGE => {
                match recorder_switch::validate_change(&self.store, meta, item, block_timestamp) {
                    Ok(vc) => {
                        recorder_switch::apply_change(&self.store, &vc)
                            .map_err(|e| anyhow::anyhow!("apply change: {}", e))?;
                        if let Some(ref mut m) = self.meta {
                            m.recorder_pubkey = Some(vc.new_recorder_pubkey);
                            m.pending_recorder_change = None;
                        }
                    }
                    Err(e) => {
                        findings.push(AuditFinding {
                            height,
                            severity: Severity::Error,
                            message: format!("invalid RECORDER_CHANGE: {}", e),
                        });
                    }
                }
            }
            RECORDER_URL_CHANGE => {
                if let Err(e) = recorder_switch::validate_url_change(
                    &self.store, meta, item, block_timestamp,
                ) {
                    findings.push(AuditFinding {
                        height,
                        severity: Severity::Error,
                        message: format!("invalid RECORDER_URL_CHANGE: {}", e),
                    });
                }
                // No store state change for URL changes.
            }
            REWARD_RATE_CHANGE => {
                match reward_rate::validate_reward_rate_change(
                    &self.store, meta, item, block_timestamp,
                ) {
                    Ok(vrc) => {
                        reward_rate::apply_reward_rate_change(&self.store, &vrc)
                            .map_err(|e| anyhow::anyhow!("apply rate change: {}", e))?;
                        if let Some(ref mut m) = self.meta {
                            m.reward_rate_num = vrc.new_rate_num.clone();
                            m.reward_rate_den = vrc.new_rate_den.clone();
                        }
                    }
                    Err(e) => {
                        findings.push(AuditFinding {
                            height,
                            severity: Severity::Error,
                            message: format!("invalid REWARD_RATE_CHANGE: {}", e),
                        });
                    }
                }
            }
            CHAIN_MIGRATION => {
                match migration::validate_chain_migration(
                    &self.store, meta, item, block_timestamp,
                ) {
                    Ok(_vm) => {
                        migration::apply_chain_migration(&self.store)
                            .map_err(|e| anyhow::anyhow!("apply migration: {}", e))?;
                        if let Some(ref mut m) = self.meta {
                            m.frozen = true;
                        }
                    }
                    Err(e) => {
                        findings.push(AuditFinding {
                            height,
                            severity: Severity::Error,
                            message: format!("invalid CHAIN_MIGRATION: {}", e),
                        });
                    }
                }
            }
            _ => {
                // Regular content (ASSIGN, CAA, etc.) — no TⒶ³ audit needed.
            }
        }
        Ok(())
    }

    /// Advance the in-memory metadata after processing a block.
    fn advance_meta(
        &mut self,
        height: u64,
        timestamp: i64,
        block: &DataItem,
    ) -> Result<()> {
        if let Some(ref mut meta) = self.meta {
            meta.block_height = height;
            meta.last_block_timestamp = timestamp;
            // Update prev_hash from the block's SHA256 (the block hash).
            // Every valid BLOCK must have a SHA256 child — warn if missing.
            match block.find_child(SHA256).and_then(|h| h.as_bytes()) {
                Some(hash_bytes) if hash_bytes.len() == 32 => {
                    meta.prev_hash.copy_from_slice(hash_bytes);
                }
                _ => {
                    tracing::warn!(
                        height,
                        "block missing valid SHA256 — prev_hash not advanced",
                    );
                }
            }
            self.store.store_chain_meta(meta)
                .map_err(|e| anyhow::anyhow!("store meta: {}", e))?;
        }
        Ok(())
    }
}

/// Check that RECORDING_FEE_ACTUAL ≤ genesis FEE_RATE.
fn check_fee_ceiling(
    meta: &ChainMeta,
    fee_item: &DataItem,
    height: u64,
    findings: &mut Vec<AuditFinding>,
) {
    use num_bigint::BigInt;

    let Some(fee_bytes) = fee_item.as_bytes() else {
        findings.push(AuditFinding {
            height,
            severity: Severity::Error,
            message: "RECORDING_FEE_ACTUAL has no data".into(),
        });
        return;
    };

    // Guard against corrupt genesis with zero denominator.
    if meta.fee_rate_den <= BigInt::from(0) {
        findings.push(AuditFinding {
            height,
            severity: Severity::Error,
            message: "genesis FEE_RATE has non-positive denominator — cannot enforce ceiling".into(),
        });
        return;
    }

    match bigint::decode_rational(fee_bytes, 0) {
        Ok((actual_rate, _)) => {
            let ceiling = BigRational::new(
                meta.fee_rate_num.clone(),
                meta.fee_rate_den.clone(),
            );
            if actual_rate > ceiling {
                findings.push(AuditFinding {
                    height,
                    severity: Severity::Error,
                    message: format!(
                        "RECORDING_FEE_ACTUAL ({}) exceeds genesis FEE_RATE ceiling ({})",
                        actual_rate, ceiling,
                    ),
                });
            }
        }
        Err(e) => {
            findings.push(AuditFinding {
                height,
                severity: Severity::Error,
                message: format!("RECORDING_FEE_ACTUAL decode error: {}", e),
            });
        }
    }
}

/// Extract the block timestamp from BLOCK_SIGNED's AUTH_SIG.
fn extract_block_timestamp(block_signed: &DataItem) -> Result<i64> {
    let auth_sig = block_signed.find_child(AUTH_SIG)
        .ok_or_else(|| anyhow::anyhow!("BLOCK_SIGNED missing AUTH_SIG"))?;
    let ts_item = auth_sig.find_child(TIMESTAMP)
        .ok_or_else(|| anyhow::anyhow!("blockmaker AUTH_SIG missing TIMESTAMP"))?;
    let ts_bytes = ts_item.as_bytes()
        .ok_or_else(|| anyhow::anyhow!("TIMESTAMP has no bytes"))?;
    if ts_bytes.len() != 8 {
        bail!("blockmaker TIMESTAMP must be 8 bytes, got {}", ts_bytes.len());
    }
    Ok(i64::from_be_bytes(ts_bytes.try_into().expect("length validated")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ao_crypto::sign::SigningKey;
    use ao_crypto::{hash, sign};
    use ao_types::timestamp::Timestamp;
    use num_bigint::BigInt;

    /// Build a minimal genesis DataItem.
    fn build_genesis(issuer_key: &SigningKey) -> DataItem {
        let ts = Timestamp::from_unix_seconds(1_772_700_000);
        let shares = BigInt::from(1_000_000u64);

        let mut shares_bytes = Vec::new();
        bigint::encode_bigint(&shares, &mut shares_bytes);

        let fee_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(1000));
        let mut fee_bytes = Vec::new();
        bigint::encode_rational(&fee_rate, &mut fee_bytes);

        let expiry_ts = Timestamp::from_unix_seconds(365 * 24 * 3600);
        let expiry_bytes = expiry_ts.raw().to_be_bytes();

        let signable_children = vec![
            DataItem::vbc_value(PROTOCOL_VER, 1),
            DataItem::bytes(CHAIN_SYMBOL, b"TST".to_vec()),
            DataItem::bytes(COIN_COUNT, shares_bytes.clone()),
            DataItem::bytes(SHARES_OUT, shares_bytes.clone()),
            DataItem::bytes(FEE_RATE, fee_bytes),
            DataItem::bytes(EXPIRY_PERIOD, expiry_bytes.to_vec()),
            DataItem::vbc_value(EXPIRY_MODE, 1),
            DataItem::container(PARTICIPANT, vec![
                DataItem::bytes(ED25519_PUB, issuer_key.public_key_bytes().to_vec()),
                DataItem::bytes(AMOUNT, shares_bytes),
            ]),
        ];
        let signable = DataItem::container(GENESIS, signable_children.clone());

        let sig = sign::sign_dataitem(issuer_key, &signable, ts);

        // Compute chain ID = SHA256 of all child encodings
        let mut content_bytes = Vec::new();
        for child in &signable_children {
            child.encode(&mut content_bytes);
        }
        // Also include AUTH_SIG in chain ID hash
        let auth_sig = DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        ]);
        auth_sig.encode(&mut content_bytes);
        let chain_id = hash::sha256(&content_bytes);

        let mut all_children = signable_children;
        all_children.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
        ]));
        all_children.push(DataItem::bytes(SHA256, chain_id.to_vec()));

        DataItem::container(GENESIS, all_children)
    }

    /// Wrap content items in a BLOCK structure.
    fn wrap_in_block(
        blockmaker: &SigningKey,
        prev_hash: &[u8; 32],
        block_contents_children: Vec<DataItem>,
        block_timestamp: Timestamp,
    ) -> DataItem {
        let mut children = vec![
            DataItem::bytes(PREV_HASH, prev_hash.to_vec()),
        ];
        children.extend(block_contents_children);
        let block_contents = DataItem::container(BLOCK_CONTENTS, children);

        let sig = sign::sign_dataitem(blockmaker, &block_contents, block_timestamp);
        let block_signed = DataItem::container(BLOCK_SIGNED, vec![
            block_contents,
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, block_timestamp.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, blockmaker.public_key_bytes().to_vec()),
            ]),
        ]);

        let signed_bytes = block_signed.to_bytes();
        let block_hash = hash::sha256(&signed_bytes);

        DataItem::container(BLOCK, vec![
            DataItem::bytes(SHA256, block_hash.to_vec()),
            block_signed,
        ])
    }

    fn build_fee_actual(num: i64, den: i64) -> DataItem {
        let rate = num_rational::BigRational::new(BigInt::from(num), BigInt::from(den));
        let mut bytes = Vec::new();
        bigint::encode_rational(&rate, &mut bytes);
        DataItem::bytes(RECORDING_FEE_ACTUAL, bytes)
    }

    #[test]
    fn test_genesis_initializes_auditor() {
        let issuer = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        assert!(!auditor.is_initialized());

        auditor.process_genesis(&genesis_json).unwrap();
        assert!(auditor.is_initialized());
        assert_eq!(auditor.audited_height(), 0);
    }

    #[test]
    fn test_fee_ceiling_ok() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Fee 1/1000 exactly matches ceiling — should be fine
        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let block = wrap_in_block(
            &blockmaker, &prev_hash,
            vec![build_fee_actual(1, 1000)],
            ts,
        );
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert!(findings.is_empty(), "fee at ceiling should pass: {:?}", findings);
    }

    #[test]
    fn test_fee_ceiling_breach() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Fee 2/1000 exceeds ceiling of 1/1000
        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let block = wrap_in_block(
            &blockmaker, &prev_hash,
            vec![build_fee_actual(2, 1000)],
            ts,
        );
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("exceeds"));
    }

    #[test]
    fn test_frozen_chain_alert() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();

        // Manually freeze the chain
        auditor.meta.as_mut().unwrap().frozen = true;

        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;
        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let block = wrap_in_block(&blockmaker, &prev_hash, vec![], ts);
        let block_json = ao_json::to_json(&block);

        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert!(findings.iter().any(|f| f.message.contains("frozen")));
    }

    #[test]
    fn test_valid_owner_key_rotation_audited() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Build a valid OWNER_KEY_ROTATION
        let new_key = SigningKey::generate();
        let mut new_pk = [0u8; 32];
        new_pk.copy_from_slice(new_key.public_key_bytes());

        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let signable = DataItem::container(OWNER_KEY_ROTATION, vec![
            DataItem::bytes(ED25519_PUB, new_pk.to_vec()),
        ]);
        let sig = sign::sign_dataitem(&issuer, &signable, ts);
        let rotation = DataItem::container(OWNER_KEY_ROTATION, vec![
            DataItem::bytes(ED25519_PUB, new_pk.to_vec()),
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, issuer.public_key_bytes().to_vec()),
            ]),
        ]);

        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            rotation,
        ]);

        let block = wrap_in_block(&blockmaker, &prev_hash, vec![page], ts);
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert!(findings.is_empty(), "valid rotation should pass: {:?}", findings);
    }

    #[test]
    fn test_invalid_rotation_signer_detected() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let attacker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Build OWNER_KEY_ROTATION signed by attacker (not a valid owner key)
        let new_key = SigningKey::generate();
        let mut new_pk = [0u8; 32];
        new_pk.copy_from_slice(new_key.public_key_bytes());

        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let signable = DataItem::container(OWNER_KEY_ROTATION, vec![
            DataItem::bytes(ED25519_PUB, new_pk.to_vec()),
        ]);
        let sig = sign::sign_dataitem(&attacker, &signable, ts);
        let rotation = DataItem::container(OWNER_KEY_ROTATION, vec![
            DataItem::bytes(ED25519_PUB, new_pk.to_vec()),
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, attacker.public_key_bytes().to_vec()),
            ]),
        ]);

        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            rotation,
        ]);

        let block = wrap_in_block(&blockmaker, &prev_hash, vec![page], ts);
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("OWNER_KEY_ROTATION"));
    }

    #[test]
    fn test_no_findings_for_regular_blocks() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Block with no TⒶ³ items and fee within ceiling
        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let block = wrap_in_block(
            &blockmaker, &prev_hash,
            vec![build_fee_actual(1, 2000)], // under ceiling
            ts,
        );
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn test_multi_block_sequence_advances_state() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        assert_eq!(auditor.audited_height(), 0);

        // Process 3 blocks in sequence
        for h in 1..=3u64 {
            let meta = auditor.meta.as_ref().unwrap();
            let prev_hash = meta.prev_hash;
            let ts = Timestamp::from_unix_seconds(1_772_700_000 + h as i64);
            let block = wrap_in_block(
                &blockmaker, &prev_hash,
                vec![build_fee_actual(1, 2000)],
                ts,
            );
            let block_json = ao_json::to_json(&block);
            let findings = auditor.audit_block(&block_json, h).unwrap();
            assert!(findings.is_empty(), "block {} had findings: {:?}", h, findings);
            assert_eq!(auditor.audited_height(), h);
        }

        // Verify prev_hash changed from genesis
        let meta = auditor.meta.as_ref().unwrap();
        assert_eq!(meta.block_height, 3);
    }

    #[test]
    fn test_reward_rate_change_invalid_only_one_sig() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();

        // Set a recorder pubkey (needed for REWARD_RATE_CHANGE validation)
        let recorder_key = SigningKey::generate();
        let mut recorder_pk = [0u8; 32];
        recorder_pk.copy_from_slice(recorder_key.public_key_bytes());
        auditor.store.set_recorder_pubkey(&recorder_pk).unwrap();
        auditor.meta.as_mut().unwrap().recorder_pubkey = Some(recorder_pk);

        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Build REWARD_RATE_CHANGE with only 1 sig (should require 2)
        let new_rate = num_rational::BigRational::new(BigInt::from(1), BigInt::from(50));
        let mut rate_bytes = Vec::new();
        bigint::encode_rational(&new_rate, &mut rate_bytes);

        let signable = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes.clone()),
        ]);

        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let owner_sig = sign::sign_dataitem(&issuer, &signable, ts);
        let rate_change = DataItem::container(REWARD_RATE_CHANGE, vec![
            DataItem::bytes(REWARD_RATE, rate_bytes),
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, owner_sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, issuer.public_key_bytes().to_vec()),
            ]),
        ]);

        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            rate_change,
        ]);

        let block = wrap_in_block(&blockmaker, &prev_hash, vec![page], ts);
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("REWARD_RATE_CHANGE"));
    }

    #[test]
    fn test_chain_migration_no_escrows() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Build CHAIN_MIGRATION with owner sig (full tier)
        let new_chain_id = [0xBB; 32];
        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let signable = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
        ]);
        let sig = sign::sign_dataitem(&issuer, &signable, ts);
        let migration = DataItem::container(CHAIN_MIGRATION, vec![
            DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, issuer.public_key_bytes().to_vec()),
            ]),
        ]);

        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            migration,
        ]);

        let block = wrap_in_block(&blockmaker, &prev_hash, vec![page], ts);
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert!(findings.is_empty(), "valid migration should pass: {:?}", findings);

        // Chain should now be frozen
        assert!(auditor.meta.as_ref().unwrap().frozen);
    }

    #[test]
    fn test_recorder_change_without_pending_fails() {
        let issuer = SigningKey::generate();
        let blockmaker = SigningKey::generate();
        let genesis = build_genesis(&issuer);
        let genesis_json = ao_json::to_json(&genesis);

        let mut auditor = ChainAuditor::open_memory().unwrap();
        auditor.process_genesis(&genesis_json).unwrap();
        let meta = auditor.meta.as_ref().unwrap();
        let prev_hash = meta.prev_hash;

        // Attempt RECORDER_CHANGE without a pending change
        let new_recorder = SigningKey::generate();
        let ts = Timestamp::from_unix_seconds(1_772_700_001);
        let signable = DataItem::container(RECORDER_CHANGE, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new.example.com".to_vec()),
        ]);
        let sig = sign::sign_dataitem(&issuer, &signable, ts);
        let change = DataItem::container(RECORDER_CHANGE, vec![
            DataItem::bytes(ED25519_PUB, new_recorder.public_key_bytes().to_vec()),
            DataItem::bytes(RECORDER_URL, b"https://new.example.com".to_vec()),
            DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, issuer.public_key_bytes().to_vec()),
            ]),
        ]);

        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            change,
        ]);

        let block = wrap_in_block(&blockmaker, &prev_hash, vec![page], ts);
        let block_json = ao_json::to_json(&block);
        let findings = auditor.audit_block(&block_json, 1).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("RECORDER_CHANGE"));
    }
}
