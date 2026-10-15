# Cognee Local MCP Desktop — Design Specification

**Date:** 2026-10-13
**Status:** Draft for user review
**Target:** Windows x64 first release

## 1. Goal

Build a distributable Windows desktop client for running Cognee locally and managing its MCP service and memory data. The application uses Tauri 2.11+, stable Rust, Vue 3.5.x, TypeScript, Vite 8, and Pinia. Vue Router is optional and should only be added if the selected screen structure needs it. Use a consistent component library and Tailwind CSS where they simplify the interface.

The desktop client must:

- Start and stop the local Cognee API and MCP services, show readiness, and surface useful logs and errors.
- Offer a modern stateless Streamable HTTP MCP endpoint at `http://127.0.0.1:12316/mcp-new`.
- Preserve the legacy `/mcp` route and the existing stdio and SSE transports.
- Manage datasets, imported source files, graph memory, and session records through Cognee's API.
- Prepare a compatible Cognee runtime automatically on first launch and produce an installable Windows x64 package.

## 2. User workflows

1. Install and launch the desktop application. On first launch, the app checks its bundled bootstrap components, prepares the Cognee runtime, and reports progress or actionable errors.
2. Start or stop the API and MCP services from a status dashboard. The user can inspect recent logs, copy the MCP URL, and see which transports are available.
3. Browse datasets and their imported data records. Inspect metadata, preview or download available source content, and delete a selected record or dataset after an explicit in-app confirmation.
4. Browse a dataset's graph visually and inspect node/edge details. Provide filtering or paging/limits so large graphs do not freeze the UI.
5. Browse session records with status and time filters and inspect a selected session's details where the Cognee API provides them.
6. Configure Cognee's model/provider settings and credentials without exposing secret values in the UI, logs, or persisted plaintext configuration.

## 3. Scope and boundaries

### In scope

- One local, single-user Windows installation; bind local services to loopback by default.
- Runtime bootstrap and lifecycle supervision for Cognee API and MCP processes.
- MCP HTTP endpoints, retaining stdio and SSE as existing options.
- Dataset, data-record, raw-file, graph, and session browsing/management supported by current Cognee APIs.
- Provider configuration sufficient to make the local Cognee instance usable.
- Windows x64 installer packaging and application-data directory management.

### Out of scope for the first release

- macOS/Linux packages, remote multi-user administration, cloud-hosted Cognee, and remote network binding.
- A general-purpose graph editor or direct edits to extracted graph nodes. Source changes should be made by replacing/re-ingesting source data using supported Cognee operations.
- MCP Tasks protocol extensions.
- Reimplementing Cognee's memory/database logic in Rust or coupling the UI directly to Cognee's internal databases.
- An offline installer unless dependency and model distribution requirements are separately specified.

## 4. Architecture

```text
┌────────────────────────────── Tauri desktop app ───────────────────────────────┐
│ Vue 3 + TypeScript + Pinia                                                     │
│  Dashboard · MCP · Datasets/files · Graph · Sessions · Settings · Logs         │
│             │ typed Tauri commands/events                                     │
│ Rust service layer                                                             │
│  runtime bootstrap · process lifecycle · config/secrets · filesystem · logs    │
└─────────────┬───────────────────────────────────────────────────┬──────────────┘
              │ loopback HTTP                                     │ child process
              ▼                                                   ▼
         Cognee API  ◄─────────────────────────────── Cognee MCP server
              │                                            /mcp-new (stateless)
              │                                            /mcp (legacy)
              ▼                                            stdio / SSE
       Cognee local stores
```

- Rust is the authority for process creation/termination, runtime installation, filesystem access, secret handling, and background work. The frontend invokes a narrow, typed Tauri command surface and receives progress/state events.
- Cognee's API remains the data-management boundary. The frontend must not read/write Cognee's SQLite, graph, or vector database files directly; backends are configurable and authorization logic belongs to the API.
- MCP runs in API mode against the supervised local Cognee API so MCP and the UI access the same memory instance. The MCP process receives API credentials through a protected process environment or a private local configuration channel, never command-line arguments.
- The default listener is `127.0.0.1:12316`. The desktop client presents the exact endpoint URL and configured transport paths. Port/path overrides, if later supported, must be reflected in both status and copy actions.
- Keep the existing `cognee-frontend/` web app independent. The desktop app is a separate `cognee-desktop/` project.

