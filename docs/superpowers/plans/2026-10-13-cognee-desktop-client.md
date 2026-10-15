# Cognee Desktop Client Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship an installable Windows x64 Tauri desktop client that prepares a local Cognee runtime, supervises API/MCP services, and manages source files and memory through Cognee APIs.

**Architecture:** Add a standalone `cognee-desktop/` Tauri 2 application. Rust owns bootstrap, secrets, process supervision, API calls, filesystem access, and log redaction; Vue consumes typed Tauri commands/events. The client manages a single local Cognee API and the FastMCP 4 MCP service implemented by the preceding `2026-10-13-cognee-mcp-fastmcp4.md` plan.

**Tech Stack:** Rust stable; Tauri 2.11+; Vue 3.5.x; TypeScript; Vite 8; Pinia; Vue Router; Tailwind CSS; Element Plus; Vue Flow; Vitest; `uv` and locked Python runtime dependencies; Windows x64 NSIS installer.

**Spec:** `docs/superpowers/specs/2026-10-13-cognee-local-mcp-desktop-design.md`

## Global Constraints

- First release supports Windows x64 only.
- Use Tauri 2.11+, Rust stable, Vue 3.5.x, TypeScript, Vite 8, and Pinia.
- The desktop client is independent under `cognee-desktop/`; keep `cognee-frontend/` unchanged.
- Bind Cognee API to `127.0.0.1:12315` and MCP to `127.0.0.1:12316` by default; expose the MCP endpoint at `/mcp-new` and legacy route at `/mcp`.
- Prepare a locked Python runtime in app data; never install into the user's system Python.
- Use Windows-protected credential storage and do not write secrets to ordinary config, logs, process arguments, or UI read-back.
- Use Cognee API routes for data operations; do not access Cognee database files directly from Rust or Vue.
- Use a Windows Job Object to contain supervised API and MCP process trees.
- Preserve source data and provider settings across application upgrades and document uninstall retention.

## Review Focus

- First launch without network must show a recoverable bootstrap error and retain the partial runtime for retry without starting Python children.
- A local port conflict must be reported with the configured port and must not terminate the unrelated process.
- API credentials expire or are unavailable after restart; the desktop must reauthenticate and refresh its protected MCP API key before MCP starts.
- API file-download responses may be large or use unsupported storage schemes; download must stream to a safe temp path and show the Cognee error without freezing the UI.
- Graph payloads and source filenames/properties may be large or malformed; UI rendering must be bounded and Rust path handling must reject traversal.

---

### Task 1: Scaffold the independent Tauri/Vue desktop project

**Files:**
- Create: `cognee-desktop/package.json`
- Create: `cognee-desktop/package-lock.json`
- Create: `cognee-desktop/index.html`
- Create: `cognee-desktop/vite.config.ts`
- Create: `cognee-desktop/tsconfig.json`
- Create: `cognee-desktop/src/main.ts`
- Create: `cognee-desktop/src/App.vue`
- Create: `cognee-desktop/src/router/index.ts`
- Create: `cognee-desktop/src/layouts/AppShell.vue`
- Create: `cognee-desktop/src/views/DashboardView.vue`
- Create: `cognee-desktop/src/views/McpView.vue`
- Create: `cognee-desktop/src/views/DatasetsView.vue`
- Create: `cognee-desktop/src/views/GraphView.vue`
- Create: `cognee-desktop/src/views/SessionsView.vue`
- Create: `cognee-desktop/src/views/SettingsView.vue`
- Create: `cognee-desktop/src/views/LogsView.vue`
- Create: `cognee-desktop/src-tauri/Cargo.toml`
- Create: `cognee-desktop/src-tauri/Cargo.lock`
- Create: `cognee-desktop/src-tauri/build.rs`
- Create: `cognee-desktop/src-tauri/src/main.rs`
- Create: `cognee-desktop/src-tauri/src/lib.rs`
- Create: `cognee-desktop/src-tauri/tauri.conf.json`
- Create: `cognee-desktop/src-tauri/capabilities/default.json`

