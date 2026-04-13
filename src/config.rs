use crate::error::{AppError, Result};
use dirs::{config_dir, home_dir};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;

pub const RENDER_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Pdf,
    Markdown,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Pdf
    }
}

impl OutputFormat {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Markdown => "md",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_sync_folder")]
    pub sync_folder: String,
    #[serde(default)]
    pub output_format: OutputFormat,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sync_folder: default_sync_folder(),
            output_format: OutputFormat::Pdf,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    #[serde(default)]
    pub device_token: String,
    #[serde(default)]
    pub device_id: String,
}

impl AuthConfig {
    pub fn is_registered(&self) -> bool {
        !self.device_token.trim().is_empty() && !self.device_id.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncReport {
    pub start_time: u64,
    pub end_time: u64,
    pub duration_ms: u64,
    pub cloud_item_count: usize,
    pub listed_count: usize,
    pub synced_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    #[serde(default)]
    pub failed_items: Vec<SyncFailure>,
    #[serde(default)]
    pub fatal_error: Option<String>,
    #[serde(default)]
    pub log_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncFailure {
    pub id: String,
    pub name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRecord {
    pub hash: String,
    pub last_modified: String,
    pub local_path: String,
    pub visible_name: String,
    pub parent: String,
    pub item_type: String,
    pub output_format: OutputFormat,
    pub render_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncState {
    #[serde(default)]
    pub last_sync_timestamp: Option<u64>,
    #[serde(default)]
    pub last_sync_report: Option<SyncReport>,
    #[serde(default)]
    pub records: std::collections::BTreeMap<String, SyncRecord>,
}

pub struct ConfigStore {
    root: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Result<Self> {
        let Some(mut root) = config_dir() else {
            return Err(AppError::Config(
                "unable to resolve config directory".into(),
            ));
        };
        root.push("remarkable");
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn auth_path(&self) -> PathBuf {
        self.root.join("auth.toml")
    }

    pub fn state_path(&self) -> PathBuf {
        self.root.join("state.toml")
    }

    pub async fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.root).await?;
        Ok(())
    }

    pub async fn load_config(&self) -> Result<AppConfig> {
        self.load_or_default(self.config_path()).await
    }

    pub async fn save_config(&self, config: &AppConfig) -> Result<()> {
        self.save(self.config_path(), config).await
    }

    pub async fn load_auth(&self) -> Result<AuthConfig> {
        self.load_or_default(self.auth_path()).await
    }

    pub async fn save_auth(&self, auth: &AuthConfig) -> Result<()> {
        self.save(self.auth_path(), auth).await
    }

    pub async fn clear_auth(&self) -> Result<()> {
        self.ensure_dir().await?;
        match fs::remove_file(self.auth_path()).await {
            Ok(_) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn load_state(&self) -> Result<SyncState> {
        self.load_or_default(self.state_path()).await
    }

    pub async fn save_state(&self, state: &SyncState) -> Result<()> {
        self.save(self.state_path(), state).await
    }

    pub async fn clear_state(&self) -> Result<()> {
        self.save_state(&SyncState::default()).await
    }

    pub fn expand_home(&self, value: &str) -> Result<PathBuf> {
        if let Some(rest) = value.strip_prefix("~/") {
            let Some(home) = home_dir() else {
                return Err(AppError::Config("unable to resolve home directory".into()));
            };
            return Ok(home.join(rest));
        }

        Ok(PathBuf::from(value))
    }

    async fn load_or_default<T>(&self, path: PathBuf) -> Result<T>
    where
        T: for<'de> Deserialize<'de> + Default,
    {
        match fs::read_to_string(&path).await {
            Ok(contents) => Ok(toml::from_str(&contents)?),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
            Err(err) => Err(err.into()),
        }
    }

    async fn save<T>(&self, path: PathBuf, value: &T) -> Result<()>
    where
        T: Serialize,
    {
        self.ensure_dir().await?;
        let contents = toml::to_string_pretty(value)?;
        fs::write(path, format!("{contents}\n")).await?;
        Ok(())
    }
}

fn default_sync_folder() -> String {
    "~/remarkable-notes".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_tilde_paths() {
        let store = ConfigStore::new().expect("store");
        let expanded = store.expand_home("~/documents").expect("expanded");
        assert!(expanded.ends_with("documents"));
    }
}
