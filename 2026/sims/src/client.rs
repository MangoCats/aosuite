use anyhow::{Context, Result, bail};
use serde::Deserialize;

/// HTTP client wrapper for the ao-recorder API.
pub struct RecorderClient {
    base_url: String,
    client: reqwest::Client,
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)] // Deserialized from JSON; not all fields read yet
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
#[allow(dead_code)] // Deserialized from JSON; not all fields read yet
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

    /// GET /chains — list all chains on the recorder.
    pub async fn list_chains(&self) -> Result<Vec<ChainListEntry>> {
        let resp = self.client
            .get(format!("{}/chains", self.base_url))
            .send()
            .await
            .context("failed to connect to recorder")?;

        if !resp.status().is_success() {
            bail!("list_chains failed: {}", resp.status());
        }
        resp.json().await.context("invalid list_chains response")
    }

    /// GET /chain/{id}/blocks?from=&to= — fetch blocks as JSON array.
    pub async fn get_blocks(&self, chain_id: &str, from: u64, to: u64) -> Result<Vec<serde_json::Value>> {
        let resp = self.client
            .get(format!("{}/chain/{}/blocks?from={}&to={}", self.base_url, chain_id, from, to))
            .send()
            .await
            .context("failed to connect to recorder")?;

        if !resp.status().is_success() {
            bail!("get_blocks failed: {}", resp.status());
        }
        resp.json().await.context("invalid get_blocks response")
    }
}

#[derive(Deserialize, Debug, Clone)]
#[allow(dead_code)]
pub struct ChainListEntry {
    pub chain_id: String,
    pub symbol: String,
    pub block_height: u64,
}
