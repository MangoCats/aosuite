//! ao-relay: Minimal WebSocket relay for AO wallet sync.
//!
//! Forwards encrypted blobs between paired devices sharing a wallet_id.
//! The relay never sees plaintext — all payloads are end-to-end encrypted.
//!
//! Spec: specs/WalletSync.md §4.2

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

/// Relay message as received/sent over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(not(test), allow(dead_code))]
struct RelayMessage {
    from: String,
    seq: u64,
    payload: String, // base64 encrypted blob
}

/// Per-wallet channel state.
struct WalletChannel {
    tx: broadcast::Sender<String>,
    last_activity: Instant,
    retained: Vec<String>, // retained messages for offline devices
}

/// Shared relay state.
struct RelayState {
    channels: RwLock<HashMap<String, WalletChannel>>,
    max_retained: usize,
    max_message_size: usize,
    retention_secs: u64,
}

impl RelayState {
    fn new(max_retained: usize, max_message_size: usize, retention_secs: u64) -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            max_retained,
            max_message_size,
            retention_secs,
        }
    }

    /// Get or create a broadcast channel for a wallet_id.
    async fn get_channel(&self, wallet_id: &str) -> broadcast::Sender<String> {
        // Try read lock first
        {
            let channels = self.channels.read().await;
            if let Some(ch) = channels.get(wallet_id) {
                return ch.tx.clone();
            }
        }
        // Create channel under write lock
        let mut channels = self.channels.write().await;
        let entry = channels.entry(wallet_id.to_string()).or_insert_with(|| {
            let (tx, _) = broadcast::channel(256);
            WalletChannel {
                tx,
                last_activity: Instant::now(),
                retained: Vec::new(),
            }
        });
        entry.last_activity = Instant::now();
        entry.tx.clone()
    }

    /// Store a message for offline delivery.
    async fn retain_message(&self, wallet_id: &str, msg: String) {
        let mut channels = self.channels.write().await;
        if let Some(ch) = channels.get_mut(wallet_id) {
            if ch.retained.len() < self.max_retained {
                ch.retained.push(msg);
            }
            ch.last_activity = Instant::now();
        }
    }

    /// Take retained messages for a wallet.
    async fn take_retained(&self, wallet_id: &str) -> Vec<String> {
        let mut channels = self.channels.write().await;
        if let Some(ch) = channels.get_mut(wallet_id) {
            ch.last_activity = Instant::now();
            std::mem::take(&mut ch.retained)
        } else {
            Vec::new()
        }
    }

    /// Clean up stale channels.
    async fn cleanup(&self) {
        let cutoff = Duration::from_secs(self.retention_secs);
        let mut channels = self.channels.write().await;
        channels.retain(|_, ch| ch.last_activity.elapsed() < cutoff);
    }
}

#[derive(Parser)]
#[command(name = "ao-relay", about = "AO wallet sync relay server")]
struct Args {
    /// Listen address
    #[arg(short, long, default_value = "0.0.0.0:3001")]
    listen: String,

    /// Maximum retained messages per wallet
    #[arg(long, default_value = "1000")]
    max_retained: usize,

    /// Maximum message size in bytes
    #[arg(long, default_value = "65536")]
    max_message_size: usize,

    /// Retention period in seconds (default 72 hours)
    #[arg(long, default_value = "259200")]
    retention_secs: u64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "ao_relay=info".into()),
        )
        .init();

    let args = Args::parse();
    let state = Arc::new(RelayState::new(
        args.max_retained,
        args.max_message_size,
        args.retention_secs,
    ));

    // Background cleanup task
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            cleanup_state.cleanup().await;
        }
    });

    let app = Router::new()
        .route("/ws/{wallet_id}", get(ws_handler))
        .route("/health", get(health_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&args.listen)
        .await
        .expect("failed to bind");
    info!("ao-relay listening on {}", args.listen);
    axum::serve(listener, app).await.expect("server error");
}

