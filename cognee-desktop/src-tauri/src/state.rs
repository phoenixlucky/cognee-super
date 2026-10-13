use crate::config::AppConfig;
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tokio::process::Child;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunState {
    #[default]
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}
#[derive(Clone, Copy, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeState {
    #[default]
    Missing,
    Preparing,
    Ready,
    Failed,
}
#[derive(Default)]
pub struct ProcessState {
    pub api: Option<ManagedChild>,
    pub mcp: Option<ManagedChild>,
}
pub struct ManagedChild {
    pub child: Child,
    #[cfg(windows)]
    pub job: isize,
}
#[cfg(windows)]
impl Drop for ManagedChild {
    fn drop(&mut self) {
        if self.job != 0 {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.job as _);
            }
            self.job = 0;
        }
    }
}
pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub process: tokio::sync::Mutex<ProcessState>,
    pub api_status: Mutex<RunState>,
    pub mcp_status: Mutex<RunState>,
    pub runtime_status: Mutex<RuntimeState>,
    pub bootstrap_cancel: tokio::sync::Mutex<Option<CancellationToken>>,
    pub logs: std::sync::Arc<Mutex<Vec<String>>>,
    pub log_path: Mutex<Option<PathBuf>>,
    pub config_path: Mutex<Option<PathBuf>>,
    pub runtime_path: Mutex<Option<PathBuf>>,
}
impl Default for AppState {
    fn default() -> Self {
        Self {
            config: Mutex::new(AppConfig::default()),
            process: tokio::sync::Mutex::new(ProcessState::default()),
            api_status: Mutex::new(RunState::Stopped),
            mcp_status: Mutex::new(RunState::Stopped),
            runtime_status: Mutex::new(RuntimeState::Missing),
            bootstrap_cancel: tokio::sync::Mutex::new(None),
            logs: std::sync::Arc::new(Mutex::new(Vec::new())),
            log_path: Mutex::new(None),
            config_path: Mutex::new(None),
            runtime_path: Mutex::new(None),
        }
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    pub api_status: RunState,
    pub mcp_status: RunState,
    pub runtime_status: RuntimeState,
    pub config: AppConfig,
}
