use crate::error::{AppError, Result};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::{sleep, Duration};

const DEVICE_REGISTER_URL: &str = "https://webapp.cloud.remarkable.com/token/json/2/device/new";
const TOKEN_REFRESH_URL: &str = "https://webapp.cloud.remarkable.com/token/json/2/user/new";
const RAW_HOST: &str = "https://eu.tectonic.remarkable.com";
const MAX_RATE_LIMIT_RETRIES: usize = 5;
const MAX_SERVER_RETRIES: usize = 3;
const BASE_DELAY_MS: u64 = 1_000;

#[derive(Debug, Clone, Deserialize)]
pub struct RootHashResponse {
    pub hash: String,
    #[allow(dead_code)]
    pub generation: u64,
    #[allow(dead_code)]
    #[serde(rename = "schemaVersion")]
    pub schema_version: u64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RawEntry {
    pub hash: String,
    pub entry_type: u32,
    pub id: String,
    pub subfiles: u32,
    pub size: u64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct EntriesFile {
    pub schema_version: u32,
    pub entries: Vec<RawEntry>,
    pub id: Option<String>,
    pub total_size: Option<u64>,
}

#[derive(Debug)]
pub struct RemarkableClient {
    http: Client,
    device_token: String,
    user_token: Option<String>,
}

impl RemarkableClient {
    pub fn new(device_token: String) -> Self {
        Self {
            http: Client::new(),
            device_token,
            user_token: None,
        }
    }

    pub async fn register(code: &str, device_id: &str) -> Result<String> {
        let http = Client::new();
        let response = http
            .post(DEVICE_REGISTER_URL)
            .header("Content-Type", "application/json")
            .body(
                serde_json::json!({
                    "code": code.trim(),
                    "deviceDesc": "desktop-rust-cli",
                    "deviceID": device_id,
                })
                .to_string(),
            )
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(AppError::Api {
                status: Some(status),
                message: text,
            });
        }

        Ok(text.trim().to_string())
    }

    pub async fn refresh_user_token(&mut self) -> Result<()> {
        if self.device_token.trim().is_empty() {
            return Err(AppError::AuthRequired);
        }

        let response = self
            .http
            .post(TOKEN_REFRESH_URL)
            .bearer_auth(&self.device_token)
            .send()
            .await?;

        let status = response.status();
        let text = response.text().await?;
        if !status.is_success() {
            return Err(AppError::Api {
                status: Some(status),
                message: text,
            });
        }

        self.user_token = Some(text.trim().to_string());
        Ok(())
    }

    pub async fn get_root_hash(&mut self) -> Result<RootHashResponse> {
        self.authed_json(&format!("{RAW_HOST}/sync/v4/root")).await
    }

    pub async fn get_entries(&mut self, hash: &str) -> Result<EntriesFile> {
        let text = self.get_text_by_hash(hash).await?;
        parse_entries_text(&text)
    }

    pub async fn get_text_by_hash(&mut self, hash: &str) -> Result<String> {
        let url = format!("{RAW_HOST}/sync/v3/files/{hash}");
        let bytes = self.authed_bytes_with_html_retry(&url, hash).await?;
        String::from_utf8(bytes).map_err(|err| AppError::InvalidResponse(err.to_string()))
    }

    pub async fn get_binary_by_hash(&mut self, hash: &str) -> Result<Vec<u8>> {
        let url = format!("{RAW_HOST}/sync/v3/files/{hash}");
        self.authed_bytes_with_html_retry(&url, hash).await
    }

    async fn authed_json<T>(&mut self, url: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let bytes = self.authed_bytes(url).await?;
        Ok(serde_json::from_slice(&bytes)?)
    }

    async fn authed_bytes_with_html_retry(&mut self, url: &str, hash: &str) -> Result<Vec<u8>> {
        for attempt in 0..=MAX_SERVER_RETRIES {
            let bytes = self.authed_bytes(url).await?;
            if !looks_like_html(&bytes) {
                return Ok(bytes);
            }

            if attempt == MAX_SERVER_RETRIES {
                return Err(AppError::InvalidResponse(format!(
                    "reMarkable returned HTML instead of file content for hash {hash}"
                )));
            }

            sleep(backoff_duration(attempt)).await;
        }

        unreachable!()
    }

    async fn authed_bytes(&mut self, url: &str) -> Result<Vec<u8>> {
        if self.user_token.is_none() {
            self.refresh_user_token().await?;
        }

        let first = self
            .request_with_retry(url, self.user_token.as_deref())
            .await;
        match first {
            Err(AppError::Api {
                status: Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
                ..
            }) => {
                self.refresh_user_token().await?;
                self.request_with_retry(url, self.user_token.as_deref())
                    .await
            }
            other => other,
        }
    }

    async fn request_with_retry(&self, url: &str, bearer: Option<&str>) -> Result<Vec<u8>> {
        let mut attempt = 0;
        loop {
            let mut request = self.http.get(url);
            if let Some(token) = bearer {
                request = request.bearer_auth(token);
            }

            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    let body = response.bytes().await?.to_vec();
                    if status.is_success() {
                        return Ok(body);
                    }

                    let retry_limit = retry_limit_for_status(status);
                    if attempt < retry_limit {
                        sleep(backoff_duration(attempt)).await;
                        attempt += 1;
                        continue;
                    }

                    return Err(AppError::Api {
                        status: Some(status),
                        message: String::from_utf8_lossy(&body).trim().to_string(),
                    });
                }
                Err(err) => {
                    if attempt < MAX_SERVER_RETRIES {
                        sleep(backoff_duration(attempt)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(err.into());
                }
            }

        }
    }
}

fn retry_limit_for_status(status: StatusCode) -> usize {
    match status {
        StatusCode::TOO_MANY_REQUESTS => MAX_RATE_LIMIT_RETRIES,
        StatusCode::INTERNAL_SERVER_ERROR
        | StatusCode::BAD_GATEWAY
        | StatusCode::SERVICE_UNAVAILABLE => MAX_SERVER_RETRIES,
        _ => 0,
    }
}

fn backoff_duration(attempt: usize) -> Duration {
    Duration::from_millis(BASE_DELAY_MS * 2_u64.pow(attempt as u32))
}

fn looks_like_html(bytes: &[u8]) -> bool {
    let prefix = String::from_utf8_lossy(&bytes[..bytes.len().min(128)]).to_ascii_lowercase();
    prefix.trim_start().starts_with("<html") || prefix.trim_start().starts_with("<!doctype html")
}

fn parse_entries_text(raw: &str) -> Result<EntriesFile> {
    let trimmed = raw.trim_end_matches('\n');
    let mut lines = trimmed.lines();
    let Some(version_line) = lines.next() else {
        return Err(AppError::InvalidResponse(
            "missing entries schema version".into(),
        ));
    };

    let schema_version = version_line.parse::<u32>().map_err(|_| {
        AppError::InvalidResponse(format!("invalid schema version: {version_line}"))
    })?;

    match schema_version {
        3 => Ok(EntriesFile {
            schema_version,
            entries: lines.map(parse_entry_line).collect::<Result<Vec<_>>>()?,
            id: None,
            total_size: None,
        }),
        4 => {
            let Some(info_line) = lines.next() else {
                return Err(AppError::InvalidResponse(
                    "missing schema v4 header info".into(),
                ));
            };
            let mut parts = info_line.split(':');
            let _hash = parts.next();
            let id = parts.nth(1).map(str::to_string);
            let total_size = parts.next().and_then(|value| value.parse::<u64>().ok());

            Ok(EntriesFile {
                schema_version,
                entries: lines.map(parse_entry_line).collect::<Result<Vec<_>>>()?,
                id,
                total_size,
            })
        }
        other => Err(AppError::InvalidResponse(format!(
            "unsupported entries schema version: {other}"
        ))),
    }
}

fn parse_entry_line(line: &str) -> Result<RawEntry> {
    let mut parts = line.split(':');
    let hash = parts
        .next()
        .ok_or_else(|| AppError::InvalidResponse(format!("malformed entry line: {line}")))?
        .to_string();
    let entry_type = parts
        .next()
        .ok_or_else(|| AppError::InvalidResponse(format!("malformed entry line: {line}")))?
        .parse::<u32>()
        .map_err(|_| AppError::InvalidResponse(format!("invalid entry type: {line}")))?;
    let id = parts
        .next()
        .ok_or_else(|| AppError::InvalidResponse(format!("malformed entry line: {line}")))?
        .to_string();
    let subfiles = parts
        .next()
        .ok_or_else(|| AppError::InvalidResponse(format!("malformed entry line: {line}")))?
        .parse::<u32>()
        .map_err(|_| AppError::InvalidResponse(format!("invalid subfiles count: {line}")))?;
    let size = parts
        .next()
        .ok_or_else(|| AppError::InvalidResponse(format!("malformed entry line: {line}")))?
        .parse::<u64>()
        .map_err(|_| AppError::InvalidResponse(format!("invalid size: {line}")))?;

    Ok(RawEntry {
        hash,
        entry_type,
        id,
        subfiles,
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_schema_three_entries() {
        let parsed = parse_entries_text("3\nabc:0:item:1:2\n").expect("parsed");
        assert_eq!(parsed.schema_version, 3);
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].id, "item");
    }
}
