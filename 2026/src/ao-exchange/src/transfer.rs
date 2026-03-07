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

/// Build, sign, and submit an assignment. Returns the recorded block info.
///
/// The last receiver's amount is automatically adjusted to satisfy the
/// balance equation: giver_total = receiver_total + fee.
pub async fn execute_transfer(
    client: &RecorderClient,
    chain_id: &str,
    givers: &[Giver],
    receivers: &mut [Receiver],
) -> Result<BlockResult> {
    if givers.is_empty() || receivers.is_empty() {
        bail!("transfer requires at least one giver and one receiver");
    }

    let info = client.chain_info(chain_id).await?;
    let shares_out: BigInt = info.shares_out.parse()?;
    let fee_num: BigInt = info.fee_rate_num.parse()?;
    let fee_den: BigInt = info.fee_rate_den.parse()?;

    let giver_total: BigInt = givers.iter().map(|g| &g.amount).sum();
    let fee_rate = num_rational::BigRational::new(fee_num.clone(), fee_den.clone());

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs() as i64;

    // Iterative fee convergence (3 rounds)
    for _ in 0..3 {
        let auth = build_authorization(givers, receivers, now_secs, &fee_rate);
        let page = DataItem::container(PAGE, vec![
            DataItem::vbc_value(PAGE_INDEX, 0),
            auth,
        ]);
        let page_bytes = page.to_bytes().len() as u64;
        let fee = fees::recording_fee(page_bytes, &fee_num, &fee_den, &shares_out);

        let other_total: BigInt = receivers[..receivers.len() - 1]
            .iter()
            .map(|r| &r.amount)
            .sum();
        let last_amount = &giver_total - &fee - &other_total;
        if last_amount <= BigInt::from(0) {
            bail!(
                "insufficient funds: giver_total={}, fee={}, other_receivers={}",
                giver_total, fee, other_total
            );
        }
        receivers.last_mut().expect("receivers is non-empty").amount = last_amount;
    }

    let auth = build_authorization(givers, receivers, now_secs, &fee_rate);
    let auth_json = ao_json::to_json(&auth);

    client.submit(chain_id, &auth_json).await
}

fn build_authorization(
    givers: &[Giver],
    receivers: &[Receiver],
    base_unix_secs: i64,
    fee_rate: &num_rational::BigRational,
) -> DataItem {
    let assignment = build_assignment(givers, receivers, fee_rate, base_unix_secs);
    let participant_count = givers.len() + receivers.len();
    let mut auth_children = vec![assignment.clone()];

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

fn build_assignment(
    givers: &[Giver],
    receivers: &[Receiver],
    fee_rate: &num_rational::BigRational,
    base_unix_secs: i64,
) -> DataItem {
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

    let mut bid_bytes = Vec::new();
    bigint::encode_rational(fee_rate, &mut bid_bytes);
    children.push(DataItem::bytes(RECORDING_BID, bid_bytes));

    let deadline_ts = Timestamp::from_unix_seconds(base_unix_secs + 86400);
    children.push(DataItem::bytes(DEADLINE, deadline_ts.to_bytes().to_vec()));

    DataItem::container(ASSIGNMENT, children)
}