async fn health_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({ "status": "ok" }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Path(wallet_id): Path<String>,
    State(state): State<Arc<RelayState>>,
) -> impl IntoResponse {
    info!(wallet_id = %wallet_id, "WebSocket connection");
    ws.on_upgrade(move |socket| handle_socket(socket, wallet_id, state))
}

async fn handle_socket(mut socket: WebSocket, wallet_id: String, state: Arc<RelayState>) {
    let tx = state.get_channel(&wallet_id).await;
    let mut rx = tx.subscribe();

    // Send retained messages to the newly connected client
    let retained = state.take_retained(&wallet_id).await;
    for msg in retained {
        if socket.send(Message::Text(msg.into())).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            // Incoming message from this client → broadcast to others + retain
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let text_str: &str = &text;
                        if text_str.len() > state.max_message_size {
                            warn!(wallet_id = %wallet_id, "message too large, dropping");
                            continue;
                        }
                        // Validate it's valid JSON (but don't inspect contents)
                        if serde_json::from_str::<serde_json::Value>(text_str).is_err() {
                            warn!(wallet_id = %wallet_id, "invalid JSON, dropping");
                            continue;
                        }
                        let msg_str = text_str.to_string();
                        let _ = tx.send(msg_str.clone());
                        state.retain_message(&wallet_id, msg_str).await;
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {} // ignore binary, ping, pong
                }
            }
            // Broadcast message from another client → forward to this client
            result = rx.recv() => {
                match result {
                    Ok(msg) => {
                        if socket.send(Message::Text(msg.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        // Notify client about skipped messages
                        let lag_msg = serde_json::json!({
                            "event": "lagged",
                            "skipped": n
                        });
                        let _ = socket.send(Message::Text(lag_msg.to_string().into())).await;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    info!(wallet_id = %wallet_id, "WebSocket disconnected");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_message_roundtrip() {
        let msg = RelayMessage {
            from: "device123".to_string(),
            seq: 42,
            payload: "base64data==".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: RelayMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.from, "device123");
        assert_eq!(parsed.seq, 42);
        assert_eq!(parsed.payload, "base64data==");
    }

    #[tokio::test]
    async fn relay_state_channel_creation() {
        let state = RelayState::new(100, 65536, 3600);
        let tx1 = state.get_channel("wallet_abc").await;
        let tx2 = state.get_channel("wallet_abc").await;
        // Same channel — receiver count should be consistent
        assert_eq!(tx1.receiver_count(), tx2.receiver_count());
    }

    #[tokio::test]
    async fn relay_state_retained_messages() {
        let state = RelayState::new(5, 65536, 3600);
        let _ = state.get_channel("w1").await;

        state.retain_message("w1", "msg1".to_string()).await;
        state.retain_message("w1", "msg2".to_string()).await;

        let retained = state.take_retained("w1").await;
        assert_eq!(retained.len(), 2);
        assert_eq!(retained[0], "msg1");

        // After take, retained should be empty
        let retained2 = state.take_retained("w1").await;
        assert_eq!(retained2.len(), 0);
    }

    #[tokio::test]
    async fn relay_state_max_retained() {
        let state = RelayState::new(2, 65536, 3600);
        let _ = state.get_channel("w2").await;

        state.retain_message("w2", "a".to_string()).await;
        state.retain_message("w2", "b".to_string()).await;
        state.retain_message("w2", "c".to_string()).await; // should be dropped

        let retained = state.take_retained("w2").await;
        assert_eq!(retained.len(), 2);
    }

    #[tokio::test]
    async fn relay_state_cleanup() {
        let state = RelayState::new(100, 65536, 0); // 0 = immediate expiry
        let _ = state.get_channel("stale").await;

        // Wait a tiny bit so it expires
        tokio::time::sleep(Duration::from_millis(10)).await;
        state.cleanup().await;

        let channels = state.channels.read().await;
        assert!(channels.is_empty());
    }
}
