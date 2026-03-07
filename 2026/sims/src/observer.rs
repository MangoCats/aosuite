use std::collections::HashMap;
use tokio::sync::mpsc;

use crate::agents::AgentState;

/// Text-mode observer that prints a live dashboard of agent states.
pub async fn run_observer(
    mut state_rx: mpsc::Receiver<AgentState>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    let mut states: HashMap<String, AgentState> = HashMap::new();
    let mut last_print = std::time::Instant::now();

    loop {
        tokio::select! {
            state = state_rx.recv() => {
                match state {
                    Some(s) => { states.insert(s.name.clone(), s); }
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

fn print_dashboard(states: &HashMap<String, AgentState>) {
    if states.is_empty() {
        return;
    }

    // Clear screen and move cursor to top
    print!("\x1B[2J\x1B[H");

    println!("╔══════════════════════════════════════════════════════════════════════════╗");
    println!("║  AO Sims — Community Simulation                                        ║");
    println!("╠══════════════════════════════════════════════════════════════════════════╣");
    println!("║ {:10} │ {:8} │ {:6} │ {:>5} │ {:>4} │ {:24} ║",
        "Agent", "Role", "Status", "Txns", "UTXOs", "Last Action");
    println!("╟────────────┼──────────┼────────┼───────┼──────┼──────────────────────────╢");

    let mut sorted: Vec<_> = states.values().collect();
    sorted.sort_by_key(|s| match s.role.as_str() {
        "vendor" => 0,
        "exchange" => 1,
        "consumer" => 2,
        _ => 3,
    });

    for state in &sorted {
        let utxo_count: usize = state.chains.iter().map(|c| c.unspent_utxos).sum();
        let last_action = if state.last_action.len() > 24 {
            &state.last_action[..24]
        } else {
            &state.last_action
        };
        println!("║ {:10} │ {:8} │ {:6} │ {:>5} │ {:>4} │ {:24} ║",
            state.name, state.role, state.status,
            state.transactions, utxo_count, last_action);
    }

    println!("╠══════════════════════════════════════════════════════════════════════════╣");

    // Chain summary
    let mut chain_map: HashMap<String, (String, num_bigint::BigInt, usize)> = HashMap::new();
    for state in &sorted {
        for chain in &state.chains {
            let entry = chain_map.entry(chain.chain_id.clone())
                .or_insert_with(|| (chain.symbol.clone(), num_bigint::BigInt::from(0), 0));
            entry.2 += chain.unspent_utxos;
        }
    }

    for (chain_id, (symbol, _, utxos)) in &chain_map {
        let short_id = if chain_id.len() > 12 { &chain_id[..12] } else { chain_id };
        println!("║ Chain: {} ({}) — {} active UTXOs {:>24} ║",
            symbol, short_id, utxos, "");
    }

    let total_txns: u64 = sorted.iter().map(|s| s.transactions).sum();
    println!("╟──────────────────────────────────────────────────────────────────────────╢");
    println!("║ Total transactions: {:53} ║", total_txns);
    println!("╚══════════════════════════════════════════════════════════════════════════╝");
}
