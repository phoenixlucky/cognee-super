use crate::{
    config::AppConfig,
    error::AppError,
    secrets,
    state::{AppState, ManagedChild, RunState, RuntimeState, StatusDto},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    time::sleep,
};
use uuid::Uuid;

fn app_paths(app: &AppHandle) -> Result<(PathBuf, PathBuf), AppError> {
    let config = app
        .path()
        .app_config_dir()
        .map_err(|_| AppError::Config("Cannot locate app config directory".into()))?
        .join("config.json");
    let data = app
        .path()
        .app_data_dir()
        .map_err(|_| AppError::Config("Cannot locate app data directory".into()))?;
    Ok((config, data))
}

fn portable_runtime(app: &AppHandle) -> Option<(PathBuf, PathBuf)> {
    let executable = std::env::current_exe().ok()?;
    let root = executable.parent()?;
    let runtime = root.join("runtime");
    let python = runtime.join(".venv/Scripts/python.exe");
    let python_home = std::fs::read_dir(runtime.join("python"))
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join("python.exe").is_file())?;
    if !python.is_file() {
        return None;
    }
    let config = runtime.join(".venv/pyvenv.cfg");
    let contents = std::fs::read_to_string(&config).ok()?;
    let home = format!("home = {}", python_home.display());
    let updated = contents
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("home =") {
                home.as_str()
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if updated != contents {
        std::fs::write(config, updated).ok()?;
    }
    let (_, data) = app_paths(app).ok()?;
    Some((python, data))
}
fn config_snapshot(state: &AppState) -> Result<AppConfig, AppError> {
    state
        .config
        .lock()
        .map(|c| c.clone())
        .map_err(|_| AppError::Config("Config lock unavailable".into()))
}
fn api_url(c: &AppConfig) -> String {
    format!("http://{}:{}", c.api_host, c.api_port)
}
fn status(state: &AppState) -> Result<StatusDto, AppError> {
    Ok(StatusDto {
        api_status: *state
            .api_status
            .lock()
            .map_err(|_| AppError::Config("Status unavailable".into()))?,
        mcp_status: *state
            .mcp_status
            .lock()
            .map_err(|_| AppError::Config("Status unavailable".into()))?,
        runtime_status: *state
            .runtime_status
            .lock()
            .map_err(|_| AppError::Config("Status unavailable".into()))?,
        config: config_snapshot(state)?,
        active_work: state.active_work.load(std::sync::atomic::Ordering::Acquire) > 0,
    })
}
struct ActiveWorkGuard(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl ActiveWorkGuard {
    fn new(state: &AppState) -> Self {
        state
            .active_work
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        Self(state.active_work.clone())
    }
}
impl Drop for ActiveWorkGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
fn append_log_file(path: &Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::metadata(path)
        .map(|m| m.len() > 5 * 1024 * 1024)
        .unwrap_or(false)
    {
        let rotated = path.with_extension("log.1");
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(path, rotated);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(file, "{line}");
    }
}
fn push_log(state: &AppState, line: String) {
    if let Ok(mut logs) = state.logs.lock() {
        logs.push(line.clone());
        if logs.len() > 600 {
            logs.drain(..200);
        }
    }
    if let Ok(path) = state.log_path.lock() {
        if let Some(path) = path.as_ref() {
            append_log_file(path, &line);
        }
    }
}
fn set_state<T: Copy>(lock: &Mutex<T>, value: T) -> Result<(), AppError> {
    *lock
        .lock()
        .map_err(|_| AppError::Config("Status unavailable".into()))? = value;
    Ok(())
}
fn ensure_port_free(port: u16) -> Result<(), AppError> {
    match std::net::TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(_) => Err(AppError::Message(format!(
            "Port {port} on 127.0.0.1 is already in use; close the other service or choose another port"
        ))),
    }
}
use std::{
    io::{ErrorKind, Write},
    process::Stdio,
    sync::Mutex,
};

fn capture_pipe<R: tokio::io::AsyncRead + Unpin + Send + 'static>(
    reader: R,
    state: std::sync::Arc<Mutex<Vec<String>>>,
    path: Option<PathBuf>,
    service: &'static str,
    secrets: Vec<String>,
) {
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(mut line)) = lines.next_line().await {
            for secret in &secrets {
                if secret.len() >= 4 {
                    line = line.replace(secret, "[redacted]");
                }
            }
            let lower = line.to_ascii_lowercase();
            if lower.contains("authorization:")
                || lower.contains("api_key=")
                || lower.contains("password=")
            {
                line = "[redacted sensitive process output]".into();
            }
            let entry = format!("{service}: {line}");
            if let Ok(mut log) = state.lock() {
                log.push(entry.clone());
                if log.len() > 600 {
                    log.drain(..200);
                }
            }
            if let Some(path) = &path {
                append_log_file(path, &entry);
            }
        }
    });
}

#[cfg(windows)]
fn assign_job(pid: u32) -> Result<isize, AppError> {
    use std::ptr::null;
    use windows_sys::Win32::{
        Foundation::CloseHandle,
        System::{
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
                SetInformationJobObject,
            },
            Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE},
        },
    };
    unsafe {
        let job = CreateJobObjectW(null(), null());
        if job.is_null() {
            return Err(AppError::Message(
                "Cannot create Windows process job".into(),
            ));
        }
        let process = OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, 0, pid);
        if process.is_null() {
            CloseHandle(job);
            return Err(AppError::Message("Cannot open Cognee child process".into()));
        }
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let ok = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limits as *const _ as *const _,
            std::mem::size_of_val(&limits) as u32,
        ) != 0
            && AssignProcessToJobObject(job, process) != 0;
        CloseHandle(process);
        if !ok {
            CloseHandle(job);
            return Err(AppError::Message(
                "Cannot assign Cognee process to its Windows job".into(),
            ));
        }
        Ok(job as isize)
    }
}
#[cfg(not(windows))]
fn assign_job(_pid: u32) -> Result<isize, AppError> {
    Ok(0)
}

fn track_child(
    mut child: Child,
    state: &AppState,
    service: &'static str,
    redactions: Vec<String>,
) -> Result<ManagedChild, AppError> {
    let pid = child
        .id()
        .ok_or_else(|| AppError::Message("Child process did not start".into()))?;
    let out = child.stdout.take();
    let err = child.stderr.take();
    let log = state.logs.clone();
    let log_path = state.log_path.lock().ok().and_then(|p| p.clone());
    if let Some(pipe) = out {
        capture_pipe(
            pipe,
            log.clone(),
            log_path.clone(),
            service,
            redactions.clone(),
        );
    }
    if let Some(pipe) = err {
        capture_pipe(pipe, log, log_path, service, redactions);
    }
    Ok(ManagedChild {
        child,
        #[cfg(windows)]
        job: assign_job(pid)?,
    })
}

