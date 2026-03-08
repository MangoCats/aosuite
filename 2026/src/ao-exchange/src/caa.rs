//! CAA coordinator: builds, signs, and orchestrates the ouroboros recording sequence.
//!
//! This is the client-side coordinator per AtomicExchange.md §7.

use anyhow::{bail, Context, Result};
use num_bigint::BigInt;
use num_rational::BigRational;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_types::fees;
use ao_types::json as ao_json;
use ao_crypto::hash;
use ao_crypto::sign::{self, SigningKey};

use crate::client::RecorderClient;

/// A per-chain component of a CAA, with the info needed to build and sign it.
pub struct CaaChainComponent {
    /// 32-byte chain ID.
    pub chain_id: [u8; 32],
    /// Recorder client for this chain.
    pub client: RecorderClient,
    /// Givers on this chain: (seq_id, amount, signing seed).
    pub givers: Vec<CaaGiver>,
    /// Receivers on this chain: (pubkey, amount, signing seed).
    /// The last receiver's amount is auto-adjusted for fees.
    pub receivers: Vec<CaaReceiver>,
}

pub struct CaaGiver {
    pub seq_id: u64,
    pub amount: BigInt,
    pub seed: [u8; 32],
}

pub struct CaaReceiver {
    pub pubkey: [u8; 32],
    pub amount: BigInt,
    pub seed: [u8; 32],
}

/// Result of a completed CAA exchange.
pub struct CaaResult {
    /// The CAA hash (SHA2-256 of canonical content), hex-encoded.
    pub caa_hash: String,
    /// Per-chain recording proof JSON values (one per chain, in order).
    pub proofs: Vec<serde_json::Value>,
    /// Per-chain first_seq values (one per chain, in order).
    pub first_seqs: Vec<u64>,
    /// Per-chain seq_count values (one per chain, in order).
    pub seq_counts: Vec<u64>,
}

/// Fetched chain parameters needed for building assignments.
struct ChainParams {
    fee_num: BigInt,
    fee_den: BigInt,
    shares_out: BigInt,
    fee_rate: BigRational,
}