## 5. Runtime bootstrap and distribution

- The installer contains the Tauri application and a small, pinned bootstrap executable/runtime component. On first launch, Rust prepares an app-owned Python environment and installs the Cognee API/MCP dependency set from lockfiles into an application-data directory.
- Use a reproducible locked dependency resolution and a supported Python version. The current MCP package declares Python `>=3.10,<3.14`; select and pin one supported minor version for the first release after validating Cognee core and all Windows wheels.
- Bootstrap operations run asynchronously with progress events, cancellation before service start, clear retry behavior, and logs with secrets redacted. Do not silently install dependencies into a system Python.
- Keep user data, runtime files, and logs in separate app-owned directories. Uninstall behavior for user data must be documented and must not silently delete memory data.
- Provider credentials and other secrets are stored using Windows-protected OS storage (Credential Manager or DPAPI-backed storage); store only secret references in ordinary config. Provider configuration must explain that an LLM/embedding provider may be required for ingestion and recall.
- The default installation requires network access on first run to prepare packages and may download additional Cognee/model assets later. The installer size, total runtime footprint, offline behavior, upgrade/migration strategy, and licensing notices require implementation validation before release.

## 6. Service lifecycle and local security

- Rust owns child process handles and starts API before MCP. MCP starts only after an API health/readiness check succeeds. A failure starting either service produces a visible failed state and a diagnostic log location.
- Use Windows Job Objects (or an equivalent process-tree containment mechanism) so stopping the app cannot leave orphaned Python/uvicorn child processes. Shutdown is graceful first, then bounded forced termination.
- Capture stdout/stderr into rotating, app-owned logs. Redact tokens, passwords, API keys, authorization headers, and sensitive provider values before persistence or UI display.
- Bind API and MCP to loopback only. Apply Host/Origin checks and restrictive CORS for browser-facing routes. These controls do not replace API authentication.
- Generate per-install local API credentials during setup, configure the API with authentication enabled, and keep credentials in OS-protected storage. The desktop app authenticates to the API; the MCP API-mode process receives only the credential it needs. Do not disable Cognee authentication as a shortcut for local use.
- Expose only required Tauri commands, validate all paths/IDs in Rust, and avoid arbitrary shell execution from the frontend. File previews/downloads must resolve through authorized Cognee API routes and safe app-managed temporary files.
- Destructive actions (delete data record/dataset, reset local store, remove app data) require a clear confirmation naming the target. Never call bulk-delete endpoints from an ambiguous UI action.

## 7. MCP protocol behavior

- Upgrade `cognee-mcp` to FastMCP 4 and its compatible MCP SDK versions using a reviewed lockfile update. The current project pins FastMCP 3.x (`fastmcp>=3.4,<4`), so this is a protocol/dependency migration, not just a route alias.
- Add stateless Streamable HTTP on `/mcp-new`, with default bind address `127.0.0.1` and port `12316` for the desktop-managed local service. Follow the modern endpoint behavior demonstrated by `E:\home\mcp-chrome-super`.
- Keep `/mcp` as the compatibility route for existing clients. Keep stdio and SSE entry points. Preserve existing tool names and input/output semantics unless FastMCP 4 requires a documented adapter.
- Support MCP protocol version `2026-07-28` where supported by the selected FastMCP 4 release; verify negotiation and backward compatibility with older clients. Do not claim support for Tasks.
- Health/readiness reporting remains separate from MCP protocol requests.

## 8. Data and screens

### Dashboard

Show API and MCP process status, runtime setup state, endpoint address, current dataset count, recent service activity, and actions to start/stop/retry or open logs.

### MCP

Show endpoint URLs (`/mcp-new` and `/mcp`), transport status, configured port, available tools, and connection instructions. Do not imply that listing tools is equivalent to an authorization boundary.

### Datasets and source files