#[tauri::command]
pub async fn get_app_status(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<StatusDto, AppError> {
    let (path, data) = app_paths(&app)?;
    if let Ok(mut p) = state.config_path.lock() {
        if p.is_none() {
            *p = Some(path.clone());
        }
    }
    if let Ok(mut p) = state.runtime_path.lock() {
        if p.is_none() {
            *p = Some(data.join("runtime"));
        }
    }
    if let Ok(mut p) = state.log_path.lock() {
        if p.is_none() {
            *p = Some(data.join("logs/service.log"));
        }
    }
    if let Ok(mut cfg) = state.config.lock() {
        *cfg = AppConfig::load(&path);
    }
    if let Ok(mut runtime) = state.runtime_status.lock() {
        let runtime_ready = portable_runtime(&app)
            .and_then(|(python, _)| {
                python
                    .parent()
                    .and_then(Path::parent)
                    .and_then(Path::parent)
                    .map(runtime_has_fastembed)
            })
            .unwrap_or_else(|| runtime_has_fastembed(&data.join("runtime")));
        *runtime = runtime_state_after_check(*runtime, runtime_ready);
    }
    let mut processes = state.process.lock().await;
    let api_exited = processes
        .api
        .as_mut()
        .and_then(|p| p.child.try_wait().ok().flatten())
        .is_some();
    let mcp_exited = processes
        .mcp
        .as_mut()
        .and_then(|p| p.child.try_wait().ok().flatten())
        .is_some();
    if api_exited {
        processes.api.take();
        let _ = set_state(&state.api_status, RunState::Failed);
        if let Some(mut mcp) = processes.mcp.take() {
            let _ = mcp.child.start_kill();
            #[cfg(windows)]
            unsafe {
                use windows_sys::Win32::{
                    Foundation::CloseHandle, System::JobObjects::TerminateJobObject,
                };
                let _ = TerminateJobObject(mcp.job as _, 1);
                CloseHandle(mcp.job as _);
            }
            let _ = set_state(&state.mcp_status, RunState::Stopped);
        }
    }
    if mcp_exited {
        processes.mcp.take();
        let _ = set_state(&state.mcp_status, RunState::Failed);
    }
    drop(processes);
    status(&state)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    api_port: u16,
    mcp_port: u16,
    data_root: String,
    llm_provider: String,
    llm_model: String,
    llm_endpoint: String,
    embedding_provider: String,
    embedding_model: String,
    graph_database_provider: String,
    vector_db_provider: String,
    vector_db_url: String,
}
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    input: SettingsInput,
) -> Result<(), AppError> {
    let (path, _) = app_paths(&app)?;
    let mut c = config_snapshot(&state)?;
    c.api_port = input.api_port;
    c.mcp_port = input.mcp_port;
    c.data_root = input.data_root;
    c.llm_provider = input.llm_provider;
    c.llm_model = input.llm_model;
    c.llm_endpoint = input.llm_endpoint;
    c.embedding_provider = input.embedding_provider;
    c.embedding_model = input.embedding_model;
    c.graph_database_provider = input.graph_database_provider;
    c.vector_db_provider = input.vector_db_provider;
    c.vector_db_url = input.vector_db_url;
    c.save(&path)?;
    *state
        .config
        .lock()
        .map_err(|_| AppError::Config("Config lock unavailable".into()))? = c;
    Ok(())
}

const OLLAMA_MODELS: [&str; 3] = [
    "qwen2.5:14b-instruct-q2_K",
    "qwen2.5:14b-instruct-q4_K_M",
    "qwen2.5:32b-instruct-q4_K_M",
];

