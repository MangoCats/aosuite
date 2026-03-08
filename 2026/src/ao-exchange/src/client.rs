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

    /// Submit a CAA for escrow recording. Returns recording proof JSON.
    pub async fn caa_submit(&self, chain_id: &str, caa_json: &serde_json::Value) -> Result<CaaProofResult> {
        let url = format!("{}/chain/{}/caa/submit", self.base_url, chain_id);
        let resp = self.http.post(&url)
            .json(caa_json)
            .send().await
            .context("caa_submit request failed")?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("caa_submit failed: {}", body);
        }
        resp.json().await.context("caa_submit parse failed")
    }

    /// Submit binding proof to finalize a CAA on a chain.
    pub async fn caa_bind(&self, chain_id: &str, bind_json: &serde_json::Value) -> Result<CaaStatusResult> {
        let url = format!("{}/chain/{}/caa/bind", self.base_url, chain_id);
        let resp = self.http.post(&url)
            .json(bind_json)
            .send().await
            .context("caa_bind request failed")?;
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("caa_bind failed: {}", body);
        }
        resp.json().await.context("caa_bind parse failed")
    }

    /// Subscribe to SSE block events. Returns a response that can be read
    /// as a chunked stream of SSE events.
    pub async fn subscribe_blocks(&self, chain_id: &str) -> Result<reqwest::Response> {
        let url = format!("{}/chain/{}/events", self.base_url, chain_id);
        let resp = self.http.get(&url).send().await
            .context("SSE subscribe failed")?;
        if !resp.status().is_success() {
            anyhow::bail!("SSE subscribe failed: {}", resp.status());
        }
        Ok(resp)
    }

    /// Query CAA escrow status.
    pub async fn caa_status(&self, chain_id: &str, caa_hash: &str) -> Result<CaaStatusResult> {
        let url = format!("{}/chain/{}/caa/{}", self.base_url, chain_id, caa_hash);
        let resp = self.http.get(&url).send().await
            .context("caa_status request failed")?;
        if !resp.status().is_success() {
            anyhow::bail!("caa_status failed: {}", resp.status());
        }
        resp.json().await.context("caa_status parse failed")
    }
}

/// SSE block event from recorder's /chain/{id}/events endpoint.
#[derive(Deserialize, Debug, Clone)]
pub struct BlockEvent {
    pub height: u64,
    pub hash: String,
    pub timestamp: i64,
    pub shares_out: String,
    pub first_seq: u64,
    pub seq_count: u64,
}

/// Result of parsing SSE text: events found and number of bytes consumed.
pub struct SseParseResult {
    pub events: Vec<BlockEvent>,
    /// Number of bytes consumed from the input (up to end of last complete event).
    pub consumed: usize,
}

/// Parse SSE text data into block events. SSE format: "event: block\ndata: {...}\n\n"
/// Returns parsed events and the byte offset consumed, so callers can retain
/// any trailing partial event data in their buffer.
pub fn parse_sse_events(text: &str) -> SseParseResult {
    let mut events = Vec::new();
    let mut data_line: Option<&str> = None;
    let mut last_consumed = 0;
    let mut pos = 0;
    for line in text.lines() {
        // Advance pos past this line + its newline delimiter
        let line_end = pos + line.len();
        let next_pos = if text[line_end..].starts_with("\r\n") {
            line_end + 2
        } else if line_end < text.len() {
            line_end + 1
        } else {
            line_end
        };

        if line.starts_with(':') {
            // SSE comment (keep-alive) — skip
        } else if line.starts_with("data: ") {
            data_line = Some(&line[6..]);
        } else if line.is_empty() {
            if let Some(data) = data_line.take() {
                if let Ok(event) = serde_json::from_str::<BlockEvent>(data) {
                    events.push(event);
                }
            }
            last_consumed = next_pos;
        }
        pos = next_pos;
    }
    SseParseResult { events, consumed: last_consumed }
}

