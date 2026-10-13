use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("Local service request failed ({0})")]
    Request(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Protected credential storage failed")]
    Secret,
    #[error("Runtime bootstrap failed: {0}")]
    Bootstrap(String),
    #[error("Runtime setup was cancelled. Retry when ready.")]
    Cancelled,
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(error: reqwest::Error) -> Self {
        Self::Request(
            error
                .status()
                .map(|s| s.as_u16().to_string())
                .unwrap_or_else(|| "network".into()),
        )
    }
}
