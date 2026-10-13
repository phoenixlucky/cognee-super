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
            llm_provider: String::new(),
            llm_model: String::new(),
            llm_endpoint: String::new(),
            embedding_provider: String::new(),
            embedding_model: String::new(),
            graph_database_provider: "ladybug".into(),
            vector_db_provider: "lancedb".into(),
            vector_db_url: String::new(),
            start_on_login: false,
        }
    }
}

impl AppConfig {
    pub fn load(path: &PathBuf) -> Self {
        fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
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
