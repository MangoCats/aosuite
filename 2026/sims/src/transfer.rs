use anyhow::{Result, bail};
use num_bigint::BigInt;

use ao_types::dataitem::DataItem;
use ao_types::typecode::*;
use ao_types::bigint;
use ao_types::timestamp::Timestamp;
use ao_types::fees;
use ao_types::json as ao_json;
use ao_crypto::sign::{self, SigningKey};

use crate::client::{RecorderClient, BlockResult};

/// A giver in a transfer: an existing UTXO being spent.
pub struct Giver {
    pub seq_id: u64,
    pub amount: BigInt,
    pub seed: [u8; 32],
}

/// A receiver in a transfer: a fresh key receiving shares.
pub struct Receiver {
    pub pubkey: [u8; 32],
    pub seed: [u8; 32],
    /// Desired share amount. The LAST receiver's amount is adjusted for fees.
    pub amount: BigInt,
}

/// Fee rate as num/den rational.
#[derive(Clone)]
pub struct FeeRate {
    pub num: BigInt,
    pub den: BigInt,
}

impl FeeRate {
    pub fn to_rational(&self) -> num_rational::BigRational {
        num_rational::BigRational::new(self.num.clone(), self.den.clone())
    }
}

/// Build, sign, and submit an assignment. Returns the recorded block info.
///
/// The last receiver's amount is automatically adjusted to satisfy the
/// balance equation: giver_total = receiver_total + fee.
///
/// All participants (givers + receivers) must provide signing keys,
/// matching the mutual-consent requirement.
pub async fn execute_transfer(
    client: &RecorderClient,
    chain_id: &str,
    givers: &[Giver],
    receivers: &mut [Receiver],
) -> Result<BlockResult> {
    if givers.is_empty() || receivers.is_empty() {
        bail!("transfer requires at least one giver and one receiver");
    }

    // Fetch chain info for fee calculation
    let info = client.chain_info(chain_id).await?;
    let shares_out: BigInt = info.shares_out.parse()?;
    let fee_num: BigInt = info.fee_rate_num.parse()?;
    let fee_den: BigInt = info.fee_rate_den.parse()?;

    let giver_total: BigInt = givers.iter().map(|g| &g.amount).sum();

    let fee_rate = FeeRate { num: fee_num.clone(), den: fee_den.clone() };

    // Iterative fee convergence (3 rounds, matching test pattern)
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;

    for _ in 0..3 {
        let auth = build_full_authorization(givers, receivers, now_secs, &fee_rate);
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            auth,
        ]);
        let page_bytes = page.to_bytes().len() as u64;
        let fee = fees::recording_fee(page_bytes, &fee_num, &fee_den, &shares_out);

        // Adjust last receiver to absorb the fee
        let other_receivers_total: BigInt = receivers[..receivers.len() - 1]
            .iter()
            .map(|r| &r.amount)
            .sum();
        let last_amount = &giver_total - &fee - &other_receivers_total;
        if last_amount <= BigInt::from(0) {
            bail!(
                "insufficient funds: giver_total={}, fee={}, other_receivers={}",
                giver_total, fee, other_receivers_total
            );
        }
        receivers.last_mut().unwrap().amount = last_amount;
    }

    // Build final authorization and submit
    let auth = build_full_authorization(givers, receivers, now_secs, &fee_rate);
    let auth_json = ao_json::to_json(&auth);

    client.submit(chain_id, &auth_json).await
}

