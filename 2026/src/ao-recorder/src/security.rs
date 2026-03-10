//! Security middleware: API key authentication and per-IP rate limiting.
//!
//! API keys are optional — when none are configured, all requests pass through.
//! Rate limiting is per source IP with separate limits for read vs write endpoints.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use axum::http::{Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

// ── API Key Authentication ──────────────────────────────────────────

/// Shared set of valid API keys. Empty = no auth required.
#[derive(Clone)]
pub struct ApiKeys(Arc<Vec<String>>);

impl ApiKeys {
    pub fn new(keys: Vec<String>) -> Self {
        ApiKeys(Arc::new(keys))
    }

    pub fn is_enforced(&self) -> bool {
        !self.0.is_empty()
    }

    pub fn is_valid(&self, key: &str) -> bool {
        self.0.iter().any(|k| constant_time_eq(k.as_bytes(), key.as_bytes()))
    }
}

/// Constant-time byte comparison to prevent timing attacks on API key validation.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Middleware that checks `Authorization: Bearer <key>` header.
/// Skips check if no API keys are configured.
pub async fn require_api_key(
    keys: ApiKeys,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if !keys.is_enforced() {
        return next.run(req).await;
    }

    let auth_header = req.headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(h) if h.starts_with("Bearer ") => {
            let token = &h[7..];
            if keys.is_valid(token) {
                next.run(req).await
            } else {
                (StatusCode::UNAUTHORIZED, "invalid API key").into_response()
            }
        }
        _ => (StatusCode::UNAUTHORIZED, "missing Authorization: Bearer <key>").into_response(),
    }
}

// ── Per-IP Rate Limiting ────────────────────────────────────────────

/// Token bucket state for a single IP.
struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

/// Per-IP rate limiter using a token bucket algorithm.
/// Tokens refill at `rate_per_second` continuously; bursts up to `rate_per_second` tokens.
#[derive(Clone)]
pub struct RateLimiter {
    state: Arc<Mutex<HashMap<IpAddr, Bucket>>>,
    rate_per_second: f64,
}

impl RateLimiter {
    pub fn new(rate_per_second: f64) -> Self {
        RateLimiter {
            state: Arc::new(Mutex::new(HashMap::new())),
            rate_per_second,
        }
    }

    /// Try to consume one token for the given IP. Returns true if allowed.
    pub fn check(&self, ip: IpAddr) -> bool {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return true, // poisoned → allow (fail open)
        };

        let now = Instant::now();
        let bucket = state.entry(ip).or_insert(Bucket {
            tokens: self.rate_per_second,
            last_refill: now,
        });

        // Refill tokens based on elapsed time
        let elapsed = now.duration_since(bucket.last_refill).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.rate_per_second)
            .min(self.rate_per_second);
        bucket.last_refill = now;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Remove stale entries (no activity for >60s). Call periodically.
    pub fn cleanup(&self) {
        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let now = Instant::now();
        state.retain(|_, b| now.duration_since(b.last_refill).as_secs() < 60);
    }
}

/// Rate limiting middleware. Returns 429 if rate exceeded.
/// Extracts client IP from ConnectInfo extensions if available.
pub async fn rate_limit(
    limiter: RateLimiter,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let ip = req.extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|ci| ci.0.ip())
        .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));

    if limiter.check(ip) {
        next.run(req).await
    } else {
        (StatusCode::TOO_MANY_REQUESTS, "rate limit exceeded").into_response()
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_keys_empty_not_enforced() {
        let keys = ApiKeys::new(vec![]);
        assert!(!keys.is_enforced());
    }

    #[test]
    fn test_api_keys_valid_check() {
        let keys = ApiKeys::new(vec!["secret123".into(), "key456".into()]);
        assert!(keys.is_enforced());
        assert!(keys.is_valid("secret123"));
        assert!(keys.is_valid("key456"));
        assert!(!keys.is_valid("wrong"));
    }

    #[test]
    fn test_rate_limiter_allows_burst() {
        let limiter = RateLimiter::new(5.0);
        let ip = IpAddr::V4(std::net::Ipv4Addr::new(1, 2, 3, 4));
        // First 5 requests should pass (burst capacity)
        for _ in 0..5 {
            assert!(limiter.check(ip));
        }
        // 6th should fail (tokens exhausted)
        assert!(!limiter.check(ip));
    }

    #[test]
    fn test_rate_limiter_different_ips_independent() {
        let limiter = RateLimiter::new(1.0);
        let ip1 = IpAddr::V4(std::net::Ipv4Addr::new(1, 1, 1, 1));
        let ip2 = IpAddr::V4(std::net::Ipv4Addr::new(2, 2, 2, 2));
        assert!(limiter.check(ip1));
        assert!(limiter.check(ip2));
        // Both should be allowed even though ip1 used a token
    }

    #[test]
    fn test_constant_time_eq_basics() {
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));   // different lengths
        assert!(!constant_time_eq(b"ab", b"abc"));
        assert!(!constant_time_eq(b"abc", b""));
    }

    #[test]
    fn test_api_key_wrong_length_rejected() {
        // Ensures keys of different lengths are rejected (even if prefix matches)
        let keys = ApiKeys::new(vec!["secret123".into()]);
        assert!(!keys.is_valid("secret12"));   // shorter
        assert!(!keys.is_valid("secret1234")); // longer
        assert!(!keys.is_valid(""));           // empty
    }

    #[test]
    fn test_rate_limiter_cleanup() {
        let limiter = RateLimiter::new(1.0);
        let ip = IpAddr::V4(std::net::Ipv4Addr::new(1, 1, 1, 1));
        limiter.check(ip);
        // State should have one entry
        assert_eq!(limiter.state.lock().unwrap().len(), 1);
        // Cleanup with recent activity should keep it
        limiter.cleanup();
        assert_eq!(limiter.state.lock().unwrap().len(), 1);
    }
}
