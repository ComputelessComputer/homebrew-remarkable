use crate::config::{ConfigStore, OutputFormat};
use crate::error::Result;
use colored::Colorize;
use std::time::{SystemTime, UNIX_EPOCH};

pub async fn run(store: &ConfigStore) -> Result<()> {
    let auth = store.load_auth().await?;
    let config = store.load_config().await?;
    let state = store.load_state().await?;
    let sync_root = store.expand_home(&config.sync_folder)?;

    println!();
    if auth.is_registered() {
        println!("{} Registered", "●".green());
    } else {
        println!("{} Not registered", "●".red());
        println!("Run `remarkable init` to connect.");
        println!();
        return Ok(());
    }

    println!("Sync folder: {}", sync_root.display().to_string().cyan());
    println!(
        "Output format: {}",
        match config.output_format {
            OutputFormat::Pdf => "pdf",
            OutputFormat::Markdown => "markdown",
        }
    );

    match state.last_sync_timestamp {
        Some(timestamp) => println!("Last sync: {}", relative_time(timestamp)),
        None => println!("Last sync: never"),
    }

    let document_count = state
        .records
        .values()
        .filter(|record| record.item_type == "DocumentType")
        .count();
    let collection_count = state
        .records
        .values()
        .filter(|record| record.item_type == "CollectionType")
        .count();
    println!("Documents: {document_count}");
    println!("Folders: {collection_count}");

    if let Some(report) = state.last_sync_report {
        println!(
            "Last report: {} synced, {} skipped, {} failed",
            report.synced_count, report.skipped_count, report.failed_count
        );
    }
    println!();
    Ok(())
}

fn relative_time(timestamp: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3_600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3_600)
    } else {
        format!("{}d ago", diff / 86_400)
    }
}