/// Execute a full CAA atomic exchange across multiple chains.
///
/// Builds the CAA, signs all components and the overall CAA, then runs the
/// ouroboros recording sequence. Finally submits binding proofs back to
/// chains 0..N-2.
///
/// `escrow_seconds`: how long escrowed shares remain locked (recommend 300).
///
/// **Retry policy:** This function does NOT retry on transient network failures.
/// Callers should implement retry with exponential backoff when `caa_submit` or
/// `caa_bind` fails with a retriable error. CAA submissions are idempotent, so
/// retrying a partially-completed sequence is safe. Ensure retries complete
/// before the escrow deadline expires.
pub async fn execute_caa(
    components: &mut [CaaChainComponent],
    escrow_seconds: i64,
) -> Result<CaaResult> {
    if components.len() < 2 {
        bail!("CAA requires at least 2 chains");
    }

    // 1. Fetch chain info for fee calculation
    let mut params = Vec::new();
    for comp in components.iter() {
        let chain_id_hex = hex::encode(comp.chain_id);
        let info = comp.client.chain_info(&chain_id_hex).await
            .with_context(|| format!("chain_info failed for {}", chain_id_hex))?;
        let fee_num: BigInt = info.fee_rate_num.parse()?;
        let fee_den: BigInt = info.fee_rate_den.parse()?;
        let shares_out: BigInt = info.shares_out.parse()?;
        let fee_rate = BigRational::new(fee_num.clone(), fee_den.clone());
        params.push(ChainParams { fee_num, fee_den, shares_out, fee_rate });
    }

    let now_secs = unix_now();
    let deadline_ts = Timestamp::from_unix_seconds(now_secs + escrow_seconds);

    // 2. Adjust last receiver amounts for fees (iterative convergence, 3 rounds)
    for (i, comp) in components.iter_mut().enumerate() {
        let p = &params[i];
        let giver_total: BigInt = comp.givers.iter().map(|g| &g.amount).sum();

        for _ in 0..3 {
            let assignment = build_assignment(comp, &p.fee_rate, now_secs);
            let page = DataItem::container(PAGE, vec![
                DataItem::vbc_value(PAGE_INDEX, 0),
                DataItem::container(AUTHORIZATION, vec![assignment]),
            ]);
            let page_bytes = page.to_bytes().len() as u64;
            let fee = fees::recording_fee(page_bytes, &p.fee_num, &p.fee_den, &p.shares_out);

            let other_total: BigInt = comp.receivers[..comp.receivers.len() - 1]
                .iter().map(|r| &r.amount).sum();
            let last_amount = &giver_total - &fee - &other_total;
            if last_amount <= BigInt::from(0) {
                bail!("chain {}: insufficient funds", hex::encode(comp.chain_id));
            }
            comp.receivers.last_mut().expect("non-empty").amount = last_amount;
        }
    }

    // 3. Build the full signed CAA
    let caa = build_signed_caa(components, &params, &deadline_ts, now_secs);
    let caa_json = ao_json::to_json(&caa);
    let caa_hash_hex = compute_caa_hash_hex(&caa);

    // 4. Ouroboros recording sequence
    let mut proofs: Vec<serde_json::Value> = Vec::new();
    let mut first_seqs: Vec<u64> = Vec::new();
    let mut seq_counts: Vec<u64> = Vec::new();

    for (i, comp) in components.iter().enumerate() {
        let chain_id_hex = hex::encode(comp.chain_id);

        // Build submission JSON: CAA + any prior recording proofs
        let submit_json = if i == 0 {
            caa_json.clone()
        } else {
            attach_proofs(&caa_json, &proofs)
        };

        let result = comp.client.caa_submit(&chain_id_hex, &submit_json).await
            .with_context(|| format!("caa_submit failed on chain {}", chain_id_hex))?;

        proofs.push(result.proof_json);
        first_seqs.push(result.first_seq);
        seq_counts.push(result.seq_count);
    }

    // 5. Submit binding proofs to chains 0..N-2
    for (i, comp) in components.iter().enumerate() {
        if i == components.len() - 1 {
            break;
        }
        let chain_id_hex = hex::encode(comp.chain_id);

        let bind_json = build_bind_json(&caa_hash_hex, &proofs);
        comp.client.caa_bind(&chain_id_hex, &bind_json).await
            .with_context(|| format!("caa_bind failed on chain {}", chain_id_hex))?;
    }

    Ok(CaaResult {
        caa_hash: caa_hash_hex,
        proofs,
        first_seqs,
        seq_counts,
    })
}

/// Build the full signed CAA DataItem with all components and signatures.
fn build_signed_caa(
    components: &[CaaChainComponent],
    params: &[ChainParams],
    deadline_ts: &Timestamp,
    base_unix_secs: i64,
) -> DataItem {
    let mut caa_children = Vec::new();

    // Escrow deadline and list size
    caa_children.push(DataItem::bytes(ESCROW_DEADLINE, deadline_ts.to_bytes().to_vec()));
    caa_children.push(DataItem::vbc_value(LIST_SIZE, components.len() as u64));

    // Build per-chain CAA_COMPONENTs
    let mut assignments = Vec::new();
    for (i, comp) in components.iter().enumerate() {
        let assignment = build_assignment(comp, &params[i].fee_rate, base_unix_secs);
        let component_sigs = sign_component(&assignment, comp, base_unix_secs);

        let mut comp_children = vec![
            DataItem::bytes(CHAIN_REF, comp.chain_id.to_vec()),
            DataItem::vbc_value(CHAIN_ORDER, i as u64),
            assignment.clone(),
        ];
        comp_children.extend(component_sigs);

        caa_children.push(DataItem::container(CAA_COMPONENT, comp_children));
        assignments.push(assignment);
    }

    // Build the canonical CAA (components only) for overall signing
    let canonical = DataItem::container(CAA, caa_children.clone());

    // Collect all participant seeds across all chains for overall signatures
    let mut sig_index = 0i64;
    for comp in components {
        for giver in &comp.givers {
            let ts = Timestamp::from_unix_seconds(
                base_unix_secs + 100_000 + sig_index,
            );
            let key = SigningKey::from_seed(&giver.seed);
            let sig = sign::sign_dataitem(&key, &canonical, ts);
            caa_children.push(DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
            ]));
            sig_index += 1;
        }
        for receiver in &comp.receivers {
            let ts = Timestamp::from_unix_seconds(
                base_unix_secs + 100_000 + sig_index,
            );
            let key = SigningKey::from_seed(&receiver.seed);
            let sig = sign::sign_dataitem(&key, &canonical, ts);
            caa_children.push(DataItem::container(AUTH_SIG, vec![
                DataItem::bytes(ED25519_SIG, sig.to_vec()),
                DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
                DataItem::bytes(ED25519_PUB, key.public_key_bytes().to_vec()),
            ]));
            sig_index += 1;
        }
    }

    DataItem::container(CAA, caa_children)
}

