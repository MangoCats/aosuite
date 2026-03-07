mod keygen;
mod genesis;
mod inspect;
mod balance;
mod assign;
mod accept;
mod refute;
mod history;
mod export;
mod caa_status;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ao", about = "Assign Onward CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new Ed25519 keypair
    Keygen(keygen::KeygenArgs),
    /// Create a genesis block
    Genesis(genesis::GenesisArgs),
    /// Inspect a binary-encoded DataItem
    Inspect(inspect::InspectArgs),
    /// Query UTXO balance on a chain
    Balance(balance::BalanceArgs),
    /// Build an assignment agreement
    Assign(assign::AssignArgs),
    /// Sign an assignment and submit to recorder
    Accept(accept::AcceptArgs),
    /// Build a refutation for an agreement
    Refute(refute::RefuteArgs),
    /// View block history from a chain
    History(history::HistoryArgs),
    /// Export blocks as JSON
    Export(export::ExportArgs),
    /// Query CAA escrow status on a chain
    CaaStatus(caa_status::CaaStatusArgs),
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Keygen(args) => keygen::run(args),
        Commands::Genesis(args) => genesis::run(args),
        Commands::Inspect(args) => inspect::run(args),
        Commands::Balance(args) => balance::run(args),
        Commands::Assign(args) => assign::run(args),
        Commands::Accept(args) => accept::run(args),
        Commands::Refute(args) => refute::run(args),
        Commands::History(args) => history::run(args),
        Commands::Export(args) => export::run(args),
        Commands::CaaStatus(args) => caa_status::run(args),
    }
}
