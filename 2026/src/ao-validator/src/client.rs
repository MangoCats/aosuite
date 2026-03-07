use anyhow::{Context, Result};
use serde::Deserialize;

/// HTTP client for fetching chain data from a recorder.
pub struct RecorderClient {
    base_url: String,
    http: reqwest::Client,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChainInfo {
    pub chain_id: String,
    pub symbol: String,
    pub block_height: u64,
}

impl RecorderClient {
    pub fn new(base_url: &str) -> Self {
        RecorderClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    /// Get chain info (mainly to learn the current block height).
    pub async fn chain_info(&self, chain_id: &str) -> Result<ChainInfo> {
        let url = format!("{}/chain/{}/info", self.base_url, chain_id);
        let resp = self.http.get(&url).send().await
            .context("chain_info request failed")?;
        if !resp.status().is_success() {
            anyhow::bail!("chain_info failed: {}", resp.status());
        }
        resp.json().await.context("chain_info parse failed")
    }

    /// Fetch blocks as JSON array. Uses the recorder's paginated endpoint.
    pub async fn get_blocks(&self, chain_id: &str, from: u64, to: u64) -> Result<Vec<serde_json::Value>> {
        let url = format!(
            "{}/chain/{}/blocks?from={}&to={}",
            self.base_url, chain_id, from, to
        );
        let resp = self.http.get(&url).send().await
            .context("get_blocks request failed")?;
        if !resp.status().is_success() {
            anyhow::bail!("get_blocks failed: {}", resp.status());
        }
        resp.json().await.context("get_blocks parse failed")
    }
}