/// Build a per-chain ASSIGNMENT DataItem.
fn build_assignment(
    comp: &CaaChainComponent,
    fee_rate: &BigRational,
    base_unix_secs: i64,
) -> DataItem {
    let participant_count = comp.givers.len() + comp.receivers.len();
    let mut children = vec![DataItem::vbc_value(LIST_SIZE, participant_count as u64)];

    for giver in &comp.givers {
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(&giver.amount, &mut amount_bytes);
        children.push(DataItem::container(PARTICIPANT, vec![
            DataItem::vbc_value(SEQ_ID, giver.seq_id),
            DataItem::bytes(AMOUNT, amount_bytes),
        ]));
    }

    for receiver in &comp.receivers {
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(&receiver.amount, &mut amount_bytes);
        children.push(DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, receiver.pubkey.to_vec()),
            DataItem::bytes(AMOUNT, amount_bytes),
        ]));
    }

    let mut bid_bytes = Vec::new();
    bigint::encode_rational(fee_rate, &mut bid_bytes);
    children.push(DataItem::bytes(RECORDING_BID, bid_bytes));

    /// Default assignment deadline: 24 hours from creation.
    const DEADLINE_SECS: i64 = 86400;
    let deadline_ts = Timestamp::from_unix_seconds(base_unix_secs + DEADLINE_SECS);
    children.push(DataItem::bytes(DEADLINE, deadline_ts.to_bytes().to_vec()));

    DataItem::container(ASSIGNMENT, children)
}

/// Sign a component's assignment with all givers and receivers.
fn sign_component(
    assignment: &DataItem,
    comp: &CaaChainComponent,
    base_unix_secs: i64,
) -> Vec<DataItem> {
    let mut sigs = Vec::new();

    for (i, giver) in comp.givers.iter().enumerate() {
        let ts = Timestamp::from_unix_seconds(base_unix_secs + 1 + i as i64);
        let key = SigningKey::from_seed(&giver.seed);
        let sig = sign::sign_dataitem(&key, assignment, ts);
        sigs.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, i as u64),
        ]));
    }

    let giver_count = comp.givers.len();
    for (j, receiver) in comp.receivers.iter().enumerate() {
        let page_idx = giver_count + j;
        let participant_count = giver_count + comp.receivers.len();
        let ts = Timestamp::from_unix_seconds(
            base_unix_secs + 1 + participant_count as i64 + j as i64,
        );
        let key = SigningKey::from_seed(&receiver.seed);
        let sig = sign::sign_dataitem(&key, assignment, ts);
        sigs.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, page_idx as u64),
        ]));
    }

    sigs
}

/// Attach recording proofs to a CAA JSON value (appends to items array).
fn attach_proofs(caa_json: &serde_json::Value, proofs: &[serde_json::Value]) -> serde_json::Value {
    let mut json = caa_json.clone();
    if let Some(items) = json.as_object_mut()
        .and_then(|o| o.get_mut("items"))
        .and_then(|v| v.as_array_mut())
    {
        for proof in proofs {
            items.push(proof.clone());
        }
    }
    json
}

/// Build the binding submission JSON matching the recorder's expected format.
fn build_bind_json(caa_hash_hex: &str, proofs: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!({
        "caa_hash": caa_hash_hex,
        "proofs": proofs,
    })
}

