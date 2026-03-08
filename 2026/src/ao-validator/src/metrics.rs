//! Optional Prometheus metrics for ao-validator.
//!
//! Enabled via the `metrics` feature flag. Exposes a `GET /metrics` endpoint
//! returning Prometheus text format.

#[cfg(feature = "metrics")]
pub mod prom {
    use lazy_static::lazy_static;
    use prometheus::{
        self, Encoder, HistogramOpts, HistogramVec, IntCounterVec, IntGaugeVec, Opts, Registry,
        TextEncoder,
    };

    lazy_static! {
        pub static ref REGISTRY: Registry = Registry::new_custom(
            Some("validator".to_string()),
            None,
        ).expect("registry creation must succeed");

        /// Validation runs by chain and result (ok, alteration, unreachable, error).
        pub static ref RUNS: IntCounterVec = {
            let opts = Opts::new("runs_total", "Total validation runs")
                .namespace("validator");
            let counter = IntCounterVec::new(opts, &["chain", "result"]).unwrap();
            REGISTRY.register(Box::new(counter.clone())).unwrap();
            counter
        };

        /// Blocks verified per chain.
        pub static ref BLOCKS_VERIFIED: IntCounterVec = {
            let opts = Opts::new("blocks_verified_total", "Total blocks verified")
                .namespace("validator");
            let counter = IntCounterVec::new(opts, &["chain"]).unwrap();
            REGISTRY.register(Box::new(counter.clone())).unwrap();
            counter
        };

        /// Verification duration per chain (seconds).
        pub static ref VERIFY_DURATION: HistogramVec = {
            let opts = HistogramOpts::new(
                "verify_duration_seconds",
                "Verification batch duration",
            )
            .namespace("validator")
            .buckets(vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0]);
            let hist = HistogramVec::new(opts, &["chain"]).unwrap();
            REGISTRY.register(Box::new(hist.clone())).unwrap();
            hist
        };

        /// Current validated height per chain.
        pub static ref VALIDATED_HEIGHT: IntGaugeVec = {
            let gauge = IntGaugeVec::new(
                Opts::new("validated_height", "Current validated block height")
                    .namespace("validator"),
                &["chain"],
            ).unwrap();
            REGISTRY.register(Box::new(gauge.clone())).unwrap();
            gauge
        };

        /// Alerts dispatched by type (unreachable, recovered, alteration).
        pub static ref ALERTS: IntCounterVec = {
            let opts = Opts::new("alerts_total", "Alerts dispatched")
                .namespace("validator");
            let counter = IntCounterVec::new(opts, &["type"]).unwrap();
            REGISTRY.register(Box::new(counter.clone())).unwrap();
            counter
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

// ── Instrumentation helpers ─────────────────────────────────────────

/// Record a validation run result.
#[cfg(feature = "metrics")]
pub fn record_run(chain: &str, result: &str) {
    prom::RUNS.with_label_values(&[chain, result]).inc();
}

#[cfg(not(feature = "metrics"))]
pub fn record_run(_chain: &str, _result: &str) {}

/// Record blocks verified in a batch.
#[cfg(feature = "metrics")]
pub fn record_blocks_verified(chain: &str, count: u64) {
    prom::BLOCKS_VERIFIED
        .with_label_values(&[chain])
        .inc_by(count);
}

#[cfg(not(feature = "metrics"))]
pub fn record_blocks_verified(_chain: &str, _count: u64) {}

/// Record verification duration.
#[cfg(feature = "metrics")]
pub fn record_verify_duration(chain: &str, duration_secs: f64) {
    prom::VERIFY_DURATION
        .with_label_values(&[chain])
        .observe(duration_secs);
}

#[cfg(not(feature = "metrics"))]
pub fn record_verify_duration(_chain: &str, _duration_secs: f64) {}

/// Update validated height gauge.
#[cfg(feature = "metrics")]
pub fn set_validated_height(chain: &str, height: u64) {
    prom::VALIDATED_HEIGHT
        .with_label_values(&[chain])
        .set(height as i64);
}

#[cfg(not(feature = "metrics"))]
pub fn set_validated_height(_chain: &str, _height: u64) {}

/// Record an alert dispatch.
#[cfg(feature = "metrics")]
pub fn record_alert(alert_type: &str) {
    prom::ALERTS.with_label_values(&[alert_type]).inc();
}

#[cfg(not(feature = "metrics"))]
pub fn record_alert(_alert_type: &str) {}

#[cfg(test)]
#[cfg(feature = "metrics")]
mod tests {
    use super::*;

    #[test]
    fn encode_metrics_returns_text() {
        record_run("test_chain", "ok");
        record_blocks_verified("test_chain", 10);
        set_validated_height("test_chain", 42);
        record_alert("alteration");

        let output = prom::encode_metrics();
        assert!(output.contains("validator_runs_total"));
        assert!(output.contains("validator_blocks_verified_total"));
        assert!(output.contains("validator_validated_height"));
        assert!(output.contains("validator_alerts_total"));
    }

    #[test]
    fn run_counter_increments() {
        let before = prom::RUNS.with_label_values(&["c1", "ok"]).get();
        record_run("c1", "ok");
        let after = prom::RUNS.with_label_values(&["c1", "ok"]).get();
        assert_eq!(after, before + 1);
    }

    #[test]
    fn height_gauge_updates() {
        set_validated_height("c2", 100);
        assert_eq!(
            prom::VALIDATED_HEIGHT.with_label_values(&["c2"]).get(),
            100
        );
        set_validated_height("c2", 200);
        assert_eq!(
            prom::VALIDATED_HEIGHT.with_label_values(&["c2"]).get(),
            200
        );
    }
}