**Interfaces:**
- Consumes: None. Use `E:\home\mcp-chrome-super\app\desktop-client` as a Tauri/Vue/Vite structural reference, not as a code dependency.
- Produces: `npm run tauri:dev`, `npm run typecheck`, `npm run build`, `npm run test:unit`, and `npm run tauri:build` scripts; a Tauri shell with no broad filesystem or shell permissions.

- [ ] Create the Vue 3.5 + TypeScript + Vite 8 + Pinia + Vue Router app and Tauri 2.11+ Rust shell. Add Tailwind, Element Plus, Vue Flow for graph display, and Vitest/Vue Test Utils for focused UI behavior checks.
- [ ] Configure Vite dev host as loopback and Tauri build output under `dist/`.
- [ ] Add minimal app navigation and empty route views for dashboard, MCP, datasets, graph, sessions, settings, and logs.
- [ ] Restrict Tauri capabilities to the commands/events this application needs; do not grant arbitrary shell execution or broad filesystem access.
- [ ] Run `npm run typecheck` and `cargo check --manifest-path src-tauri/Cargo.toml` from `cognee-desktop`.

### Task 2: Define Rust application state, configuration, and secret storage

**Files:**
- Create: `cognee-desktop/src-tauri/src/state.rs`
- Create: `cognee-desktop/src-tauri/src/config.rs`
- Create: `cognee-desktop/src-tauri/src/secrets.rs`
- Create: `cognee-desktop/src-tauri/src/error.rs`
- Create: `cognee-desktop/src-tauri/tests/secrets.rs`
- Modify: `cognee-desktop/src-tauri/Cargo.toml`
- Modify: `cognee-desktop/src-tauri/src/lib.rs`
- Create: `cognee-desktop/src/stores/app.ts`
- Create: `cognee-desktop/src/types/app.ts`

**Interfaces:**
- Produces Rust `AppState`, `AppConfig`, `ServiceStatus`, `BootstrapStatus`, and serializable `AppError` types.
- Produces secure secret operations `put_secret(name: &str, value: &str)`, `get_secret(name: &str) -> Result<Option<String>, AppError>`, and `delete_secret(name: &str)` backed by Windows Credential Manager.
- Produces Pinia `useAppStore()` for config summary and runtime/service state, with no secret values.

- [ ] Define config defaults: API host `127.0.0.1`, API port `12315`, MCP host `127.0.0.1`, MCP port `12316`, data root under app-local data, and start-on-login disabled.
- [ ] Persist non-secret config as versioned JSON in Tauri app config; store provider credentials, generated default-user password, `FASTAPI_USERS_JWT_SECRET`, and Cognee API key in Windows Credential Manager.
- [ ] Ensure serialization and Debug output omit secret values; redact secret names and values before returning errors to the frontend.
- [ ] Add `secrets.rs` coverage proving secrets survive a protected-store round trip and never appear in serialized `AppConfig` or `AppError` output.
- [ ] Register managed state in `lib.rs` and add only explicit secret-presence/replace/remove commands to the frontend bridge.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test secrets`, `cargo check`, and `npm run typecheck`.

### Task 3: Build the locked first-run Cognee runtime bootstrap

**Files:**
- Create: `cognee-desktop/runtime/pyproject.toml`
- Create: `cognee-desktop/runtime/uv.lock`
- Create: `cognee-desktop/resources/uv/uv.exe` (generated by packaging workflow; do not hand-edit)
- Create: `cognee-desktop/scripts/fetch-uv.ps1`
- Create: `cognee-desktop/src-tauri/src/bootstrap.rs`
- Create: `cognee-desktop/src-tauri/tests/bootstrap.rs`
- Modify: `cognee-desktop/src-tauri/src/lib.rs`
- Modify: `cognee-desktop/src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: `AppState`, app config/data directories, and `uv.exe` pinned by version and SHA-256.
- Produces: `prepare_runtime(cancel: CancellationToken) -> Result<RuntimeInfo, AppError>` plus `bootstrap-progress` Tauri events with phase, percent when known, and non-secret message.
- `RuntimeInfo` contains Python executable, API/MCP entry points, runtime version, and lock hash.