#[derive(Deserialize, Debug, Clone)]
pub struct CaaProofResult {
    pub caa_hash: String,
    pub chain_id: String,
    pub block_height: u64,
    pub block_hash: String,
    pub first_seq: u64,
    pub seq_count: u64,
    pub proof_json: serde_json::Value,
}

#[derive(Deserialize, Debug, Clone)]
pub struct CaaStatusResult {
    pub caa_hash: String,
    pub status: String,
    pub chain_order: u64,
    pub deadline: i64,
    pub block_height: u64,
    pub has_proof: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_block_event() {
        let text = "event: block\ndata: {\"height\":42,\"hash\":\"abc\",\"timestamp\":1000,\"shares_out\":\"500\",\"first_seq\":0,\"seq_count\":3}\n\n";
        let result = parse_sse_events(text);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].height, 42);
        assert_eq!(result.events[0].hash, "abc");
        assert_eq!(result.events[0].timestamp, 1000);
        assert_eq!(result.events[0].seq_count, 3);
        assert_eq!(result.consumed, text.len());
    }

    #[test]
    fn parse_multiple_block_events() {
        let text = concat!(
            "event: block\n",
            "data: {\"height\":1,\"hash\":\"a\",\"timestamp\":100,\"shares_out\":\"10\",\"first_seq\":0,\"seq_count\":1}\n",
            "\n",
            "event: block\n",
            "data: {\"height\":2,\"hash\":\"b\",\"timestamp\":200,\"shares_out\":\"20\",\"first_seq\":1,\"seq_count\":2}\n",
            "\n",
        );
        let result = parse_sse_events(text);
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0].height, 1);
        assert_eq!(result.events[1].height, 2);
        assert_eq!(result.consumed, text.len());
    }

    #[test]
    fn parse_empty_text() {
        let result = parse_sse_events("");
        assert!(result.events.is_empty());
        assert_eq!(result.consumed, 0);
    }

    #[test]
    fn parse_ignores_malformed_json() {
        let text = "event: block\ndata: {not valid json}\n\n";
        let result = parse_sse_events(text);
        assert!(result.events.is_empty());
        // Consumed up to the blank line even though JSON was invalid
        assert_eq!(result.consumed, text.len());
    }

    #[test]
    fn parse_ignores_incomplete_event() {
        // No trailing blank line — event not yet complete
        let text = "event: block\ndata: {\"height\":1,\"hash\":\"a\",\"timestamp\":100,\"shares_out\":\"10\",\"first_seq\":0,\"seq_count\":1}\n";
        let result = parse_sse_events(text);
        assert!(result.events.is_empty());
        assert_eq!(result.consumed, 0);
    }

    #[test]
    fn parse_data_without_event_line() {
        let text = "data: {\"height\":5,\"hash\":\"x\",\"timestamp\":500,\"shares_out\":\"50\",\"first_seq\":0,\"seq_count\":0}\n\n";
        let result = parse_sse_events(text);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].height, 5);
        assert_eq!(result.consumed, text.len());
    }

    #[test]
    fn parse_retains_trailing_partial_data() {
        // Complete event followed by start of another (no trailing blank line)
        let complete = "data: {\"height\":1,\"hash\":\"a\",\"timestamp\":100,\"shares_out\":\"10\",\"first_seq\":0,\"seq_count\":1}\n\n";
        let partial = "data: {\"height\":2,\"hash\":\"b\",\"timestamp\":200,\"shares_out\":\"20\",\"first_seq\":1,\"seq_count\":2}\n";
        let text = format!("{}{}", complete, partial);
        let result = parse_sse_events(&text);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.consumed, complete.len());
        // Caller retains text[consumed..] which is the partial data
        assert_eq!(&text[result.consumed..], partial);
    }

    #[test]
    fn parse_skips_sse_comments() {
        let text = ": keep-alive\ndata: {\"height\":1,\"hash\":\"a\",\"timestamp\":100,\"shares_out\":\"10\",\"first_seq\":0,\"seq_count\":1}\n\n";
        let result = parse_sse_events(text);
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].height, 1);
    }
}
