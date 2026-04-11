mod client;
mod commands;
mod config;
mod error;
mod sync;

use clap::{Parser, Subcommand};
use colored::Colorize;
use commands::{init, reset, status, sync as sync_command};
use config::ConfigStore;
use error::Result;

#[derive(Parser, Debug)]
#[command(
    name = "remarkable",
    version,
    about = "Sync your reMarkable documents locally"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Init,
    Sync {
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
    },
    Status,
    Reset {
        #[arg(long)]
        auth: bool,
    },
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{} {}", "error:".red().bold(), err);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    let store = ConfigStore::new()?;

    match cli.command {
        Some(Command::Init) => init::run(&store).await,
        Some(Command::Sync { force, dry_run }) => sync_command::run(&store, force, dry_run).await,
        Some(Command::Status) => status::run(&store).await,
        Some(Command::Reset { auth }) => reset::run(&store, auth).await,
        None => {
            let auth = store.load_auth().await?;
            if auth.is_registered() {
                sync_command::run(&store, false, false).await
            } else {
                init::run(&store).await
            }
        }
    }
}