#[tauri::command]
pub async fn check_ollama() -> bool {
    Command::new("ollama")
        .arg("--version")
        .output()
        .await
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[tauri::command]
pub async fn pull_ollama_model(
    model: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), AppError> {
    if !OLLAMA_MODELS.contains(&model.as_str()) {
        return Err(AppError::Config(
            "Select one of the supported Ollama presets".into(),
        ));
    }

    let _active = ActiveWorkGuard::new(&state);
    let mut command = Command::new("ollama");
    command
        .args(["pull", model.as_str()])
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    let child = command.spawn().map_err(|_| {
        AppError::Message("未检测到 Ollama。请先安装 Ollama for Windows，再重新检查。".into())
    })?;
    let mut child = track_child(child, &state, "ollama", vec![])?;
    let status = child
        .child
        .wait()
        .await
        .map_err(|_| AppError::Message("无法读取 Ollama 下载进程状态".into()))?;
    if !status.success() {
        return Err(AppError::Message(format!(
            "ollama pull 下载失败（退出码 {}），请查看服务日志后重试。",
            status
                .code()
                .map(|code| code.to_string())
                .unwrap_or_else(|| "未知".into())
        )));
    }
    push_log(&state, format!("Ollama model ready: {model}"));
    Ok(())
}

#[tauri::command]
pub fn set_provider_secret(kind: String, value: String) -> Result<(), AppError> {
    if !["llm", "embedding", "vector"].contains(&kind.as_str()) || value.is_empty() {
        return Err(AppError::Config("Invalid secret field".into()));
    }
    secrets::put_secret(&format!("provider.{kind}"), &value)
}
#[tauri::command]
pub fn clear_provider_secret(kind: String) -> Result<(), AppError> {
    if !["llm", "embedding", "vector"].contains(&kind.as_str()) {
        return Err(AppError::Config("Invalid secret field".into()));
    }
    secrets::delete_secret(&format!("provider.{kind}"))
}
#[tauri::command]
pub fn get_settings_summary() -> Result<std::collections::HashMap<String, bool>, AppError> {
    Ok([
        ("llm", "provider.llm"),
        ("embedding", "provider.embedding"),
        ("vector", "provider.vector"),
    ]
    .into_iter()
    .map(|(k, n)| Ok((k.to_string(), secrets::get_secret(n)?.is_some())))
    .collect::<Result<_, AppError>>()?)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntryDto {
    service: String,
    line: String,
    timestamp: String,
}
#[tauri::command]
pub fn get_recent_logs(state: tauri::State<'_, AppState>) -> Result<Vec<LogEntryDto>, AppError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    state
        .logs
        .lock()
        .map(|l| {
            l.iter()
                .map(|line| {
                    let (service, msg) = line.split_once(": ").unwrap_or(("app", line.as_str()));
                    LogEntryDto {
                        service: service.to_string(),
                        line: msg.to_string(),
                        timestamp: now.to_string(),
                    }
                })
                .collect()
        })
        .map_err(|_| AppError::Config("Logs unavailable".into()))
}

async fn bootstrap_output(
    mut command: Command,
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<std::process::Output, AppError> {
    command.kill_on_drop(true);
    tokio::select! {result=command.output()=>result.map_err(|_|AppError::Bootstrap("Could not start uv. Retry the runtime setup.".into())),_=cancel.cancelled()=>Err(AppError::Cancelled)}
}
#[tauri::command]
pub async fn cancel_runtime(state: tauri::State<'_, AppState>) -> Result<bool, AppError> {
    let guard = state.bootstrap_cancel.lock().await;
    if let Some(token) = guard.as_ref() {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
pub async fn prepare_runtime(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<StatusDto, AppError> {
    set_state(&state.runtime_status, RuntimeState::Preparing)?;
    let cancel = tokio_util::sync::CancellationToken::new();
    *state.bootstrap_cancel.lock().await = Some(cancel.clone());
    let result: Result<(), AppError> = async {
        let (_, data) = app_paths(&app)?;
        if let Some((python, _)) = portable_runtime(&app) {
            if runtime_importable(&python).await {
                return Ok(());
            }
            return Err(AppError::Bootstrap(
                "Bundled portable runtime is incomplete. Re-extract the portable package and retry."
                    .into(),
            ));
        }
        let runtime = data.join("runtime");
        tokio::fs::create_dir_all(&runtime)
            .await
            .map_err(|_| AppError::Bootstrap("Cannot create app runtime directory".into()))?;
        let resource = app
            .path()
            .resource_dir()
            .map_err(|_| AppError::Bootstrap("Cannot locate bundled runtime files".into()))?;
        let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
        let uv = if resource.join("uv.exe").exists() {
            resource.join("uv.exe")
        } else {
            dev.join("resources/uv/uv.exe")
        };
        let project = if resource.join("runtime/pyproject.toml").exists() {
            resource.join("runtime/pyproject.toml")
        } else {
            dev.join("runtime/pyproject.toml")
        };
        if !uv.exists() || !project.exists() {
            return Err(AppError::Bootstrap(
                "Bundled uv or runtime lockfile is missing. Reinstall the application.".into(),
            ));
        }
        let venv = runtime.join(".venv");
        if venv.exists() && !venv.join("Scripts/python.exe").is_file() {
            tokio::fs::remove_dir_all(&venv)
                .await
                .map_err(|_| AppError::Bootstrap("Cannot recreate incomplete Python environment".into()))?;
        }
        let python_dir = runtime.join("python");
        let cache = data.join("uv-cache");
        tokio::fs::create_dir_all(&cache)
            .await
            .map_err(|_| AppError::Bootstrap("Cannot prepare runtime cache".into()))?;
        let _ = app.emit(
            "bootstrap-progress",
            json!({"phase":"python","percent":10,"message":"正在准备 Python 3.12.10"}),
        );
        let mut python = Command::new(&uv);
        python
            .args(["python", "install", "3.12.10", "--install-dir"])
            .arg(&python_dir)
            .arg("--no-bin")
            .env("UV_PYTHON_INSTALL_DIR", &python_dir)
            .env("UV_CACHE_DIR", &cache)
            .current_dir(&runtime);
        let output = bootstrap_output(python, &cancel).await?;
        if !output.status.success() {
            return Err(AppError::Bootstrap(
                "Python runtime download failed. Check network access and retry.".into(),
            ));
        }
        let _ = app.emit(
            "bootstrap-progress",
            json!({"phase":"dependencies","percent":42,"message":"正在同步已锁定的 Cognee 依赖"}),
        );
        let mut sync = Command::new(&uv);
        sync.args(["sync", "--locked", "--no-dev", "--no-editable", "--project"])
            .arg(project)
            .arg("--python")
            .arg("3.12.10")
            .env("UV_PYTHON_INSTALL_DIR", &python_dir)
            .env("UV_CACHE_DIR", &cache)
            .env("UV_PROJECT_ENVIRONMENT", &venv)
            .current_dir(&runtime);
        let output = bootstrap_output(sync, &cancel).await?;
        if !output.status.success() {
            push_log(
                &state,
                String::from_utf8_lossy(&output.stderr)
                    .chars()
                    .take(4000)
                    .collect(),
            );
            return Err(AppError::Bootstrap(
                "Cognee dependencies could not be installed. Review logs and retry.".into(),
            ));
        }
        let python = venv.join("Scripts/python.exe");
        if !runtime_importable(&python).await {
            push_log(
                &state,
                "Required Cognee runtime packages are still unavailable after locked dependency sync".into(),
            );
            return Err(AppError::Bootstrap(
                "Required Cognee runtime packages are still unavailable after runtime repair. Review logs and retry."
                    .into(),
            ));
        }
        Ok(())
    }
    .await;
    state.bootstrap_cancel.lock().await.take();
    match result {
        Ok(()) => {
            let _ = app.emit(
                "bootstrap-progress",
                json!({"phase":"ready","percent":100,"message":"Cognee 运行环境已就绪"}),
            );
            set_state(&state.runtime_status, RuntimeState::Ready)?;
            push_log(&state, "Cognee runtime is ready".into());
            status(&state)
        }
        Err(error) => {
            set_state(
                &state.runtime_status,
                if matches!(&error, AppError::Cancelled) {
                    RuntimeState::Missing
                } else {
                    RuntimeState::Failed
                },
            )?;
            let _ = app.emit(
                "bootstrap-progress",
                json!({"phase":"failed","percent":0,"message":error.to_string()}),
            );
            Err(error)
        }
    }
}

fn runtime_paths(app: &AppHandle) -> Result<(PathBuf, PathBuf), AppError> {
    if let Some(runtime) = portable_runtime(app) {
        return Ok(runtime);
    }
    let (_, data) = app_paths(app)?;
    Ok((data.join("runtime/.venv/Scripts/python.exe"), data))
}

fn runtime_has_fastembed(runtime_dir: &Path) -> bool {
    runtime_dir.join(".venv/Scripts/python.exe").is_file()
        && runtime_dir
            .join(".venv/Lib/site-packages/fastembed/__init__.py")
            .is_file()
}

async fn runtime_importable(python: &Path) -> bool {
    Command::new(python)
        .args(["-I", "-c", "import fastembed, src"])
        .status()
        .await
        .is_ok_and(|status| status.success())
}

fn runtime_needs_repair(python_exists: bool, fastembed_importable: bool) -> bool {
    !python_exists || !fastembed_importable
}

fn runtime_state_after_check(current: RuntimeState, runtime_ready: bool) -> RuntimeState {
    if current == RuntimeState::Preparing {
        current
    } else if runtime_ready {
        RuntimeState::Ready
    } else if current == RuntimeState::Ready {
        RuntimeState::Missing
    } else {
        current
    }
}

fn random_secret() -> String {
    Uuid::new_v4().to_string() + &Uuid::new_v4().to_string()
}
async fn ensure_default_credentials() -> Result<(String, String), AppError> {
    let email = "default_user@example.com".to_string();
    let password = match secrets::get_secret("local.default_password")? {
        Some(v) => v,
        None => {
            let v = random_secret();
            secrets::put_secret("local.default_password", &v)?;
            v
        }
    };
    let jwt = match secrets::get_secret("local.jwt_secret")? {
        Some(v) => v,
        None => {
            let v = random_secret();
            secrets::put_secret("local.jwt_secret", &v)?;
            v
        }
    };
    let _ = jwt;
    Ok((email, password))
}
fn provider_env(c: &AppConfig) -> Vec<(String, String)> {
    let mut e = vec![];
    for (k, v) in [
        ("LLM_PROVIDER", &c.llm_provider),
        ("LLM_MODEL", &c.llm_model),
        ("LLM_ENDPOINT", &c.llm_endpoint),
        ("EMBEDDING_PROVIDER", &c.embedding_provider),
        ("EMBEDDING_MODEL", &c.embedding_model),
        ("GRAPH_DATABASE_PROVIDER", &c.graph_database_provider),
        ("VECTOR_DB_PROVIDER", &c.vector_db_provider),
        ("VECTOR_DB_URL", &c.vector_db_url),
    ] {
        if !v.trim().is_empty() {
            e.push((k.into(), v.clone()));
        }
    }
    for (k, name) in [
        ("LLM_API_KEY", "provider.llm"),
        ("EMBEDDING_API_KEY", "provider.embedding"),
        ("VECTOR_DB_KEY", "provider.vector"),
    ] {
        if k == "LLM_API_KEY" && c.llm_provider == "ollama" {
            e.push((k.into(), "ollama".into()));
        } else if let Ok(Some(v)) = secrets::get_secret(name) {
            e.push((k.into(), v));
        }
    }
    e
}

#[tauri::command]
pub async fn start_api(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<StatusDto, AppError> {
    let (python, _) = runtime_paths(&app)?;
    let python_exists = python.is_file();
    let importable = python_exists && runtime_importable(&python).await;
    if runtime_needs_repair(python_exists, importable) {
        prepare_runtime(app.clone(), state.clone()).await?;
    }
    let (python, data) = runtime_paths(&app)?;
    if !runtime_importable(&python).await {
        let _ = set_state(&state.runtime_status, RuntimeState::Failed);
        return Err(AppError::Bootstrap(
            "Required Cognee runtime packages could not be imported after runtime repair. Review logs and retry.".into(),
        ));
    }
    let c = config_snapshot(&state)?;
    ensure_port_free(c.api_port)?;
    set_state(&state.api_status, RunState::Starting)?;
    if !python.exists() {
        set_state(&state.api_status, RunState::Failed)?;
        return Err(AppError::Bootstrap(
            "Runtime Python executable is missing".into(),
        ));
    }
    let (email, password) = ensure_default_credentials().await?;
    let jwt = secrets::get_secret("local.jwt_secret")?.ok_or(AppError::Secret)?;
    let root = if c.data_root.trim().is_empty() {
        data.join("cognee-data")
    } else {
        PathBuf::from(&c.data_root)
    };
    tokio::fs::create_dir_all(&root)
        .await
        .map_err(|_| AppError::Config("Cannot create memory data directory".into()))?;
    let mut cmd = Command::new(python);
    cmd.args(["-m", "cognee.api.client"])
        .current_dir(&root)
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("HTTP_API_HOST", "127.0.0.1")
        .env("HTTP_API_PORT", c.api_port.to_string())
        .env("REQUIRE_AUTHENTICATION", "true")
        .env("ENABLE_BACKEND_ACCESS_CONTROL", "true")
        .env("DEFAULT_USER_EMAIL", email.clone())
        .env("DEFAULT_USER_PASSWORD", password.clone())
        .env("FASTAPI_USERS_JWT_SECRET", jwt.clone())
        .env("DATA_ROOT_DIRECTORY", root.join("data"))
        .env("SYSTEM_ROOT_DIRECTORY", root.join("system"))
        .env("CACHE_ROOT_DIRECTORY", root.join("cache"))
        .env("FASTEMBED_CACHE_PATH", root.join("cache").join("fastembed"))
        .env("COGNEE_LOGS_DIR", root.join("logs"))
        .env("COGNEE_REPOS_DIR", root.join("repos"));
    let provider_settings = provider_env(&c);
    for (k, v) in &provider_settings {
        cmd.env(k, v);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().map_err(|e| {
        let _ = set_state(&state.api_status, RunState::Failed);
        let msg = if e.kind() == ErrorKind::AddrInUse {
            format!(
                "Port {} is already in use; stop the other listener or choose a different port",
                c.api_port
            )
        } else {
            format!("Cannot launch Cognee API: {e}")
        };
        AppError::Message(msg)
    })?;
    let mut redactions = vec![password, jwt];
    redactions.extend(provider_settings.into_iter().map(|(_, value)| value));
    let managed = track_child(child, &state, "api", redactions)?;
    {
        let mut p = state.process.lock().await;
        p.api = Some(managed);
    }
    let url = format!("http://127.0.0.1:{}/health", c.api_port);
    let client = reqwest::Client::new();
    let mut ready = false;
    for _ in 0..90 {
        if let Ok(r) = client
            .get(&url)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            if r.status().is_success() {
                ready = true;
                break;
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
    if !ready {
        stop_one(&state, false).await?;
        set_state(&state.api_status, RunState::Failed)?;
        push_log(
            &state,
            format!(
                "Cognee API did not become ready on port {}. Check logs.",
                c.api_port
            ),
        );
        return Err(AppError::Message(format!(
            "Cognee API did not become ready on 127.0.0.1:{}; check logs",
            c.api_port
        )));
    }
    set_state(&state.api_status, RunState::Running)?;
    push_log(
        &state,
        format!("Cognee API listening on 127.0.0.1:{}", c.api_port),
    );
    status(&state)
}

async fn bearer_token(c: &AppConfig) -> Result<String, AppError> {
    let (email, password) = ensure_default_credentials().await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?;
    let response = client
        .post(format!("{}/api/v1/auth/login", api_url(c)))
        .form(&[
            ("username", email.as_str()),
            ("password", password.as_str()),
        ])
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(AppError::Message(
            "Local Cognee authentication failed; restart the API to recover credentials".into(),
        ));
    }
    let body: Value = response_json(response, 1024 * 1024).await?;
    body.get("access_token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AppError::Message("Cognee did not return an access token".into()))
}
async fn ensure_mcp_key(c: &AppConfig) -> Result<String, AppError> {
    if let Some(key) = secrets::get_secret("local.mcp_api_key")? {
        let r = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?
            .get(format!("{}/api/v1/auth/me", api_url(c)))
            .header("x-api-key", &key)
            .send()
            .await?;
        if r.status().is_success() {
            return Ok(key);
        }
    }
    let token = bearer_token(c).await?;
    let r = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()?
        .post(format!("{}/api/v1/auth/api-keys", api_url(c)))
        .bearer_auth(token)
        .json(&json!({"name":"Cognee Desktop MCP"}))
        .send()
        .await?;
    if !r.status().is_success() {
        return Err(AppError::Message(
            "Could not create a protected MCP API key".into(),
        ));
    }
    let v: Value = response_json(r, 1024 * 1024).await?;
    let key = v
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Message("Cognee did not return an API key".into()))?
        .to_owned();
    secrets::put_secret("local.mcp_api_key", &key)?;
    Ok(key)
}

#[tauri::command]
pub async fn start_mcp(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<StatusDto, AppError> {
    if *state
        .api_status
        .lock()
        .map_err(|_| AppError::Config("Status unavailable".into()))?
        != RunState::Running
    {
        return Err(AppError::Message(
            "Start the Cognee API before starting MCP".into(),
        ));
    }
    let c = config_snapshot(&state)?;
    ensure_port_free(c.mcp_port)?;
    set_state(&state.mcp_status, RunState::Starting)?;
    let key = match ensure_mcp_key(&c).await {
        Ok(key) => key,
        Err(e) => {
            set_state(&state.mcp_status, RunState::Failed)?;
            return Err(e);
        }
    };
    let (python, _) = runtime_paths(&app)?;
    let mut cmd = Command::new(python);
    cmd.args([
        "-m",
        "src.server",
        "--transport",
        "http-new",
        "--host",
        "127.0.0.1",
        "--port",
        &c.mcp_port.to_string(),
        "--api-url",
        &api_url(&c),
    ])
    .env("COGNEE_API_KEY", key.clone())
    .env("COGNEE_API_AUTH_SCHEME", "x-api-key")
    .kill_on_drop(true)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    let child = cmd.spawn().map_err(|e| {
        let _ = set_state(&state.mcp_status, RunState::Failed);
        let msg = if e.kind() == ErrorKind::AddrInUse {
            format!(
                "Port {} is already in use; stop the other listener or choose a different port",
                c.mcp_port
            )
        } else {
            format!("Cannot launch Cognee MCP: {e}")
        };
        AppError::Message(msg)
    })?;
    let managed = track_child(child, &state, "mcp", vec![key])?;
    {
        let mut p = state.process.lock().await;
        p.mcp = Some(managed);
    }
    let client = reqwest::Client::new();
    let mut ready = false;
    for _ in 0..45 {
        if let Ok(r) = client
            .get(format!("http://127.0.0.1:{}/health", c.mcp_port))
            .timeout(Duration::from_secs(1))
            .send()
            .await
        {
            if r.status().is_success() {
                ready = true;
                break;
            }
        }
        sleep(Duration::from_millis(500)).await;
    }
    if !ready {
        stop_one(&state, true).await?;
        set_state(&state.mcp_status, RunState::Failed)?;
        return Err(AppError::Message(format!(
            "Cognee MCP did not become ready on port {}",
            c.mcp_port
        )));
    }
    set_state(&state.mcp_status, RunState::Running)?;
    push_log(
        &state,
        format!(
            "Cognee MCP listening at http://127.0.0.1:{}/mcp-new",
            c.mcp_port
        ),
    );
    status(&state)
}

async fn stop_one(state: &AppState, mcp: bool) -> Result<(), AppError> {
    let lock = if mcp {
        &state.mcp_status
    } else {
        &state.api_status
    };
    set_state(lock, RunState::Stopping)?;
    let mut p = state.process.lock().await;
    let managed = if mcp { p.mcp.take() } else { p.api.take() };
    if let Some(mut item) = managed {
        let _ = item.child.start_kill();
        let _ = tokio::time::timeout(Duration::from_secs(3), item.child.wait()).await;
        drop(item);
    }
    set_state(lock, RunState::Stopped)?;
    Ok(())
}
#[tauri::command]
pub async fn stop_api(state: tauri::State<'_, AppState>) -> Result<StatusDto, AppError> {
    stop_one(&state, true).await?;
    stop_one(&state, false).await?;
    status(&state)
}
#[tauri::command]
pub async fn stop_mcp(state: tauri::State<'_, AppState>) -> Result<StatusDto, AppError> {
    stop_one(&state, true).await?;
    status(&state)
}
#[tauri::command]
pub async fn stop_all(state: tauri::State<'_, AppState>) -> Result<StatusDto, AppError> {
    stop_one(&state, true).await?;
    stop_one(&state, false).await?;
    status(&state)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDto {
    name: String,
    description: String,
    input_schema: Value,
}
#[tauri::command]
pub async fn list_mcp_tools(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ToolDto>, AppError> {
    let c = config_snapshot(&state)?;
    let key = secrets::get_secret("local.mcp_api_key")?.ok_or_else(|| {
        AppError::Message("MCP API key is unavailable; start the MCP service first".into())
    })?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let endpoint = format!("http://127.0.0.1:{}/mcp-new", c.mcp_port);
    let request_meta = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "cognee-desktop", "version": "0.1.0"}
    });
    for (id, method) in [(1, "server/discover"), (2, "tools/list")] {
        let response = client
            .post(&endpoint)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .header("MCP-Protocol-Version", "2026-07-28")
            .header("Mcp-Method", method)
            .header("x-api-key", &key)
            .json(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":{"_meta":request_meta}}))
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(AppError::Message(format!(
                "MCP {method} request returned HTTP {}",
                response.status().as_u16()
            )));
        }
        let body =
            String::from_utf8_lossy(&bounded_body(response, 5 * 1024 * 1024).await?).to_string();
        let body = body
            .lines()
            .find(|line| line.starts_with("data: "))
            .map(|line| &line[6..])
            .unwrap_or(&body);
        let value: Value = serde_json::from_str(body)
            .map_err(|_| AppError::Message("MCP returned an unsupported tools response".into()))?;
        if let Some(error) = value.get("error") {
            return Err(AppError::Message(format!("MCP {method} failed: {error}")));
        }
        if method == "tools/list" {
            let tools = value
                .pointer("/result/tools")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    AppError::Message("MCP tools/list response has no tools array".into())
                })?;
            return Ok(tools
                .iter()
                .filter_map(|t| {
                    Some(ToolDto {
                        name: t.get("name")?.as_str()?.to_owned(),
                        description: t
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_owned(),
                        input_schema: t.get("inputSchema").cloned().unwrap_or(Value::Null),
                    })
                })
                .collect());
        }
    }
    let _ = app;
    Err(AppError::Message(
        "MCP tools/list request did not complete".into(),
    ))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetDto {
    id: String,
    name: String,
    created_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DataDto {
    id: String,
    name: String,
    file_type: String,
    size: Option<u64>,
    created_at: String,
}
async fn api_request(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<Value, AppError> {
    let c = config_snapshot(state)?;
    let token = bearer_token(&c).await?;
    let mut req = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?
        .request(method, format!("{}{}", api_url(&c), path))
        .bearer_auth(token);
    if let Some(v) = body {
        req = req.json(&v);
    }
    let r = req.send().await?;
    if !r.status().is_success() {
        return Err(AppError::Message(format!(
            "Cognee API returned HTTP {}",
            r.status().as_u16()
        )));
    }
    if r.status() == reqwest::StatusCode::NO_CONTENT {
        return Ok(Value::Null);
    }
    response_json(r, 10 * 1024 * 1024).await
}
async fn bounded_body(response: reqwest::Response, limit: usize) -> Result<Vec<u8>, AppError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(AppError::Message(
            "Cognee response exceeds the desktop safety limit".into(),
        ));
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        let chunk = chunk.map_err(AppError::from)?;
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(AppError::Message(
                "Cognee response exceeds the desktop safety limit".into(),
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
async fn response_json(response: reqwest::Response, limit: usize) -> Result<Value, AppError> {
    serde_json::from_slice(&bounded_body(response, limit).await?)
        .map_err(|_| AppError::Message("Cognee returned invalid JSON".into()))
}
fn value_array(v: Value) -> Vec<Value> {
    v.as_array()
        .cloned()
        .or_else(|| v.get("items").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}
fn data_size_bytes(value: &Value) -> Option<u64> {
    value
        .get("data_size")
        .and_then(Value::as_u64)
        .or_else(|| value.get("dataSize").and_then(Value::as_u64))
}
fn session_summary(value: &Value, max_chars: usize) -> String {
    value
        .get("summary")
        .and_then(Value::as_str)
        .or_else(|| value.get("label").and_then(Value::as_str))
        .or_else(|| value.get("agent_id").and_then(Value::as_str))
        .unwrap_or("")
        .chars()
        .take(max_chars)
        .collect()
}
#[tauri::command]
pub async fn list_datasets(state: tauri::State<'_, AppState>) -> Result<Vec<DatasetDto>, AppError> {
    Ok(
        value_array(api_request(&state, reqwest::Method::GET, "/api/v1/datasets", None).await?)
            .iter()
            .filter_map(|v| {
                Some(DatasetDto {
                    id: v.get("id")?.as_str()?.into(),
                    name: v.get("name")?.as_str()?.into(),
                    created_at: v
                        .get("created_at")
                        .or_else(|| v.get("createdAt"))
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .into(),
                })
            })
            .collect(),
    )
}
#[tauri::command]
pub async fn create_dataset(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<(), AppError> {
    if name.trim().is_empty() || name.len() > 120 {
        return Err(AppError::Config(
            "Dataset name must be 1–120 characters".into(),
        ));
    }
    api_request(
        &state,
        reqwest::Method::POST,
        "/api/v1/datasets",
        Some(json!({"name":name})),
    )
    .await?;
    Ok(())
}
#[tauri::command]
pub async fn delete_dataset(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
) -> Result<(), AppError> {
    let id =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    api_request(
        &state,
        reqwest::Method::DELETE,
        &format!("/api/v1/datasets/{id}"),
        None,
    )
    .await?;
    Ok(())
}
#[tauri::command]
pub async fn list_data(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
    limit: u16,
    offset: u32,
) -> Result<Vec<DataDto>, AppError> {
    let id =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let limit = limit.clamp(1, 200);
    let offset = offset.min(1_000_000);
    let v = api_request(
        &state,
        reqwest::Method::GET,
        &format!("/api/v1/datasets/{id}/data?limit={limit}&offset={offset}"),
        None,
    )
    .await?;
    Ok(value_array(v)
        .iter()
        .filter_map(|v| {
            Some(DataDto {
                id: v.get("id")?.as_str()?.into(),
                name: v
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("untitled")
                    .into(),
                file_type: v
                    .get("extension")
                    .or_else(|| v.get("mime_type"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
                size: data_size_bytes(v),
                created_at: v
                    .get("created_at")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .into(),
            })
        })
        .collect())
}
#[tauri::command]
pub async fn delete_data(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
    data_id: String,
) -> Result<(), AppError> {
    let d =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let id = Uuid::parse_str(&data_id).map_err(|_| AppError::Config("Invalid data id".into()))?;
    api_request(
        &state,
        reqwest::Method::DELETE,
        &format!("/api/v1/datasets/{d}/data/{id}"),
        None,
    )
    .await?;
    Ok(())
}
#[tauri::command]
pub async fn upload_source(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    dataset_id: String,
) -> Result<(), AppError> {
    let id =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let selected = app
        .dialog()
        .file()
        .blocking_pick_files()
        .unwrap_or_default();
    if selected.is_empty() {
        return Ok(());
    }
    let _active_work = ActiveWorkGuard::new(&state);
    let c = config_snapshot(&state)?;
    let token = bearer_token(&c).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(600))
        .build()?;
    for selected_path in selected {
        let path = selected_path
            .into_path()
            .map_err(|_| AppError::Config("Cannot access the selected source file".into()))?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|_| AppError::Config("Cannot read the selected source file".into()))?;
        if !metadata.is_file() || metadata.len() > 200 * 1024 * 1024 {
            return Err(AppError::Config(
                "Select regular files no larger than 200 MB".into(),
            ));
        }
        let part = reqwest::multipart::Part::file(&path)
            .await
            .map_err(|_| AppError::Config("Cannot read the selected source file".into()))?;
        let form = reqwest::multipart::Form::new()
            .text("datasetId", id.to_string())
            .part("data", part);
        let response = client
            .post(format!("{}/api/v1/add", api_url(&c)))
            .bearer_auth(&token)
            .multipart(form)
            .send()
            .await?;
        if !response.status().is_success() {
            return Err(AppError::Message(format!(
                "Cognee upload returned HTTP {}",
                response.status().as_u16()
            )));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn cognify_dataset(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
) -> Result<(), AppError> {
    let id =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let _active_work = ActiveWorkGuard::new(&state);
    let config = config_snapshot(&state)?;
    let token = bearer_token(&config).await?;
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(60 * 60))
        .build()?
        .post(format!("{}/api/v1/cognify", api_url(&config)))
        .bearer_auth(token)
        .json(&json!({"dataset_ids":[id],"run_in_background":false}))
        .send()
        .await?;
    if !response.status().is_success() {
        return Err(AppError::Message(format!(
            "Cognee memory build returned HTTP {}",
            response.status().as_u16()
        )));
    }
    let _ = response_json(response, 10 * 1024 * 1024).await?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatasetProgressDto {
    status: String,
    completed_items: Option<usize>,
    total_items: Option<usize>,
    current_stage: Option<String>,
}
#[tauri::command]
pub async fn get_dataset_progress(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
) -> Result<DatasetProgressDto, AppError> {
    let id =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let path = format!("/api/v1/datasets/status/progress?dataset={id}&pipeline=cognify_pipeline");
    let value = api_request(&state, reqwest::Method::GET, &path, None).await?;
    let dataset_key = id.to_string();
    let run = value.get(dataset_key.as_str()).unwrap_or(&Value::Null);
    let raw_status = run.get("status").and_then(Value::as_str).unwrap_or("idle");
    let status = match raw_status {
        "PipelineRunStarted"
        | "PipelineRunProgress"
        | "PipelineRunYield"
        | "pending"
        | "running"
        | "DATASET_PROCESSING_INITIATED"
        | "DATASET_PROCESSING_STARTED" => "running",
        "PipelineRunCompleted"
        | "PipelineRunAlreadyCompleted"
        | "completed"
        | "DATASET_PROCESSING_COMPLETED" => "completed",
        "PipelineRunErrored" | "failed" | "DATASET_PROCESSING_ERRORED" => "failed",
        _ => "idle",
    }
    .to_string();
    let progress = run
        .get("progress")
        .filter(|value| !value.is_null())
        .unwrap_or(run);
    Ok(DatasetProgressDto {
        status,
        completed_items: progress
            .get("completed_items")
            .and_then(Value::as_u64)
            .map(|v| v as usize),
        total_items: progress
            .get("total_items")
            .and_then(Value::as_u64)
            .map(|v| v as usize),
        current_stage: progress
            .get("current_stage")
            .and_then(Value::as_str)
            .map(|v| v.chars().take(120).collect()),
    })
}
#[tauri::command]
pub async fn download_raw(
    app: AppHandle,
    state: tauri::State<'_, AppState>,
    dataset_id: String,
    data_id: String,
    filename: String,
) -> Result<Option<String>, AppError> {
    let d =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let id = Uuid::parse_str(&data_id).map_err(|_| AppError::Config("Invalid data id".into()))?;
    let safe_name = Path::new(&filename)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("cognee-source.download")
        .chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
        .take(150)
        .collect::<String>();
    let selected_path = app
        .dialog()
        .file()
        .set_file_name(if safe_name.is_empty() {
            "cognee-source.download"
        } else {
            &safe_name
        })
        .blocking_save_file();
    let Some(selected_path) = selected_path else {
        return Ok(None);
    };
    let destination = selected_path
        .into_path()
        .map_err(|_| AppError::Config("Cannot access the selected destination".into()))?;
    let parent = destination
        .parent()
        .ok_or_else(|| AppError::Config("Choose a valid destination folder".into()))?;
    let c = config_snapshot(&state)?;
    let token = bearer_token(&c).await?;
    let r = reqwest::Client::new()
        .get(format!("{}/api/v1/datasets/{d}/data/{id}/raw", api_url(&c)))
        .bearer_auth(token)
        .send()
        .await?;
    if !r.status().is_success() {
        return Err(AppError::Message(format!(
            "Cognee raw download returned HTTP {}",
            r.status().as_u16()
        )));
    }
    if r.content_length()
        .is_some_and(|length| length > 200 * 1024 * 1024)
    {
        return Err(AppError::Message(
            "Downloaded file exceeds 200 MB limit".into(),
        ));
    }
    let temp = parent.join(format!(".cognee-{id}.partial"));
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .await
        .map_err(|_| AppError::Config("Cannot create temporary download".into()))?;
    let mut stream = r.bytes_stream();
    let mut total = 0usize;
    while let Some(chunk) = futures_util::StreamExt::next(&mut stream).await {
        let bytes = chunk.map_err(AppError::from)?;
        total += bytes.len();
        if total > 200 * 1024 * 1024 {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(AppError::Message(
                "Downloaded file exceeds 200 MB limit".into(),
            ));
        }
        tokio::io::AsyncWriteExt::write_all(&mut file, &bytes)
            .await
            .map_err(|_| AppError::Config("Cannot write downloaded source".into()))?;
    }
    drop(file);
    if tokio::fs::try_exists(&destination).await.unwrap_or(false) {
        tokio::fs::remove_file(&destination)
            .await
            .map_err(|_| AppError::Config("Cannot replace the selected destination".into()))?;
    }
    tokio::fs::rename(&temp, &destination)
        .await
        .map_err(|_| AppError::Config("Cannot finalize downloaded source".into()))?;
    Ok(Some(destination.to_string_lossy().to_string()))
}
#[tauri::command]
pub async fn preview_raw(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
    data_id: String,
    filename: String,
) -> Result<String, AppError> {
    let ext = PathBuf::from(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !["txt", "md", "csv", "json", "log", "yaml", "yml", "xml"].contains(&ext.as_str()) {
        return Err(AppError::Config(
            "Preview is available only for text-based files".into(),
        ));
    }
    let d =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let id = Uuid::parse_str(&data_id).map_err(|_| AppError::Config("Invalid data id".into()))?;
    let c = config_snapshot(&state)?;
    let token = bearer_token(&c).await?;
    let r = reqwest::Client::new()
        .get(format!("{}/api/v1/datasets/{d}/data/{id}/raw", api_url(&c)))
        .bearer_auth(token)
        .send()
        .await?;
    if !r.status().is_success() {
        return Err(AppError::Message(format!(
            "Cognee preview returned HTTP {}",
            r.status().as_u16()
        )));
    }
    if r.content_length().is_some_and(|n| n > 1024 * 1024) {
        return Err(AppError::Message(
            "Preview is limited to 1 MB; download the file instead".into(),
        ));
    }
    let b = bounded_body(r, 1024 * 1024).await.map_err(|_| {
        AppError::Message("Preview is limited to 1 MB; download the file instead".into())
    })?;
    Ok(String::from_utf8_lossy(&b).to_string())
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphDto {
    nodes: Vec<Value>,
    edges: Vec<Value>,
    truncated: bool,
}
#[tauri::command]
pub async fn get_graph(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
    query: Option<String>,
    max_nodes: Option<usize>,
) -> Result<GraphDto, AppError> {
    let d =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let cap = max_nodes.unwrap_or(500).clamp(1, 1000);
    let mut path = format!("/api/v1/visualize/json?dataset_id={d}&full=false&max_nodes={cap}");
    if let Some(q) = query {
        if !q.trim().is_empty() {
            if q.chars().count() > 2000 {
                return Err(AppError::Config(
                    "Graph search is limited to 2,000 characters".into(),
                ));
            }
            path.push_str("&query=");
            path.push_str(&url::form_urlencoded::byte_serialize(q.as_bytes()).collect::<String>());
        }
    }
    let v = api_request(&state, reqwest::Method::GET, &path, None).await?;
    let ns = value_array(v.get("nodes").cloned().unwrap_or(Value::Null));
    let es = value_array(
        v.get("links")
            .or_else(|| v.get("edges"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    let truncated = ns.len() >= cap;
    let nodes=ns.into_iter().take(cap).enumerate().map(|(i,n)|json!({"id":n.get("id").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(||format!("node-{i}")),"label":n.get("name").or_else(||n.get("label")).and_then(Value::as_str).unwrap_or("Entity"),"position":{"x":((i%8)*150)as i32,"y":((i/8)*100)as i32},"data":n})).collect();
    let edges=es.into_iter().take(4000).enumerate().filter_map(|(i,e)|{let source=e.get("source").or_else(||e.get("start")).and_then(Value::as_str)?;let target=e.get("target").or_else(||e.get("end")).and_then(Value::as_str)?;Some(json!({"id":e.get("id").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(||format!("edge-{i}")),"source":source,"target":target,"label":e.get("relationship_name").or_else(||e.get("relation")).or_else(||e.get("label")).and_then(Value::as_str)}))}).collect();
    Ok(GraphDto {
        nodes,
        edges,
        truncated,
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDto {
    session_id: String,
    summary: String,
    status: String,
    updated_at: String,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPageDto {
    items: Vec<SessionDto>,
    total: u64,
    has_more: bool,
}
#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, AppState>,
    range: String,
    status: Option<String>,
    limit: u16,
    offset: u32,
) -> Result<SessionPageDto, AppError> {
    let range = if ["24h", "7d", "30d", "all"].contains(&range.as_str()) {
        range
    } else {
        "7d".into()
    };
    let mut path = format!(
        "/api/v1/sessions?range={range}&limit={}&offset={}",
        limit.clamp(1, 200),
        offset
    );
    if let Some(s) = status {
        path.push_str("&status=");
        path.push_str(&url::form_urlencoded::byte_serialize(s.as_bytes()).collect::<String>());
    }
    let v = api_request(&state, reqwest::Method::GET, &path, None).await?;
    let items = v
        .get("sessions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|v| SessionDto {
            session_id: v
                .get("session_id")
                .or_else(|| v.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            summary: session_summary(v, 300),
            status: v
                .get("effective_status")
                .or_else(|| v.get("status"))
                .and_then(Value::as_str)
                .unwrap_or("unknown")
                .into(),
            updated_at: v
                .get("last_activity_at")
                .or_else(|| v.get("updated_at"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
        })
        .collect();
    Ok(SessionPageDto {
        items,
        total: v.get("total").and_then(Value::as_u64).unwrap_or(0),
        has_more: v.get("has_more").and_then(Value::as_bool).unwrap_or(false),
    })
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionDetailDto {
    session_id: String,
    summary: String,
    status: String,
    updated_at: String,
    started_at: String,
    ended_at: String,
}
#[tauri::command]
pub async fn get_session(
    state: tauri::State<'_, AppState>,
    session_id: String,
) -> Result<SessionDetailDto, AppError> {
    if session_id.is_empty()
        || session_id.chars().count() > 200
        || session_id.chars().any(char::is_control)
    {
        return Err(AppError::Config("Invalid session id".into()));
    }
    let id = url::form_urlencoded::byte_serialize(session_id.as_bytes()).collect::<String>();
    let v = api_request(
        &state,
        reqwest::Method::GET,
        &format!("/api/v1/sessions/{id}"),
        None,
    )
    .await?;
    Ok(SessionDetailDto {
        session_id,
        summary: session_summary(&v, 1000),
        status: v
            .get("effective_status")
            .or_else(|| v.get("status"))
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .into(),
        updated_at: v
            .get("last_activity_at")
            .or_else(|| v.get("updated_at"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        started_at: v
            .get("started_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        ended_at: v
            .get("ended_at")
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
    })
}

#[cfg(test)]
mod dto_mapping_tests {
    use super::{data_size_bytes, session_summary};
    use serde_json::json;

    #[test]
    fn data_size_accepts_cognee_camel_case_field() {
        assert_eq!(data_size_bytes(&json!({"dataSize": 512})), Some(512));
        assert_eq!(
            data_size_bytes(&json!({"data_size": null, "dataSize": 512})),
            Some(512)
        );
    }

    #[test]
    fn session_summary_uses_safe_api_label_when_summary_is_missing() {
        let row =
            json!({"summary": null, "label": "A safe session label", "traces": ["private trace"]});
        assert_eq!(session_summary(&row, 100), "A safe session label");
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::{runtime_has_fastembed, runtime_needs_repair, runtime_state_after_check};
    use crate::state::RuntimeState;
    use std::{fs, path::PathBuf};
    use uuid::Uuid;

    #[test]
    fn runtime_is_not_ready_when_fastembed_is_missing() {
        let runtime_dir: PathBuf = std::env::temp_dir()
            .join("cognee-runtime-readiness")
            .join(Uuid::new_v4().to_string());
        let python = runtime_dir.join(".venv/Scripts/python.exe");
        fs::create_dir_all(python.parent().unwrap()).unwrap();
        fs::write(&python, b"test interpreter").unwrap();

        assert!(!runtime_has_fastembed(&runtime_dir));

        fs::remove_dir_all(runtime_dir).unwrap();
    }

    #[test]
    fn runtime_is_ready_when_python_and_fastembed_are_present() {
        let runtime_dir: PathBuf = std::env::temp_dir()
            .join("cognee-runtime-readiness")
            .join(Uuid::new_v4().to_string());
        let python = runtime_dir.join(".venv/Scripts/python.exe");
        let fastembed = runtime_dir.join(".venv/Lib/site-packages/fastembed/__init__.py");
        fs::create_dir_all(python.parent().unwrap()).unwrap();
        fs::create_dir_all(fastembed.parent().unwrap()).unwrap();
        fs::write(&python, b"test interpreter").unwrap();
        fs::write(&fastembed, b"test package").unwrap();

        assert!(runtime_has_fastembed(&runtime_dir));

        fs::remove_dir_all(runtime_dir).unwrap();
    }

    #[test]
    fn runtime_repair_is_selected_for_missing_or_unimportable_python() {
        assert!(runtime_needs_repair(false, false));
        assert!(runtime_needs_repair(true, false));
        assert!(!runtime_needs_repair(true, true));
    }

    #[test]
    fn status_refresh_preserves_preparing_during_runtime_repair() {
        assert_eq!(
            runtime_state_after_check(RuntimeState::Preparing, true),
            RuntimeState::Preparing
        );
        assert_eq!(
            runtime_state_after_check(RuntimeState::Preparing, false),
            RuntimeState::Preparing
        );
    }
}
