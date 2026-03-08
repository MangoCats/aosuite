//! Optional Prometheus metrics for ao-recorder.
//!
//! Enabled via the `metrics` feature flag. Exposes a `GET /metrics` endpoint
//! returning Prometheus text format.

#[cfg(feature = "metrics")]
pub mod prom {
    use lazy_static::lazy_static;
    use prometheus::{
        self, Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGauge, Opts, Registry,
        TextEncoder,
    };

    lazy_static! {
        pub static ref REGISTRY: Registry = Registry::new_custom(
            Some("recorder".to_string()),
            None,
        ).expect("registry creation must succeed");

        /// Block submissions: total count by status (ok, error).
        pub static ref BLOCKS_SUBMITTED: IntCounterVec = {
            let opts = Opts::new("blocks_submitted_total", "Total block submissions")
                .namespace("recorder");
            let counter = IntCounterVec::new(opts, &["status"]).unwrap();
            REGISTRY.register(Box::new(counter.clone())).unwrap();
            counter
        };

        /// Block submission latency in seconds.
        pub static ref BLOCK_SUBMIT_DURATION: HistogramVec = {
            let opts = HistogramOpts::new(
                "block_submit_duration_seconds",
                "Block submission latency",
            )
            .namespace("recorder")
            .buckets(vec![0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0]);
            let hist = HistogramVec::new(opts, &["chain"]).unwrap();
            REGISTRY.register(Box::new(hist.clone())).unwrap();
            hist
        };

        /// Blob uploads: total count by status (ok, too_large, quota_exceeded, error).
        pub static ref BLOBS_UPLOADED: IntCounterVec = {
            let opts = Opts::new("blobs_uploaded_total", "Total blob uploads")
                .namespace("recorder");
            let counter = IntCounterVec::new(opts, &["status"]).unwrap();
            REGISTRY.register(Box::new(counter.clone())).unwrap();
            counter
        };

        /// Blob upload size in bytes (histogram).
        pub static ref BLOB_SIZE: HistogramVec = {
            let opts = HistogramOpts::new("blob_size_bytes", "Uploaded blob sizes")
                .namespace("recorder")
                .buckets(vec![1024.0, 10240.0, 102400.0, 524288.0, 1048576.0, 5242880.0]);
            let hist = HistogramVec::new(opts, &["chain"]).unwrap();
            REGISTRY.register(Box::new(hist.clone())).unwrap();
            hist
        };

        /// Active SSE connections (gauge).
        pub static ref SSE_CONNECTIONS: IntGauge = {
            let gauge = IntGauge::with_opts(
                Opts::new("sse_connections_active", "Active SSE connections")
                    .namespace("recorder"),
            ).unwrap();
            REGISTRY.register(Box::new(gauge.clone())).unwrap();
            gauge
        };

        /// Active WebSocket connections (gauge).
        pub static ref WS_CONNECTIONS: IntGauge = {
            let gauge = IntGauge::with_opts(
                Opts::new("ws_connections_active", "Active WebSocket connections")
                    .namespace("recorder"),
            ).unwrap();
            REGISTRY.register(Box::new(gauge.clone())).unwrap();
            gauge
        };

        /// Active chains hosted (gauge).
        pub static ref CHAINS_HOSTED: IntGauge = {
            let gauge = IntGauge::with_opts(
                Opts::new("chains_hosted", "Number of chains hosted")
                    .namespace("recorder"),
            ).unwrap();
            REGISTRY.register(Box::new(gauge.clone())).unwrap();
            gauge
        };

    }

    /// Encode all registered metrics as Prometheus text format.
    pub fn encode_metrics() -> String {
        let encoder = TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }
}

/// Axum handler for GET /metrics.
/// Returns 404 if the `metrics` feature is not enabled.
#[cfg(feature = "metrics")]
pub async fn metrics_handler() -> axum::response::Response {
    use axum::http::{header, StatusCode};
    use axum::response::IntoResponse;
    let body = prom::encode_metrics();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        body,
    )
        .into_response()
}

