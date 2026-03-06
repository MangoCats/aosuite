mod keygen;
mod genesis;
mod inspect;

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
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Keygen(args) => keygen::run(args),
        Commands::Genesis(args) => genesis::run(args),
        Commands::Inspect(args) => inspect::run(args),
    }
}