/// Build a complete AUTHORIZATION DataItem with all signatures.
fn build_full_authorization(
    givers: &[Giver],
    receivers: &[Receiver],
    base_unix_secs: i64,
    fee_rate: &FeeRate,
) -> DataItem {
    let assignment = build_assignment(givers, receivers, fee_rate);

    let participant_count = givers.len() + receivers.len();
    let mut auth_children = vec![assignment.clone()];

    // Each participant signs with incrementing timestamps
    for (i, giver) in givers.iter().enumerate() {
        let ts = Timestamp::from_unix_seconds(base_unix_secs + 1 + i as i64);
        let key = SigningKey::from_seed(&giver.seed);
        let sig = sign::sign_dataitem(&key, &assignment, ts);

        auth_children.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, i as u64),
        ]));
    }

    for (j, receiver) in receivers.iter().enumerate() {
        let page_idx = givers.len() + j;
        let ts = Timestamp::from_unix_seconds(
            base_unix_secs + 1 + participant_count as i64 + j as i64,
        );
        let key = SigningKey::from_seed(&receiver.seed);
        let sig = sign::sign_dataitem(&key, &assignment, ts);

        auth_children.push(DataItem::container(AUTH_SIG, vec![
            DataItem::bytes(ED25519_SIG, sig.to_vec()),
            DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
            DataItem::vbc_value(PAGE_INDEX, page_idx as u64),
        ]));
    }

    DataItem::container(AUTHORIZATION, auth_children)
}

/// Build an ASSIGNMENT DataItem from givers and receivers.
fn build_assignment(givers: &[Giver], receivers: &[Receiver], fee_rate: &FeeRate) -> DataItem {
    let participant_count = givers.len() + receivers.len();
    let mut children = vec![DataItem::vbc_value(LIST_SIZE, participant_count as u64)];

    for giver in givers {
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(&giver.amount, &mut amount_bytes);
        children.push(DataItem::container(PARTICIPANT, vec![
            DataItem::vbc_value(SEQ_ID, giver.seq_id),
            DataItem::bytes(AMOUNT, amount_bytes),
        ]));
    }

    for receiver in receivers {
        let mut amount_bytes = Vec::new();
        bigint::encode_bigint(&receiver.amount, &mut amount_bytes);
        children.push(DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, receiver.pubkey.to_vec()),
            DataItem::bytes(AMOUNT, amount_bytes),
        ]));
    }

    // Recording bid must be >= chain fee rate
    let bid = fee_rate.to_rational();
    let mut bid_bytes = Vec::new();
    bigint::encode_rational(&bid, &mut bid_bytes);
    children.push(DataItem::bytes(RECORDING_BID, bid_bytes));

    // Deadline: 1 day from now
    let deadline_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64 + 86400;
    let deadline_ts = Timestamp::from_unix_seconds(deadline_secs);
    children.push(DataItem::bytes(DEADLINE, deadline_ts.to_bytes().to_vec()));

    DataItem::container(ASSIGNMENT, children)
}

// ── TⒶ³ operation builders (Sim-G) ──────────────────────────────────

/// Build a signed OWNER_KEY_ROTATION DataItem.
/// The signer must be a current valid owner key.
pub fn build_owner_key_rotation(
    signer_seed: &[u8; 32],
    new_pubkey: &[u8; 32],
) -> DataItem {
    let signer = SigningKey::from_seed(signer_seed);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let sign_ts = Timestamp::from_unix_seconds(now_secs);

    let signable_children = vec![
        DataItem::bytes(ED25519_PUB, new_pubkey.to_vec()),
    ];
    let signable = DataItem::container(OWNER_KEY_ROTATION, signable_children.clone());
    let sig = sign::sign_dataitem(&signer, &signable, sign_ts);
    let mut children = signable_children;
    children.push(DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, sign_ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, signer.public_key_bytes().to_vec()),
    ]));
    DataItem::container(OWNER_KEY_ROTATION, children)
}

/// Build a signed RECORDER_CHANGE_PENDING DataItem.
/// The owner signs, specifying the new recorder's pubkey and URL.
pub fn build_recorder_change_pending(
    owner_seed: &[u8; 32],
    new_recorder_pubkey: &[u8; 32],
    new_recorder_url: &str,
) -> DataItem {
    let owner = SigningKey::from_seed(owner_seed);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let sign_ts = Timestamp::from_unix_seconds(now_secs);

    let signable_children = vec![
        DataItem::bytes(ED25519_PUB, new_recorder_pubkey.to_vec()),
        DataItem::bytes(RECORDER_URL, new_recorder_url.as_bytes().to_vec()),
    ];
    let signable = DataItem::container(RECORDER_CHANGE_PENDING, signable_children.clone());
    let sig = sign::sign_dataitem(&owner, &signable, sign_ts);
    let mut children = signable_children;
    children.push(DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, sign_ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, owner.public_key_bytes().to_vec()),
    ]));
    DataItem::container(RECORDER_CHANGE_PENDING, children)
}

