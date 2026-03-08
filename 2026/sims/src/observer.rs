use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::agents::{AgentState, ViewerEvent, ViewerState};

/// Text-mode observer that prints a live dashboard of agent states.
pub async fn run_observer(
    mut state_rx: mpsc::Receiver<ViewerEvent>,
    viewer_state: Arc<ViewerState>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut states: HashMap<String, AgentState> = HashMap::new();
    let mut last_print = std::time::Instant::now();

    loop {
        tokio::select! {
            event = state_rx.recv() => {
                match event {
                    Some(ViewerEvent::State(s)) => {
                        viewer_state.update_agent((*s).clone()).await;
                        states.insert(s.name.clone(), *s);
                    }
                    Some(ViewerEvent::Transaction(t)) => {
                        viewer_state.add_transaction(t).await;
                    }
                    None => break,
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() { break; }
            }
        }

        // Print at most every 2 seconds
        if last_print.elapsed() >= std::time::Duration::from_secs(2) {
            print_dashboard(&states);
            last_print = std::time::Instant::now();
        }
    }

    // Final print
    print_dashboard(&states);
}

/// Print a line inside the dashboard box, truncating if longer than width.
fn print_line(w: usize, line: &str) {
    if line.len() > w {
        println!("║{}║", &line[..w]);
    } else {
        println!("║{:w$}║", line);
    }
}

fn format_shares(shares: &num_bigint::BigInt) -> String {
    let s = shares.to_string();
    if s.len() <= 12 {
        s
    } else {
        // Show as e.g. "1.09e12"
        let digits = s.len();
        format!("{}.{}e{}", &s[..1], &s[1..3], digits - 1)
    }
}

fn print_dashboard(states: &HashMap<String, AgentState>) {
    if states.is_empty() {
        return;
    }

    const W: usize = 88; // inner width between borders

    // Clear screen and move cursor to top
    print!("\x1B[2J\x1B[H");

    println!("╔{:═>W$}╗", "");
    println!("║{:^W$}║", "AO Sims — Community Simulation");
    println!("╠{:═>W$}╣", "");
    println!("║ {:10} │ {:8} │ {:6} │ {:>5} │ {:>5} │ {:>12} │ {:20} ║",
        "Agent", "Role", "Status", "Txns", "UTXOs", "Shares", "Last Action");
    println!("╟{:─>W$}╢", "");

    let mut sorted: Vec<_> = states.values().collect();
    sorted.sort_by_key(|s| match s.role.as_str() {
        "vendor" => 0,
        "exchange" => 1,
        "consumer" => 2,
        "validator" => 3,
        "attacker" => 4,
        _ => 5,
    });

    for state in &sorted {
        let utxo_count: usize = state.chains.iter().map(|c| c.unspent_utxos).sum();
        let total_shares: num_bigint::BigInt = state.chains.iter().map(|c| &c.shares).sum();
        let shares_str = format_shares(&total_shares);
        let last_action = if state.last_action.len() > 20 {
            &state.last_action[..20]
        } else {
            &state.last_action
        };
        println!("║ {:10} │ {:8} │ {:6} │ {:>5} │ {:>5} │ {:>12} │ {:20} ║",
            state.name, state.role, state.status,
            state.transactions, utxo_count, shares_str, last_action);
    }

    println!("╠{:═>W$}╣", "");

    // Chain summary
    let mut chain_map: HashMap<String, (String, usize)> = HashMap::new();
    for state in &sorted {
        for chain in &state.chains {
            let entry = chain_map.entry(chain.chain_id.clone())
                .or_insert_with(|| (chain.symbol.clone(), 0));
            entry.1 += chain.unspent_utxos;
        }
    }

    for (chain_id, (symbol, utxos)) in &chain_map {
        let short_id = if chain_id.len() > 12 { &chain_id[..12] } else { chain_id };
        let line = format!(" Chain: {} ({}) — {} active UTXOs", symbol, short_id, utxos);
        print_line(W, &line);
    }

    // Validator summaries
    for state in &sorted {
        if let Some(ref vs) = state.validator_status {
            let line = format!(" {} — {} chains, {} verified, {} alerts",
                state.name, vs.monitored_chains.len(), vs.total_blocks_verified, vs.alerts.len());
            print_line(W, &line);
        }
    }

    // Attacker summaries
    for state in &sorted {
        if let Some(ref atk) = state.attacker_status {
            let status = if atk.unexpected_accepts > 0 { "FAIL" } else { "ok" };
            let line = format!(" {} [{}] — {}/{} rejected, {} bad [{}]",
                state.name, atk.attack_type, atk.rejections, atk.attempts, atk.unexpected_accepts, status);
            print_line(W, &line);
        }
    }

    let total_txns: u64 = sorted.iter().map(|s| s.transactions).sum();
    println!("╟{:─>W$}╢", "");
    let txn_line = format!(" Total transactions: {}", total_txns);
    println!("║{:W$}║", txn_line);
    println!("╚{:═>W$}╝", "");
}
