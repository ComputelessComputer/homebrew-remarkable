use std::fmt::{Display, Formatter};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug)]
pub enum AppError {
    Io(std::io::Error),
    Http(reqwest::Error),
    Json(serde_json::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
    Api {
        status: Option<reqwest::StatusCode>,
        message: String,
    },
    Config(String),
    InvalidResponse(String),
    AuthRequired,
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Http(err) => write!(f, "HTTP error: {err}"),
            Self::Json(err) => write!(f, "JSON error: {err}"),
            Self::TomlDe(err) => write!(f, "TOML parse error: {err}"),
            Self::TomlSer(err) => write!(f, "TOML serialization error: {err}"),
            Self::Api { status, message } => match status {
                Some(code) => write!(f, "API error ({code}): {message}"),
                None => write!(f, "API error: {message}"),
            },
            Self::Config(message) => write!(f, "Configuration error: {message}"),
            Self::InvalidResponse(message) => write!(f, "Invalid response: {message}"),
            Self::AuthRequired => write!(
                f,
                "Not registered. Run `remarkable init` to connect your device."
            ),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<reqwest::Error> for AppError {
    fn from(value: reqwest::Error) -> Self {
        Self::Http(value)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

impl From<toml::de::Error> for AppError {
    fn from(value: toml::de::Error) -> Self {
        Self::TomlDe(value)
    }
}

impl From<toml::ser::Error> for AppError {
    fn from(value: toml::ser::Error) -> Self {
        Self::TomlSer(value)
    }
}