- [ ] Add a dedicated runtime project referencing the repository Cognee package and `cognee-mcp` package, with only runtime dependencies and one supported Python minor version pinned to a specific patch release.
- [ ] Resolve the smallest dependency set that supports the Cognee API, default local graph/vector stores, API authentication, and all MCP tools; keep Windows wheels pinned in `uv.lock` and record installed size/network estimate in the desktop README.
- [ ] Add a packaging script that downloads the pinned Windows x64 uv executable, verifies SHA-256, and places it in Tauri resources.
- [ ] Implement async `uv python install`, venv creation, and `uv sync --locked` under the app runtime directory; never mutate global Python or user PATH.
- [ ] Emit progress, support cancellation before service startup, and preserve retryable partial files after a bootstrap error.
- [ ] Add a fake-uv bootstrap test where download fails: assert a retryable error is emitted and neither API nor MCP process starts.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test bootstrap` and manually bootstrap a clean temporary app-data directory to confirm `python -m cognee.api.client` and `cognee-mcp` resolve from the locked environment.

### Task 4: Implement safe Windows process supervision and log handling

**Files:**
- Create: `cognee-desktop/src-tauri/src/process_supervisor.rs`
- Create: `cognee-desktop/src-tauri/src/logs.rs`
- Create: `cognee-desktop/src-tauri/src/service.rs`
- Create: `cognee-desktop/src-tauri/tests/process_supervisor.rs`
- Modify: `cognee-desktop/src-tauri/Cargo.toml`
- Modify: `cognee-desktop/src-tauri/src/state.rs`

**Interfaces:**
- `start_api() -> Result<ServiceStatus, AppError>`
- `start_mcp(api_key: &SecretString) -> Result<ServiceStatus, AppError>`
- `stop_service(service: ServiceKind) -> Result<ServiceStatus, AppError>`
- `stop_all() -> Result<(), AppError>`
- `subscribe_logs(service: ServiceKind) -> LogSubscription`
- Service kinds are `Api` and `Mcp`; statuses are `Stopped`, `Starting`, `Running`, `Stopping`, and `Failed`.

- [ ] Launch child processes with `CREATE_NO_WINDOW`, restricted working directories, explicit environment maps, and no credentials in arguments.
- [ ] Assign each child tree to a Windows Job Object configured to terminate remaining children when the job closes.
- [ ] Start API on `127.0.0.1:12315` via `.venv/Scripts/python.exe -m cognee.api.client`, poll `/health` until ready or a bounded timeout, then permit MCP startup. Detect port conflicts and report them without killing the listener.
- [ ] Start MCP via `.venv/Scripts/cognee-mcp.exe --transport http-new --host 127.0.0.1 --port 12316 --api-url http://127.0.0.1:12315`; accept the API key as an in-memory secret and inject it only as `COGNEE_API_KEY` with `COGNEE_API_AUTH_SCHEME=x-api-key` in the child environment.
- [ ] Add graceful shutdown followed by bounded forced termination; application exit must await `stop_all()`.
- [ ] Stream stdout/stderr to rotating app log files, redact configured secret values and authorization headers, and emit redacted log events to Vue.
- [ ] Add a process-supervisor integration test with a loopback listener on the configured port: assert startup reports a conflict and leaves the unrelated listener alive; assert stopping a fixture parent terminates its child process.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test process_supervisor`, `cargo check`; manually inspect process arguments, log files, and process tree after start/stop.

### Task 5: Add Cognee API client and protected local authentication

**Files:**
- Create: `cognee-desktop/src-tauri/src/cognee_api.rs`
- Create: `cognee-desktop/src-tauri/src/auth.rs`
- Create: `cognee-desktop/src-tauri/tests/cognee_api.rs`
- Modify: `cognee-desktop/src-tauri/src/service.rs`
- Modify: `cognee-desktop/src-tauri/src/lib.rs`

**Interfaces:**
- `CogneeApiClient::new(base_url: Url, auth: ApiAuth)` where `ApiAuth` is `Bearer(SecretString)` or `ApiKey(SecretString)`.
- `CogneeApiClient::login(username: &str, password: &SecretString) -> Result<SecretString, AppError>`
- `CogneeApiClient::create_mcp_api_key(token: &SecretString) -> Result<SecretString, AppError>`
- `ensure_local_api_key() -> Result<SecretString, AppError>` obtains the default-user bearer token and creates/reuses a named local API key after API startup.
- `CogneeApiClient` methods: `list_datasets`, `list_data(dataset_id, limit, offset)`, `upload_source(dataset_id, path)`, `get_pipeline_progress(dataset_id)`, `download_raw(dataset_id, data_id, destination)`, `delete_data(dataset_id, data_id)`, `delete_dataset(dataset_id)`, `get_graph(dataset_id, max_nodes, query)`, `list_sessions(range, status, limit, offset)`, `get_session(session_id)`.

- [ ] Start the Cognee API with `ENABLE_BACKEND_ACCESS_CONTROL=true`, `REQUIRE_AUTHENTICATION=true`, generated `DEFAULT_USER_EMAIL=default_user@example.com`, `DEFAULT_USER_PASSWORD`, `FASTAPI_USERS_JWT_SECRET`, `HTTP_API_HOST=127.0.0.1`, `HTTP_API_PORT=12315`, and app-owned data paths.
- [ ] After API readiness, log in through `/api/v1/auth/login` using Bearer auth, create a named MCP API key through `/api/v1/auth/api-keys`, and save the API key and password only in Windows Credential Manager.
- [ ] Implement authenticated API requests with bounded timeouts, response-size limits for JSON, and streaming file downloads to sanitized temporary paths.
- [ ] Reuse and validate the Credential Manager API key with `/api/v1/auth/me`; when it is missing or rejected, reauthenticate and create a replacement before MCP startup. Pass the key to the MCP process only as `COGNEE_API_KEY` with `COGNEE_API_AUTH_SCHEME=x-api-key`; never place it in arguments.
- [ ] Map authorization, port, missing-data, and unsupported raw-storage errors to actionable `AppError` values without including secret-bearing response headers.
- [ ] Add mocked API tests for login, API-key creation, expired-key recovery, large streaming downloads, unsupported storage responses, and rejecting a filename containing path traversal.
- [ ] Run `cargo test --manifest-path src-tauri/Cargo.toml --test cognee_api`, `cargo check`; manually check login, key creation, protected persistence/reload, and API error mapping on disposable app data.

### Task 6: Expose typed Tauri commands and desktop service dashboard

**Files:**
- Create: `cognee-desktop/src-tauri/src/commands.rs`
- Create: `cognee-desktop/src-tauri/src/mcp_client.rs`
- Modify: `cognee-desktop/src-tauri/src/lib.rs`
- Create: `cognee-desktop/src/components/ServiceStatusCard.vue`
- Create: `cognee-desktop/src/components/LogPanel.vue`
- Create: `cognee-desktop/src/components/McpEndpointCard.vue`
- Modify: `cognee-desktop/src/views/McpView.vue`
- Create: `cognee-desktop/src/stores/app.test.ts`
- Modify: `cognee-desktop/src/views/DashboardView.vue`
- Modify: `cognee-desktop/src/stores/app.ts`
- Modify: `cognee-desktop/src/types/app.ts`

**Interfaces:**
- Commands: `get_app_status`, `prepare_runtime`, `start_api`, `start_mcp`, `stop_api`, `stop_mcp`, `stop_all`, `get_recent_logs`, and `list_mcp_tools`.
- Events: `bootstrap-progress`, `service-status`, and `service-log`.

- [ ] Register typed commands and events; `start_api` runs bootstrap/readiness/auth setup, then `start_mcp` passes the in-memory API key to the supervisor. Return only status, safe config summaries, progress, and redacted logs.
- [ ] Implement `list_mcp_tools()` as a protocol-aware `initialize` then `tools/list` exchange against the local `/mcp-new` endpoint; return only tool names, descriptions, and input schemas.
- [ ] Build dashboard cards for runtime/API/MCP states, dataset count, endpoint URL `http://127.0.0.1:12316/mcp-new`, copy URL, start/stop/retry, and open-log actions. Add an MCP view for `/mcp-new`, legacy `/mcp`, tools, transport status, and connection instructions.
- [ ] Show partial service failures independently: API can remain visibly healthy while MCP is retried; API loss stops dependent MCP and updates status.
- [ ] Add confirmation for stopping all services only when background ingestion is active; present active-work status before confirmation.
- [ ] Test dashboard rendering for API-only failure and MCP-only failure; assert the MCP URL always matches the active configured host and port.
- [ ] Run the focused Vitest file, frontend typecheck/build, and Rust `cargo check`.

