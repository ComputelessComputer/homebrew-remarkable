use crate::client::RemarkableClient;
use crate::config::ConfigStore;
use crate::error::{AppError, Result};
use crate::sync::SyncEngine;
use colored::Colorize;

pub async fn run(store: &ConfigStore, force: bool, dry_run: bool) -> Result<()> {
    let auth = store.load_auth().await?;
    if !auth.is_registered() {
        return Err(AppError::AuthRequired);
    }

    let config = store.load_config().await?;
    let state = store.load_state().await?;
    let sync_root = store.expand_home(&config.sync_folder)?;

    let client = RemarkableClient::new(auth.device_token);
    let mut engine = SyncEngine::new(client, config.clone(), state, sync_root.clone());
    let outcome = engine.sync(force, dry_run).await?;

    if !dry_run {
        store.save_state(&engine.into_state()).await?;
    }

    if dry_run {
        println!();
        println!("{}", "Dry run complete".yellow().bold());
        if outcome.dry_run_items.is_empty() {
            println!("Everything is already up to date.");
        } else {
            println!("{} documents would sync:", outcome.dry_run_items.len());
            for name in outcome.dry_run_items.iter().take(20) {
                println!("  - {name}");
            }
            if outcome.dry_run_items.len() > 20 {
                println!("  ... and {} more", outcome.dry_run_items.len() - 20);
            }
        }
        println!();
        return Ok(());
    }

    let report = outcome.report;
    println!();
    println!(
        "{} {} found, {} synced, {} skipped, {} failed",
        "Sync complete.".green().bold(),
        report.cloud_item_count,
        report.synced_count,
        report.skipped_count,
        report.failed_count
    );
    println!("Destination: {}", sync_root.display());
    if let Some(log_path) = report.log_path {
        println!("Log: {log_path}");
    }
    if report.failed_count > 0 {
        for failed in report.failed_items.iter().take(5) {
            println!("{} {}: {}", "failed".red(), failed.name, failed.error);
        }
    }
    println!();

    Ok(())
}
