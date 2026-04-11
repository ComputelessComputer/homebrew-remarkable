use crate::config::ConfigStore;
use crate::error::Result;
use colored::Colorize;

pub async fn run(store: &ConfigStore, auth: bool) -> Result<()> {
    store.clear_state().await?;
    println!("{}", "Sync state cleared.".green().bold());
    println!("The next sync will re-download all documents.");

    if auth {
        store.clear_auth().await?;
        println!("{}", "Device authentication removed.".green().bold());
        println!("Run `remarkable init` to reconnect.");
    }

    println!();
    Ok(())
}
