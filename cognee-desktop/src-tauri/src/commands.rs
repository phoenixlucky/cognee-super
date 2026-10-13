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
    })
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
        if data.join("runtime/.venv/Scripts/python.exe").exists() {
            *runtime = RuntimeState::Ready;
        }
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
        sync.args(["sync", "--locked", "--project"])
            .arg(project)
            .arg("--python")
            .arg("3.12.10")
            .env("UV_PYTHON_INSTALL_DIR", &python_dir)
            .env("UV_CACHE_DIR", &cache)
            .env("UV_PROJECT_ENVIRONMENT", venv)
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
    let (_, data) = app_paths(app)?;
    Ok((data.join("runtime/.venv/Scripts/python.exe"), data))
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
        if let Ok(Some(v)) = secrets::get_secret(name) {
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
    if !runtime_paths(&app)?.0.exists() {
        prepare_runtime(app.clone(), state.clone()).await?;
    }
    let c = config_snapshot(&state)?;
    ensure_port_free(c.api_port)?;
    set_state(&state.api_status, RunState::Starting)?;
    let (python, data) = runtime_paths(&app)?;
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
    let client = reqwest::Client::new();
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
    let body: Value = response.json().await?;
    body.get("access_token")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AppError::Message("Cognee did not return an access token".into()))
}
async fn ensure_mcp_key(c: &AppConfig) -> Result<String, AppError> {
    if let Some(key) = secrets::get_secret("local.mcp_api_key")? {
        let r = reqwest::Client::new()
            .get(format!("{}/api/v1/auth/me", api_url(c)))
            .header("x-api-key", &key)
            .send()
            .await?;
        if r.status().is_success() {
            return Ok(key);
        }
    }
    let token = bearer_token(c).await?;
    let r = reqwest::Client::new()
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
    let v: Value = r.json().await?;
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
    let exe = python.with_file_name("cognee-mcp.exe");
    let mut cmd = Command::new(exe);
    cmd.args([
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
    let req = json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{"protocolVersion":"2026-07-28","clientInfo":{"name":"cognee-desktop","version":"0.1.0"},"capabilities":{}}}});
    let r = reqwest::Client::new()
        .post(format!("http://127.0.0.1:{}/mcp-new", c.mcp_port))
        .header("content-type", "application/json")
        .header("accept", "application/json, text/event-stream")
        .header("x-api-key", key)
        .json(&req)
        .send()
        .await?;
    let t = r.text().await?;
    let body = t
        .lines()
        .find(|l| l.starts_with("data: "))
        .map(|l| &l[6..])
        .unwrap_or(&t);
    let v: Value = serde_json::from_str(body)
        .map_err(|_| AppError::Message("MCP returned an unsupported tools response".into()))?;
    let tools = v
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let _ = app;
    Ok(tools
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
        .collect())
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
    let mut req = reqwest::Client::new()
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
    r.json().await.map_err(AppError::from)
}
fn value_array(v: Value) -> Vec<Value> {
    v.as_array()
        .cloned()
        .or_else(|| v.get("items").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}
#[tauri::command]
pub async fn list_datasets(state: tauri::State<'_, AppState>) -> Result<Vec<DatasetDto>, AppError> {
    Ok(
        value_array(api_request(&state, reqwest::Method::GET, "/api/v1/datasets/", None).await?)
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
        "/api/v1/datasets/",
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
    state: tauri::State<'_, AppState>,
    dataset_id: String,
    path: String,
) -> Result<(), AppError> {
    let id =
        Uuid::parse_str(&dataset_id).map_err(|_| AppError::Config("Invalid dataset id".into()))?;
    let p = PathBuf::from(path);
    if !p.is_file() {
        return Err(AppError::Config(
            "Selected source file does not exist".into(),
        ));
    }
    let c = config_snapshot(&state)?;
    let token = bearer_token(&c).await?;
    let part = reqwest::multipart::Part::file(&p)
        .await
        .map_err(|_| AppError::Config("Cannot read the selected source file".into()))?;
    let form = reqwest::multipart::Form::new()
        .text("datasetId", id.to_string())
        .part("data", part);
    let r = reqwest::Client::new()
        .post(format!("{}/api/v1/add", api_url(&c)))
        .bearer_auth(token)
        .multipart(form)
        .send()
        .await?;
    if !r.status().is_success() {
        return Err(AppError::Message(format!(
            "Cognee upload returned HTTP {}",
            r.status().as_u16()
        )));
    }
    Ok(())
}
#[tauri::command]
pub async fn download_raw(
    state: tauri::State<'_, AppState>,
    dataset_id: String,
    data_id: String,
) -> Result<String, AppError> {
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
            "Cognee raw download returned HTTP {}",
            r.status().as_u16()
        )));
    }
    let data = state
        .runtime_path
        .lock()
        .map_err(|_| AppError::Config("Data path unavailable".into()))?
        .as_ref()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::Config("App has not initialized".into()))?;
    let dir = data.join("downloads");
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|_| AppError::Config("Cannot create download directory".into()))?;
    let path = dir.join(format!("{id}.download"));
    let temp = dir.join(format!("{id}.download.tmp"));
    let mut file = tokio::fs::File::create(&temp)
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
    tokio::fs::rename(&temp, &path)
        .await
        .map_err(|_| AppError::Config("Cannot finalize downloaded source".into()))?;
    Ok(path.to_string_lossy().to_string())
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
    let b = r.bytes().await?;
    if b.len() > 1024 * 1024 {
        return Err(AppError::Message(
            "Preview is limited to 1 MB; download the file instead".into(),
        ));
    }
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
#[tauri::command]
pub async fn list_sessions(
    state: tauri::State<'_, AppState>,
    range: String,
    status: Option<String>,
    limit: u16,
    offset: u32,
) -> Result<Vec<SessionDto>, AppError> {
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
    Ok(value_array(v)
        .iter()
        .map(|v| SessionDto {
            session_id: v
                .get("session_id")
                .or_else(|| v.get("id"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            summary: v
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .chars()
                .take(300)
                .collect(),
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
        .collect())
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
    let id =
        Uuid::parse_str(&session_id).map_err(|_| AppError::Config("Invalid session id".into()))?;
    let v = api_request(
        &state,
        reqwest::Method::GET,
        &format!("/api/v1/sessions/{id}"),
        None,
    )
    .await?;
    Ok(SessionDetailDto {
        session_id: id.to_string(),
        summary: v
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .chars()
            .take(1000)
            .collect(),
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
