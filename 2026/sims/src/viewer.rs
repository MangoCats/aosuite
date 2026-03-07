use std::sync::Arc;

use axum::{
    extract::{Path, Query, State, WebSocketUpgrade, ws},
    http::Method,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};

use crate::agents::ViewerState;

#[derive(Clone)]
pub struct ViewerAppState {
    pub viewer: Arc<ViewerState>,
}

pub fn build_viewer_router(state: ViewerAppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET])
        .allow_headers(Any);

    Router::new()
        .route("/api/agents", get(list_agents))
        .route("/api/agents/{name}", get(get_agent))
        .route("/api/chains", get(list_chains))
        .route("/api/transactions", get(list_transactions))
        .route("/api/agents/{name}/transactions", get(agent_transactions))
        .route("/api/ws", get(ws_handler))
        .layer(cors)
        .with_state(state)
}

async fn list_agents(
    State(state): State<ViewerAppState>,
) -> Json<Vec<crate::agents::AgentState>> {
    Json(state.viewer.get_agents().await)
}

async fn get_agent(
    State(state): State<ViewerAppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.viewer.get_agent(&name).await {
        Some(agent) => Json(serde_json::json!(agent)).into_response(),
        None => (axum::http::StatusCode::NOT_FOUND, "agent not found").into_response(),
    }
}

#[derive(Serialize)]
struct ChainSummary {
    chain_id: String,
    symbol: String,
    total_utxos: usize,
    agents: Vec<String>,
}

async fn list_chains(
    State(state): State<ViewerAppState>,
) -> Json<Vec<ChainSummary>> {
    let agents = state.viewer.get_agents().await;
    let mut chains: std::collections::HashMap<String, ChainSummary> = std::collections::HashMap::new();

    for agent in &agents {
        for holding in &agent.chains {
            let entry = chains.entry(holding.chain_id.clone()).or_insert_with(|| ChainSummary {
                chain_id: holding.chain_id.clone(),
                symbol: holding.symbol.clone(),
                total_utxos: 0,
                agents: Vec::new(),
            });
            entry.total_utxos += holding.unspent_utxos;
            entry.agents.push(agent.name.clone());
        }
    }

    let mut result: Vec<ChainSummary> = chains.into_values().collect();
    result.sort_by(|a, b| a.symbol.cmp(&b.symbol));
    Json(result)
}

#[derive(Deserialize)]
struct TxQuery {
    since: Option<u64>,
    limit: Option<usize>,
    agent: Option<String>,
}

async fn list_transactions(
    State(state): State<ViewerAppState>,
    Query(q): Query<TxQuery>,
) -> Json<Vec<crate::agents::TransactionEvent>> {
    let since = q.since.unwrap_or(0);
    let limit = q.limit.unwrap_or(200).min(1000);
    if let Some(agent) = q.agent {
        let all = state.viewer.get_agent_transactions(&agent).await;
        Json(all.into_iter().filter(|t| t.id > since).take(limit).collect())
    } else {
        Json(state.viewer.get_transactions(since, limit).await)
    }
}

async fn agent_transactions(
    State(state): State<ViewerAppState>,
    Path(name): Path<String>,
) -> Json<Vec<crate::agents::TransactionEvent>> {
    Json(state.viewer.get_agent_transactions(&name).await)
}

async fn ws_handler(
    State(state): State<ViewerAppState>,
    ws_upgrade: WebSocketUpgrade,
) -> impl IntoResponse {
    ws_upgrade.on_upgrade(move |socket| ws_connection(socket, state))
}

async fn ws_connection(mut socket: ws::WebSocket, state: ViewerAppState) {
    let mut rx = state.viewer.subscribe.clone();

    // Send initial snapshot
    let agents = state.viewer.get_agents().await;
    let txns = state.viewer.get_transactions(0, 10_000).await;
    // Track highest tx_id sent so updates don't re-send old transactions
    let mut last_sent_id = txns.last().map_or(0, |t| t.id);
    let snapshot = serde_json::json!({
        "type": "snapshot",
        "agents": agents,
        "transactions": txns,
    });
    if socket.send(ws::Message::Text(snapshot.to_string().into())).await.is_err() {
        return;
    }

    // Stream incremental updates
    while let Ok(()) = rx.changed().await {
        let _ = *rx.borrow();
        let agents = state.viewer.get_agents().await;
        let new_txns = state.viewer.get_transactions(last_sent_id, 100).await;
        if let Some(last) = new_txns.last() {
            last_sent_id = last.id;
        }

        let update = serde_json::json!({
            "type": "update",
            "agents": agents,
            "transactions": new_txns,
        });
        if socket.send(ws::Message::Text(update.to_string().into())).await.is_err() {
            break;
        }
    }
}
