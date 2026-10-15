# Cognee Desktop

Windows x64 desktop client for managing a local Cognee API, its FastMCP 4 server, source files, datasets, sessions, and bounded graph views.

## Development

Requirements: Windows x64, Rust stable with the MSVC toolchain, Node.js 24+, and WebView2.

```powershell
npm ci
pwsh -ExecutionPolicy Bypass -File scripts/fetch-uv.ps1
pwsh -ExecutionPolicy Bypass -File scripts/package-runtime.ps1
npm run tauri:dev
```

The desktop app is a separate project; `cognee-frontend/` is not part of this client. The UI stack is Vue 3.5, TypeScript, Vite 8, Pinia, Vue Router, Tailwind CSS, Element Plus, and Vue Flow. Tauri commands are narrowly registered and the webview receives no arbitrary shell or filesystem access.

Quality checks:

```powershell
npm run typecheck
npm run test:unit
npm run build
$env:CARGO_TARGET_DIR = 'E:\build\cognee-desktop'
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --test secrets
```

Build the Windows x64 NSIS installer with `npm run tauri:build`. The packaging scripts refresh the MCP source snapshot from this repository and verify the bundled uv executable before building.

Create a no-installer portable ZIP from a completed release build with `npm run package:portable`. Packaging downloads and includes CPython 3.12.10 and the locked production dependencies in the ZIP. Extract it to a writable folder and start `Cognee Desktop.exe`; first launch does not need to download Python or install packages. App data and credentials remain in the current Windows user's app-data directory and Credential Manager, so the ZIP itself can be moved but does not carry memory data between PCs. Model files may still download when first used.

## First launch and runtime

The installer bundles the Tauri application, the Cognee MCP source snapshot, its checked-in `uv.lock`, and uv 0.11.19. It does not bundle Python or the Python wheels. On first setup, uv installs CPython 3.12.10 into Cognee Desktop's app-data directory and runs `uv sync --locked` there. This keeps Python packages out of system Python and the user's PATH. The portable ZIP bundles the same Python runtime and production dependencies under its `runtime` directory and runs them from there.

First setup needs network access to the Astral uv/Python distribution and the Python package index. The lock resolves 322 package records across supported platforms. Runtime sync excludes the project's development group. As a Windows x64 planning estimate, the current MCP development environment occupies about 2.28 GiB; production runtime size varies with the selected wheels. Keep several GiB free for first setup and its download cache. Runtime cache files stay under app data and can be removed after a successful setup if disk space is needed. Python is installed without creating executable shims in the user's PATH.

Before starting the API, the desktop checks that the managed Python environment can import FastEmbed and the bundled MCP package. If either package is missing or broken, it repairs the environment with the bundled lockfile and may download missing packages; repair needs network access when those packages are not cached. The dashboard also keeps a manual repair action available for a ready environment. LLM-backed memory construction needs a provider key configured in Settings, while local FastEmbed embeddings work without one.

Python, the virtual environment, Cognee data, config, downloads, and logs are stored under the Tauri app-data directory. The default listeners are:

- Cognee API: `http://127.0.0.1:12315`
- New stateless MCP endpoint: `http://127.0.0.1:12316/mcp-new`
- Legacy-compatible MCP endpoint: `http://127.0.0.1:12316/mcp`

Start the API first; MCP becomes available after the API passes its health check and the client provisions a protected API key. Both listeners bind to loopback. A port conflict is shown without terminating the process that already owns that port. The dashboard and MCP page show the current endpoint and tool list.

Closing the main window hides Cognee Desktop to the Windows notification area, and the API and MCP services keep running. Use **打开 Cognee Desktop** in the tray menu to restore the window. Use **退出** in the tray menu to stop both services and exit the application.

Ingestion may need an LLM provider. Local embedding support is included by Cognee's locked dependency set. Enter model credentials in Settings; secret values are write-only in the UI and saved in Windows Credential Manager. The generated local account password, JWT signing secret, and MCP API key use the same protected store. No plaintext environment file is created.

## Memory and files

The Datasets page lists datasets and their source records using authenticated Cognee API routes. It can upload files, trigger graph-memory construction with progress polling, safely preview text formats up to 1 MiB, save a selected source through the Windows file dialog, and delete a selected source or dataset after a target-specific confirmation. Uploads and downloads are limited to 200 MiB. Graph requests use `/api/v1/visualize/json` with `full=false` and a maximum of 1,000 nodes. Session DTOs expose only status, timestamps, and bounded summary text; internal traces are not sent to Vue.

The app does not promise cancellation of Cognee ingestion because the current general add route does not offer a corresponding cancellation API. Runtime setup can be cancelled; partial runtime files are preserved for retry.

## Upgrade and uninstall

Application upgrades retain app data and Windows Credential Manager entries under the stable application identifier `com.cognee.local.desktop`. Uninstall removes the app binaries; it does not intentionally remove memory data, runtime data, logs, or protected credentials. Back up the app-data directory before manually clearing it. Provider settings are applied after service restart, which can interrupt an active pipeline.

## Packaging and signing

`scripts/fetch-uv.ps1` pins uv 0.11.19 and verifies both the release archive checksum and executable checksum. `scripts/package-runtime.ps1` copies the MCP project and source at the repository's locked version. `src-tauri/tauri.conf.json` packages the verified uv executable plus the runtime manifest, lock, and source.

Local builds are unsigned. The optional `scripts/sign-installer.ps1` signs an installer only when `COGNEE_SIGNING_CERT_BASE64` and `COGNEE_SIGNING_CERT_PASSWORD` are provided. CI secrets are never written into the repository.

See [LICENSES.md](LICENSES.md) for the bundled bootstrap and application notices.