### Task 7: Implement dataset and source-file management

**Files:**
- Modify: `cognee-desktop/src/views/DatasetsView.vue`
- Create: `cognee-desktop/src/components/DataRecordTable.vue`
- Create: `cognee-desktop/src/components/ConfirmDeleteDialog.vue`
- Create: `cognee-desktop/src/stores/memory.ts`
- Create: `cognee-desktop/src/types/memory.ts`
- Create: `cognee-desktop/src/stores/memory.test.ts`
- Modify: `cognee-desktop/src-tauri/src/commands.rs`
- Modify: `cognee-desktop/src-tauri/src/cognee_api.rs`

**Interfaces:**
- Frontend store methods: `loadDatasets()`, `loadDataPage(datasetId, limit, offset)`, `uploadSource(datasetId, path)`, `downloadRaw(datasetId, dataId)`, `deleteData(datasetId, dataId)`, and `deleteDataset(datasetId)`.
- Data screens consume DTOs normalized in Rust from the Cognee API; no raw API token crosses the Tauri bridge.

- [ ] Implement dataset list/create/delete and data-record paging using `GET/POST /api/v1/datasets/`, `GET /api/v1/datasets/{dataset_id}/data`, and the scoped delete routes.
- [ ] Implement source metadata, safe preview for text-based formats, download for unsupported formats, and per-record delete. Require a confirmation dialog showing dataset and filename for deletes.
- [ ] Implement `upload_source` through `POST /api/v1/add` with `datasetId` and the file multipart field; poll `/api/v1/datasets/status/progress?datasets=<dataset-id>` at a bounded cadence. Do not promise cancellation because the current Cognee API does not expose a matching general cancellation route.
- [ ] Test the memory store with a source filename containing path separators; assert the download path remains inside app temp storage and destructive actions require explicit target confirmation.
- [ ] Run the focused Vitest file, frontend typecheck/build, and Rust `cargo check`; manually browse a dataset with one source file.

