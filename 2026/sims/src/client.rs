use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// HTTP client wrapper for the ao-recorder API.
pub struct RecorderClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChainInfo {
    pub chain_id: String,
    pub symbol: String,
    pub shares_out: String,
    pub coin_count: String,
    pub fee_rate_num: String,
    pub fee_rate_den: String,
    pub block_height: u64,
    pub next_seq_id: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UtxoInfo {
    pub seq_id: u64,
    pub pubkey: String,
    pub amount: String,
    pub status: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BlockResult {
    pub height: u64,
    pub hash: String,
    pub first_seq: u64,
    pub seq_count: u64,
}

#[derive(Deserialize, Debug)]
struct ErrorResponse {
    error: String,
}

impl RecorderClient {
    pub fn new(base_url: &str) -> Self {
        RecorderClient {
            base_url: base_url.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// POST /chains — create a new chain from genesis JSON.
    pub async fn create_chain(
        &self,
        genesis_json: &serde_json::Value,
    ) -> Result<ChainInfo> {
        let body = serde_json::json!({ "genesis": genesis_json });
        let resp = self.client
            .post(format!("{}/chains", self.base_url))
            .json(&body)
            .send()
            .await
            .context("failed to connect to recorder")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await
                .unwrap_or(ErrorResponse { error: "unknown".into() });
            bail!("create_chain failed: {}", err.error);
        }
        resp.json().await.context("invalid create_chain response")
    }

    /// GET /chain/{id}/info
    pub async fn chain_info(&self, chain_id: &str) -> Result<ChainInfo> {
        let resp = self.client
            .get(format!("{}/chain/{}/info", self.base_url, chain_id))
            .send()
            .await
            .context("failed to connect to recorder")?;

        if !resp.status().is_success() {
            bail!("chain_info failed: {}", resp.status());
        }
        resp.json().await.context("invalid chain_info response")
    }

    /// GET /chain/{id}/utxo/{seq_id}
    pub async fn get_utxo(&self, chain_id: &str, seq_id: u64) -> Result<UtxoInfo> {
        let resp = self.client
            .get(format!("{}/chain/{}/utxo/{}", self.base_url, chain_id, seq_id))
            .send()
            .await
            .context("failed to connect to recorder")?;

        if !resp.status().is_success() {
            bail!("get_utxo failed: {}", resp.status());
        }
        resp.json().await.context("invalid utxo response")
    }

    /// POST /chain/{id}/submit — submit a signed AUTHORIZATION.
    pub async fn submit(
        &self,
        chain_id: &str,
        auth_json: &serde_json::Value,
    ) -> Result<BlockResult> {
        let resp = self.client
            .post(format!("{}/chain/{}/submit", self.base_url, chain_id))
            .json(auth_json)
            .send()
            .await
            .context("failed to connect to recorder")?;

        if !resp.status().is_success() {
            let err: ErrorResponse = resp.json().await
                .unwrap_or(ErrorResponse { error: "unknown".into() });
            bail!("submit failed: {}", err.error);
        }
        resp.json().await.context("invalid submit response")
    }
}