/// Build a signed CHAIN_MIGRATION DataItem.
/// The owner signs, specifying the new chain's ID via CHAIN_REF.
pub fn build_chain_migration(
    owner_seed: &[u8; 32],
    new_chain_id: &[u8; 32],
) -> DataItem {
    let owner = SigningKey::from_seed(owner_seed);
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let sign_ts = Timestamp::from_unix_seconds(now_secs);

    let signable_children = vec![
        DataItem::bytes(CHAIN_REF, new_chain_id.to_vec()),
    ];
    let signable = DataItem::container(CHAIN_MIGRATION, signable_children.clone());
    let sig = sign::sign_dataitem(&owner, &signable, sign_ts);
    let mut children = signable_children;
    children.push(DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, sign_ts.to_bytes().to_vec()),
        DataItem::bytes(ED25519_PUB, owner.public_key_bytes().to_vec()),
    ]));
    DataItem::container(CHAIN_MIGRATION, children)
}

/// Build a genesis block DataItem for creating a new chain.
pub fn build_genesis(
    issuer_seed: &[u8; 32],
    symbol: &str,
    description: &str,
    coins: &BigInt,
    shares: &BigInt,
    chain_fee_rate: &FeeRate,
) -> (DataItem, serde_json::Value) {
    let issuer_key = SigningKey::from_seed(issuer_seed);
    let pubkey = issuer_key.public_key_bytes().to_vec();

    let mut shares_bytes = Vec::new();
    bigint::encode_bigint(shares, &mut shares_bytes);

    let mut coin_bytes = Vec::new();
    bigint::encode_bigint(coins, &mut coin_bytes);

    let fee_rate = chain_fee_rate.to_rational();
    let mut fee_bytes = Vec::new();
    bigint::encode_rational(&fee_rate, &mut fee_bytes);

    let expiry_period = Timestamp::from_unix_seconds(31_557_600); // 1 year
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let ts = Timestamp::from_unix_seconds(now_secs);

    let signable_children = vec![
        DataItem::vbc_value(PROTOCOL_VER, 1),
        DataItem::bytes(CHAIN_SYMBOL, symbol.as_bytes().to_vec()),
        DataItem::bytes(DESCRIPTION, description.as_bytes().to_vec()),
        DataItem::bytes(COIN_COUNT, coin_bytes),
        DataItem::bytes(SHARES_OUT, shares_bytes.clone()),
        DataItem::bytes(FEE_RATE, fee_bytes),
        DataItem::bytes(EXPIRY_PERIOD, expiry_period.to_bytes().to_vec()),
        DataItem::vbc_value(EXPIRY_MODE, 1),
        DataItem::container(PARTICIPANT, vec![
            DataItem::bytes(ED25519_PUB, pubkey),
            DataItem::bytes(AMOUNT, shares_bytes),
        ]),
    ];
    let signable = DataItem::container(GENESIS, signable_children.clone());
    let sig = sign::sign_dataitem(&issuer_key, &signable, ts);

    let mut all_children = signable_children;
    all_children.push(DataItem::container(AUTH_SIG, vec![
        DataItem::bytes(ED25519_SIG, sig.to_vec()),
        DataItem::bytes(TIMESTAMP, ts.to_bytes().to_vec()),
    ]));

    // Compute chain ID hash: SHA256 of all children bytes
    let mut content_bytes = Vec::new();
    for child in &all_children {
        child.encode(&mut content_bytes);
    }
    let chain_hash = ao_crypto::hash::sha256(&content_bytes);
    all_children.push(DataItem::bytes(SHA256, chain_hash.to_vec()));

    let genesis = DataItem::container(GENESIS, all_children);
    let json = ao_json::to_json(&genesis);
    (genesis, json)
}