Use Cognee dataset and data-record endpoints for listing, pagination, metadata, raw content retrieval/download, and deletion. Show source filename, extension/type, size, and dataset. Preview only formats the client can render safely; otherwise offer download. Upload/ingestion can be included if supported by existing API endpoints and can display pipeline progress; otherwise expose a clear “add source” flow through the supported Cognee API.

### Graph memory

Use the dataset graph endpoint or supported visualization endpoint to show nodes and edges, with search/filter and bounded rendering. Selecting a node or edge opens its available properties and provenance. Large graph loading must be paginated, capped, or otherwise bounded.

### Session records

Use `/api/v1/sessions` list/filter and detail endpoints for session metadata/status. The first release should not expose internal trace payloads or credentials unless the API explicitly returns safe fields and the UI labels them clearly.

### Settings

Expose provider/model settings, data directory location, port, and startup behavior. Secret entry uses secure fields; read-back displays presence/last-updated state, never the secret itself.

## 9. Error handling and recovery

- Represent runtime preparation, API startup, MCP startup, running, stopping, stopped, and failed as explicit states.
- Preserve the last useful error and a log path after failure. Offer retry for transient download/startup errors.
- If the API is healthy but MCP fails, leave the API state visible and allow MCP-only retry. If API stops unexpectedly, stop/restart dependent MCP in a controlled way and preserve logs.
- Detect port conflicts before start and report the conflicting listener/address. Do not kill unrelated processes to free a port.
- On app upgrade, retain user memory data and provider configuration. Apply explicit migrations with backup/rollback guidance before modifying persistent Cognee stores.

## 10. Acceptance criteria

1. A clean Windows x64 machine can install the application, complete first-run runtime setup, and start the local Cognee API and MCP services without requiring a preinstalled Python environment.
2. The dashboard reports accurate API/MCP state and exposes recent redacted logs.
3. A modern MCP client can call `server/discover`, list tools, and call a tool through `/mcp-new` using the `2026-07-28` per-request metadata without an initialize handshake or session header. An older supported client can use the initialize/session flow through `/mcp`; stdio and SSE still start.
4. The UI can list datasets, browse source records, retrieve/download a source file, delete a selected record, and browse graph memory through supported API endpoints.
5. The UI can list and filter session records and open supported session details.
6. Service listeners bind to loopback by default; protected API credentials and provider secrets do not appear in plaintext config, process arguments, logs, or UI read-back.
7. Stopping the service/app leaves no supervised child process running. App upgrade preserves user data.
8. Windows x64 installer can be built from documented repository commands with locked runtime dependencies.

## 11. Verification plan (for implementation phase)

Do not run these during this specification-only phase. During implementation, add focused automated coverage for MCP transport negotiation/route compatibility, lifecycle state transitions/process-tree cleanup, secret redaction, and API-backed dataset/file/graph/session operations. Perform a Windows x64 packaging smoke check and a manual end-to-end run with one supported provider. Exercise an older MCP client against `/mcp` and a modern client against `/mcp-new`.

## 12. Decisions and open implementation checks

Approved product decisions:

- Independent Tauri/Vue desktop client, Windows x64 first.
- Runtime auto-preparation, supervised API and MCP services, local data management through Cognee API.
- Modern stateless `/mcp-new` at port `12316`, with `/mcp`, stdio, and SSE compatibility.
- Manage both original imported files and memory records.

Validate while planning/implementing:

- Exact pinned FastMCP 4 and MCP SDK versions that provide the requested protocol negotiation.
- Windows-compatible Python version, wheel availability, install size, and whether Cognee's current MCP dependency set can be reduced for the desktop runtime.
- Cognee API bootstrap/auth configuration: generate an install-local credential without weakening default authentication, and ensure MCP API mode accepts it.
- Which existing API routes cover upload/ingestion progress, raw preview, graph limits, and session details in the exact pinned Cognee release.
- Tauri plugin choices for OS-protected secrets, auto-updater policy, installer format, code signing, and user-data retention during uninstall.
- Whether first-run setup downloads packages only, or also model assets; report expected network and disk requirements to the user.