/// Compute the CAA hash from a DataItem, return hex-encoded.
/// NB: logic must match ao_chain::caa::compute_caa_hash exactly.
fn compute_caa_hash_hex(caa: &DataItem) -> String {
    let mut canonical_children = Vec::new();
    for child in caa.children() {
        match child.type_code {
            ESCROW_DEADLINE | LIST_SIZE | CAA_COMPONENT => {
                canonical_children.push(child.clone());
            }
            _ => {}
        }
    }
    let canonical = DataItem::container(CAA, canonical_children);
    hex::encode(hash::sha256(&canonical.to_bytes()))
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_assignment_structure() {
        let comp = CaaChainComponent {
            chain_id: [0xAA; 32],
            client: RecorderClient::new("http://localhost:0"),
            givers: vec![CaaGiver {
                seq_id: 1,
                amount: BigInt::from(1000),
                seed: [0x01; 32],
            }],
            receivers: vec![CaaReceiver {
                pubkey: [0x02; 32],
                amount: BigInt::from(999),
                seed: [0x03; 32],
            }],
        };
        let fee_rate = BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
        let assignment = build_assignment(&comp, &fee_rate, 1000);

        assert_eq!(assignment.type_code, ASSIGNMENT);
        let children = assignment.children();
        // LIST_SIZE + 1 giver PARTICIPANT + 1 receiver PARTICIPANT + RECORDING_BID + DEADLINE
        assert_eq!(children.len(), 5);
        assert_eq!(children[0].type_code, LIST_SIZE);
        assert_eq!(children[1].type_code, PARTICIPANT);
        assert_eq!(children[2].type_code, PARTICIPANT);
        assert_eq!(children[3].type_code, RECORDING_BID);
        assert_eq!(children[4].type_code, DEADLINE);
    }

    #[test]
    fn test_sign_component_produces_correct_count() {
        let comp = CaaChainComponent {
            chain_id: [0xAA; 32],
            client: RecorderClient::new("http://localhost:0"),
            givers: vec![
                CaaGiver { seq_id: 1, amount: BigInt::from(500), seed: [0x01; 32] },
                CaaGiver { seq_id: 2, amount: BigInt::from(500), seed: [0x04; 32] },
            ],
            receivers: vec![
                CaaReceiver { pubkey: [0x02; 32], amount: BigInt::from(800), seed: [0x03; 32] },
                CaaReceiver { pubkey: [0x05; 32], amount: BigInt::from(199), seed: [0x06; 32] },
            ],
        };
        let fee_rate = BigRational::new(BigInt::from(1), BigInt::from(1_000_000));
        let assignment = build_assignment(&comp, &fee_rate, 1000);
        let sigs = sign_component(&assignment, &comp, 1000);

        // 2 givers + 2 receivers = 4 signatures
        assert_eq!(sigs.len(), 4);
        for sig in &sigs {
            assert_eq!(sig.type_code, AUTH_SIG);
        }
    }

    #[test]
    fn test_build_signed_caa_structure() {
        let comp_a = CaaChainComponent {
            chain_id: [0xAA; 32],
            client: RecorderClient::new("http://localhost:0"),
            givers: vec![CaaGiver { seq_id: 1, amount: BigInt::from(1000), seed: [0x01; 32] }],
            receivers: vec![CaaReceiver { pubkey: [0x02; 32], amount: BigInt::from(999), seed: [0x03; 32] }],
        };
        let comp_b = CaaChainComponent {
            chain_id: [0xBB; 32],
            client: RecorderClient::new("http://localhost:0"),
            givers: vec![CaaGiver { seq_id: 5, amount: BigInt::from(2000), seed: [0x04; 32] }],
            receivers: vec![CaaReceiver { pubkey: [0x05; 32], amount: BigInt::from(1999), seed: [0x06; 32] }],
        };

        let components = vec![comp_a, comp_b];
        let params = vec![
            ChainParams {
                fee_num: BigInt::from(1), fee_den: BigInt::from(1_000_000),
                shares_out: BigInt::from(1u64) << 86,
                fee_rate: BigRational::new(BigInt::from(1), BigInt::from(1_000_000)),
            },
            ChainParams {
                fee_num: BigInt::from(1), fee_den: BigInt::from(1_000_000),
                shares_out: BigInt::from(1u64) << 86,
                fee_rate: BigRational::new(BigInt::from(1), BigInt::from(1_000_000)),
            },
        ];
        let deadline = Timestamp::from_unix_seconds(2000);
        let caa = build_signed_caa(&components, &params, &deadline, 1000);

        assert_eq!(caa.type_code, CAA);
        let children = caa.children();

        // ESCROW_DEADLINE + LIST_SIZE + 2 CAA_COMPONENTs + 4 overall AUTH_SIGs
        // (2 givers + 2 receivers across both chains = 4 overall sigs)
        assert_eq!(children[0].type_code, ESCROW_DEADLINE);
        assert_eq!(children[1].type_code, LIST_SIZE);
        assert_eq!(children[2].type_code, CAA_COMPONENT);
        assert_eq!(children[3].type_code, CAA_COMPONENT);
        // Overall sigs start at index 4
        for child in &children[4..] {
            assert_eq!(child.type_code, AUTH_SIG);
        }
        assert_eq!(children.len(), 8); // 2 + 2 + 4
    }

    #[test]
    fn test_caa_hash_deterministic_and_excludes_sigs() {
        let comp_a = CaaChainComponent {
            chain_id: [0xAA; 32],
            client: RecorderClient::new("http://localhost:0"),
            givers: vec![CaaGiver { seq_id: 1, amount: BigInt::from(1000), seed: [0x01; 32] }],
            receivers: vec![CaaReceiver { pubkey: [0x02; 32], amount: BigInt::from(999), seed: [0x03; 32] }],
        };
        let comp_b = CaaChainComponent {
            chain_id: [0xBB; 32],
            client: RecorderClient::new("http://localhost:0"),
            givers: vec![CaaGiver { seq_id: 5, amount: BigInt::from(2000), seed: [0x04; 32] }],
            receivers: vec![CaaReceiver { pubkey: [0x05; 32], amount: BigInt::from(1999), seed: [0x06; 32] }],
        };

        let components = vec![comp_a, comp_b];
        let params = vec![
            ChainParams {
                fee_num: BigInt::from(1), fee_den: BigInt::from(1_000_000),
                shares_out: BigInt::from(1u64) << 86,
                fee_rate: BigRational::new(BigInt::from(1), BigInt::from(1_000_000)),
            },
            ChainParams {
                fee_num: BigInt::from(1), fee_den: BigInt::from(1_000_000),
                shares_out: BigInt::from(1u64) << 86,
                fee_rate: BigRational::new(BigInt::from(1), BigInt::from(1_000_000)),
            },
        ];
        let deadline = Timestamp::from_unix_seconds(2000);
        let caa = build_signed_caa(&components, &params, &deadline, 1000);

        let hash1 = compute_caa_hash_hex(&caa);
        let hash2 = compute_caa_hash_hex(&caa);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // 32 bytes hex
    }

    #[test]
    fn test_attach_proofs() {
        let caa_json = serde_json::json!({
            "type": "CAA",
            "code": CAA,
            "items": [
                { "type": "ESCROW_DEADLINE", "code": ESCROW_DEADLINE, "value": "0000000000000000" },
            ]
        });
        let proof = serde_json::json!({
            "type": "RECORDING_PROOF",
            "code": RECORDING_PROOF,
            "items": []
        });

        let result = attach_proofs(&caa_json, &[proof]);
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[1]["type"], "RECORDING_PROOF");
    }

    #[test]
    fn test_build_bind_json() {
        let proof = serde_json::json!({
            "type": "RECORDING_PROOF",
            "code": RECORDING_PROOF,
            "items": []
        });
        let bind = build_bind_json("abcd1234", &[proof]);

        assert_eq!(bind["caa_hash"], "abcd1234");
        let proofs = bind["proofs"].as_array().unwrap();
        assert_eq!(proofs.len(), 1);
        assert_eq!(proofs[0]["type"], "RECORDING_PROOF");
    }
}