#[cfg(not(feature = "metrics"))]
pub async fn metrics_handler() -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_FOUND
}

// ── Instrumentation helpers (no-ops when feature disabled) ──────────

/// Record a successful block submission with latency.
#[cfg(feature = "metrics")]
pub fn record_block_submitted(chain: &str, duration_secs: f64) {
    prom::BLOCKS_SUBMITTED.with_label_values(&["ok"]).inc();
    prom::BLOCK_SUBMIT_DURATION
        .with_label_values(&[chain])
        .observe(duration_secs);
}

#[cfg(not(feature = "metrics"))]
pub fn record_block_submitted(_chain: &str, _duration_secs: f64) {}

/// Record a failed block submission.
#[cfg(feature = "metrics")]
pub fn record_block_submit_error() {
    prom::BLOCKS_SUBMITTED.with_label_values(&["error"]).inc();
}

#[cfg(not(feature = "metrics"))]
pub fn record_block_submit_error() {}

/// Record a blob upload.
#[cfg(feature = "metrics")]
pub fn record_blob_uploaded(chain: &str, size: usize, status: &str) {
    prom::BLOBS_UPLOADED.with_label_values(&[status]).inc();
    if status == "ok" {
        prom::BLOB_SIZE
            .with_label_values(&[chain])
            .observe(size as f64);
    }
}

#[cfg(not(feature = "metrics"))]
pub fn record_blob_uploaded(_chain: &str, _size: usize, _status: &str) {}

/// Increment/decrement SSE connection gauge.
#[cfg(feature = "metrics")]
pub fn sse_connected() {
    prom::SSE_CONNECTIONS.inc();
}

#[cfg(not(feature = "metrics"))]
pub fn sse_connected() {}

#[cfg(feature = "metrics")]
pub fn sse_disconnected() {
    prom::SSE_CONNECTIONS.dec();
}

#[cfg(not(feature = "metrics"))]
pub fn sse_disconnected() {}

/// Increment/decrement WebSocket connection gauge.
#[cfg(feature = "metrics")]
pub fn ws_connected() {
    prom::WS_CONNECTIONS.inc();
}

#[cfg(not(feature = "metrics"))]
pub fn ws_connected() {}

#[cfg(feature = "metrics")]
pub fn ws_disconnected() {
    prom::WS_CONNECTIONS.dec();
}

#[cfg(not(feature = "metrics"))]
pub fn ws_disconnected() {}

/// Update chains hosted gauge.
#[cfg(feature = "metrics")]
pub fn set_chains_hosted(count: i64) {
    prom::CHAINS_HOSTED.set(count);
}

#[cfg(not(feature = "metrics"))]
pub fn set_chains_hosted(_count: i64) {}

#[cfg(test)]
#[cfg(feature = "metrics")]
mod tests {
    use super::*;

    #[test]
    fn encode_metrics_returns_text() {
        record_block_submitted("test_chain", 0.042);
        record_block_submit_error();
        record_blob_uploaded("test_chain", 1024, "ok");
        sse_connected();
        set_chains_hosted(3);

        let output = prom::encode_metrics();
        assert!(output.contains("recorder_blocks_submitted_total"));
        assert!(output.contains("recorder_block_submit_duration_seconds"));
        assert!(output.contains("recorder_blobs_uploaded_total"));
        assert!(output.contains("recorder_sse_connections_active"));
        assert!(output.contains("recorder_chains_hosted"));
    }

    #[test]
    fn block_counter_increments() {
        let before = prom::BLOCKS_SUBMITTED
            .with_label_values(&["ok"])
            .get();
        record_block_submitted("c1", 0.01);
        let after = prom::BLOCKS_SUBMITTED
            .with_label_values(&["ok"])
            .get();
        assert_eq!(after, before + 1);
    }

    #[test]
    fn sse_gauge_tracks_connections() {
        let base = prom::SSE_CONNECTIONS.get();
        sse_connected();
        assert_eq!(prom::SSE_CONNECTIONS.get(), base + 1);
        sse_disconnected();
        assert_eq!(prom::SSE_CONNECTIONS.get(), base);
    }
}