### Task 8: Implement bounded graph memory visualization

**Files:**
- Modify: `cognee-desktop/src/views/GraphView.vue`
- Create: `cognee-desktop/src/components/GraphCanvas.vue`
- Create: `cognee-desktop/src/stores/graph.ts`
- Create: `cognee-desktop/src/types/graph.ts`
- Create: `cognee-desktop/src/stores/graph.test.ts`
- Modify: `cognee-desktop/src-tauri/src/commands.rs`
- Modify: `cognee-desktop/src-tauri/src/cognee_api.rs`

**Interfaces:**
- `loadGraph(datasetId: string, query?: string) -> Promise<GraphDto>` requests `/api/v1/visualize/json` with `max_nodes <= 1000` by default and never sets `full=true`.
- Graph DTO includes normalized nodes, links, optional provenance, and truncation/count metadata.

- [ ] Request bounded graph data, normalize nodes and links in Rust, and return truncation metadata to the frontend.
- [ ] Render through Vue Flow with node/edge selection, search, and loading/error states; avoid full-graph requests by default.
- [ ] Test malformed nodes/links and oversized data; assert rendering stays under the configured node cap and reports truncation.
- [ ] Run the focused Vitest file, frontend typecheck/build, and Rust `cargo check`; manually inspect a real graph and node provenance.

### Task 9: Add session record browsing

**Files:**
- Modify: `cognee-desktop/src/views/SessionsView.vue`
- Create: `cognee-desktop/src/stores/sessions.ts`
- Create: `cognee-desktop/src/types/sessions.ts`
- Create: `cognee-desktop/src/stores/sessions.test.ts`
- Modify: `cognee-desktop/src-tauri/src/commands.rs`
- Modify: `cognee-desktop/src-tauri/src/cognee_api.rs`

**Interfaces:**
- `loadSessions(filters: { range: '24h' | '7d' | '30d' | 'all'; status?: string; limit: number; offset: number })`.
- `loadSession(sessionId: string)` returns an allowlisted session DTO without raw trace payloads.

