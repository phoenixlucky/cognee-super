use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u8,
    pub api_host: String,
    pub api_port: u16,
    pub mcp_host: String,
    pub mcp_port: u16,
    pub data_root: String,
    pub llm_provider: String,
    pub llm_model: String,
    pub llm_endpoint: String,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub graph_database_provider: String,
    pub vector_db_provider: String,
    pub vector_db_url: String,
    pub start_on_login: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            api_host: "127.0.0.1".into(),
            api_port: 12315,
            mcp_host: "127.0.0.1".into(),
            mcp_port: 12316,
            data_root: String::new(),
            llm_provider: "ollama".into(),
            llm_model: "qwen2.5:14b-instruct-q2_K".into(),
            llm_endpoint: "http://localhost:11434".into(),
            embedding_provider: "fastembed".into(),
            embedding_model: "BAAI/bge-small-zh-v1.5".into(),
            graph_database_provider: "ladybug".into(),
            vector_db_provider: "lancedb".into(),
            vector_db_url: String::new(),
            start_on_login: false,
        }
    }
}

impl AppConfig {
    pub fn load(path: &PathBuf) -> Self {
        let mut config: Self = fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let defaults = Self::default();
        for (value, default) in [
            (&mut config.llm_provider, defaults.llm_provider),
            (&mut config.llm_model, defaults.llm_model),
            (&mut config.llm_endpoint, defaults.llm_endpoint),
            (&mut config.embedding_provider, defaults.embedding_provider),
            (&mut config.embedding_model, defaults.embedding_model),
        ] {
            if value.trim().is_empty() {
                *value = default;
            }
        }
        config
    }
    pub fn save(&self, path: &PathBuf) -> Result<(), crate::error::AppError> {
        if self.api_port < 1024 || self.mcp_port < 1024 || self.api_port == self.mcp_port {
            return Err(crate::error::AppError::Config(
                "Ports must be distinct values between 1024 and 65535".into(),
            ));
        }
        if self.api_host != "127.0.0.1" || self.mcp_host != "127.0.0.1" {
            return Err(crate::error::AppError::Config(
                "Services must bind to 127.0.0.1".into(),
            ));
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|_| {
                crate::error::AppError::Config("Cannot create app config directory".into())
            })?;
        }
        fs::write(
            path,
            serde_json::to_vec_pretty(self)
                .map_err(|_| crate::error::AppError::Config("Cannot encode app config".into()))?,
        )
        .map_err(|_| crate::error::AppError::Config("Cannot save app config".into()))
    }
}
