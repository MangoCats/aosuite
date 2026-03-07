use anyhow::{Context, Result};
use serde::Deserialize;

/// Lightweight HTTP client for talking to an ao-recorder instance.
pub struct RecorderClient {
    base_url: String,
    http: reqwest::Client,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChainInfo {
    pub chain_id: String,
    pub symbol: String,
    pub block_height: u64,
    pub shares_out: String,
    pub coin_count: String,
    pub fee_rate_num: String,
    pub fee_rate_den: String,
    pub next_seq_id: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ChainListEntry {
    pub chain_id: String,
    pub symbol: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct BlockResult {
    pub height: u64,
    pub hash: String,
    pub first_seq: u64,
    pub seq_count: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct UtxoInfo {
    pub seq_id: u64,
    pub pubkey: String,
    pub amount: String,
    pub status: String,
}

impl RecorderClient {
    pub fn new(base_url: &str) -> Self {
        RecorderClient {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: reqwest::Client::new(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// List all chains on this recorder.
    pub async fn list_chains(&self) -> Result<Vec<ChainListEntry>> {
        let url = format!("{}/chains", self.base_url);
        let resp = self.http.get(&url).send().await
            .context("list_chains request failed")?;
        resp.json().await.context("list_chains parse failed")
    }

    /// Get chain info.
    pub async fn chain_info(&self, chain_id: &str) -> Result<ChainInfo> {
        let url = format!("{}/chain/{}/info", self.base_url, chain_id);
        let resp = self.http.get(&url).send().await
            .context("chain_info request failed")?;
        if !resp.status().is_success() {
            anyhow::bail!("chain_info failed: {}", resp.status());
        }
        resp.json().await.context("chain_info parse failed")
    }

    /// Get UTXO info.
    pub async fn get_utxo(&self, chain_id: &str, seq_id: u64) -> Result<UtxoInfo> {
        let url = format!("{}/chain/{}/utxo/{}", self.base_url, chain_id, seq_id);
        let resp = self.http.get(&url).send().await
            .context("get_utxo request failed")?;
        if !resp.status().is_success() {
            anyhow::bail!("get_utxo failed: {}", resp.status());
        }
        resp.json().await.context("get_utxo parse failed")
    }

    /// Submit an authorization JSON and return block result.
    pub async fn submit(&self, chain_id: &str, json: &serde_json::Value) -> Result<BlockResult> {
        let url = format!("{}/chain/{}/submit", self.base_url, chain_id);
        let resp = self.http.post(&url)
            .json(json)
            .send().await
            .context("submit request failed")?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("submit failed: {}", body);
        }
        resp.json().await.context("submit parse failed")
    }
}
