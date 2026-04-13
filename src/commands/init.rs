use crate::client::RemarkableClient;
use crate::config::{AppConfig, AuthConfig, ConfigStore};
use crate::error::{AppError, Result};
use colored::Colorize;
use std::io::{self, Write};
use std::process::Command;
use uuid::Uuid;

const CONNECT_URL: &str = "https://my.remarkable.com/device/desktop/connect";

pub async fn run(store: &ConfigStore) -> Result<()> {
    println!();
    println!("{}", "Connect your reMarkable device".bold());
    println!("1. Visit {}", CONNECT_URL.cyan());
    println!("2. Sign in and copy the one-time code");
    println!();

    open_browser(CONNECT_URL);

    let code = prompt("Enter your one-time code: ")?;
    if code.trim().is_empty() {
        return Err(AppError::Config("no code entered".into()));
    }

    let device_id = Uuid::new_v4().to_string();
    let device_token = RemarkableClient::register(&code, &device_id).await?;

    let mut client = RemarkableClient::new(device_token.clone());
    client.refresh_user_token().await?;

    let folder = prompt("Where should notes be synced? [~/remarkable-notes]: ")?;
    let sync_folder = if folder.trim().is_empty() {
        "~/remarkable-notes".to_string()
    } else {
        folder.trim().to_string()
    };

    store
        .save_auth(&AuthConfig {
            device_token,
            device_id,
        })
        .await?;
    store
        .save_config(&AppConfig {
            sync_folder,
            output_format: Default::default(),
        })
        .await?;

    println!();
    println!("{}", "Device registered.".green().bold());
    println!("Config saved to {}", store.root().display());
    println!("Run `remarkable sync` to start syncing.");
    println!();

    Ok(())
}

fn prompt(message: &str) -> Result<String> {
    print!("{message}");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end().to_string())
}

fn open_browser(url: &str) {
    let command = if cfg!(target_os = "macos") {
        ("open", vec![url])
    } else if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", "start", url])
    } else {
        ("xdg-open", vec![url])
    };

    let _ = Command::new(command.0).args(command.1).spawn();
}