- [ ] Implement session list filters and pagination with `/api/v1/sessions`; show safe metadata and effective status.
- [ ] Implement detail loading with `/api/v1/sessions/{session_id}` and allowlist summary fields; do not forward raw trace values to Vue.
- [ ] Test status/range filters and omission of trace payloads from the frontend DTO.
- [ ] Run the focused Vitest file, frontend typecheck/build, and Rust `cargo check`; manually inspect a session list and detail.

### Task 10: Add settings and provider configuration

**Files:**
- Modify: `cognee-desktop/src/views/SettingsView.vue`
- Create: `cognee-desktop/src/stores/settings.ts`
- Create: `cognee-desktop/src/types/settings.ts`
- Modify: `cognee-desktop/src-tauri/src/config.rs`
- Modify: `cognee-desktop/src-tauri/src/secrets.rs`
- Modify: `cognee-desktop/src-tauri/src/commands.rs`
- Modify: `cognee-desktop/src-tauri/src/service.rs`

**Interfaces:**
- Commands: `get_settings_summary`, `save_settings(SettingsInput)`, `set_provider_secret(SecretKind, String)`, and `clear_provider_secret(SecretKind)`.
- Secret kind is an enum for LLM, embedding, vector-store, and generated local API credentials; raw secrets are write-only in UI responses.

- [ ] Persist provider/model/endpoint fields and secret references in app config. Inject Cognee's supported environment fields (including `LLM_PROVIDER`, `LLM_MODEL`, `LLM_ENDPOINT`, `LLM_API_KEY`, `EMBEDDING_PROVIDER`, `EMBEDDING_MODEL`, `EMBEDDING_API_KEY`, `GRAPH_DATABASE_PROVIDER`, `VECTOR_DB_PROVIDER`, `VECTOR_DB_URL`, and `VECTOR_DB_KEY`) into the API child at launch. Keep data root and service ports in app config.
- [ ] Store API/model/vector credentials only in Credential Manager. Do not call `/api/v1/settings`: its response contains API key fields and its update payload writes API keys into Cognee settings. Do not persist a plaintext environment file.
- [ ] Add validation for provider/endpoint and show whether required provider settings are present without echoing secret values.
- [ ] Restart the API and dependent MCP when provider settings change; explain that active pipelines may be interrupted before applying the change.
- [ ] Test settings summaries and serialization to assert raw secrets never appear in frontend responses or ordinary config JSON.
- [ ] Run the focused Vitest file, frontend typecheck/build, and Rust `cargo check`; manually inspect persisted config and logs for plaintext secret leakage.

### Task 11: Package Windows x64 installer and document lifecycle

**Files:**
- Modify: `cognee-desktop/src-tauri/tauri.conf.json`
- Create: `cognee-desktop/README.md`
- Create: `cognee-desktop/LICENSES.md`
- Create: `.github/workflows/cognee-desktop-windows.yml`
- Create: `cognee-desktop/scripts/package-runtime.ps1`
- Create: `cognee-desktop/scripts/sign-installer.ps1`

**Interfaces:**
- Produces a Windows x64 NSIS installer with bundled Vue assets and pinned `uv.exe`; runtime Python/packages prepare on first launch into app data.
- Documents dev, runtime bootstrap, packaging, upgrade, uninstall/data retention, network requirements, and provider configuration.

- [ ] Configure NSIS packaging, application identity, icons, updater-disabled first-release policy, and resource inclusion for the verified `uv.exe`.
- [ ] Add optional Authenticode signing that reads a PFX and password only from CI secret variables; local artifacts without a configured certificate must be clearly marked unsigned.
- [ ] Make packaging fail if uv checksum verification or lockfile synchronization fails.
- [ ] Add Windows CI steps for Rust target installation, locked npm install, `npm run typecheck`, frontend build, Rust `cargo check`, and installer build.
- [ ] Include dependency license notices for shipped bootstrap/runtime components and document runtime download/licensing responsibilities.
- [ ] Build the installer on Windows x64 and run a clean-machine smoke check for install, first-run bootstrap, API/MCP startup, stop, upgrade preserving user data, and uninstall retention behavior.
