# Cognee Desktop Runtime Recovery and Tray — Design

**Date:** 2026-10-14  
**Status:** Approved for planning  
**Target:** Windows x64 Cognee Desktop

## Goal

Make runtime readiness reflect the dependencies Cognee actually needs, repair an incomplete local runtime through the bundled locked environment, and let the desktop app keep supervised services running after the main window closes.

## User-visible behavior

- A runtime with Python present but without an importable `fastembed` package is reported as not ready.
- Starting the API checks that `fastembed` can be imported. If it is missing, the desktop runs the bundled `uv sync --locked` flow before starting Cognee, then checks again and reports a useful failure if repair did not work.
- The dashboard keeps a manual runtime repair action available when the runtime is ready. Runtime preparation and repair use the same locked sync path and progress events.
- If the default LLM provider has no stored API key, the dashboard points to Settings and explains that LLM-backed operations need a key. Missing credentials do not disable the supported keyless local embedding path.
- Closing the main window hides it to the notification area. Cognee API and MCP child processes continue running.
- The tray menu can reopen the main window or explicitly quit. Quit stops the supervised MCP and API processes before exiting the desktop process.

## Design

### Runtime verification and repair

Treat Python and the keyless default embedding runtime as one readiness unit. Check the installed virtual environment for the FastEmbed import and use the bundled runtime project and lockfile as the only repair source. The ordinary status refresh must stay cheap; the startup preflight performs the definitive import check and synchronizes only when it fails. Status reporting must also detect an absent FastEmbed package so the dashboard does not claim a known-incomplete environment is ready.

The existing `prepare_runtime` command remains the single setup path. It already executes `uv sync --locked`; reuse it for automatic repair and the dashboard repair action. Preserve application data and credentials while repairing the virtual environment. If synchronization succeeds but the import still fails, return a repair-specific error with the runtime log location rather than waiting for repeated API health failures.

Settings already store provider and model options plus write-only API credentials. Add a dashboard hint for the default LLM configuration when its key is absent, linking to Settings. Do not invent credentials or require cloud credentials for local FastEmbed operation.

### Tray lifecycle

Use Tauri's native tray icon and menu. Intercept the main window's close request and hide the window without stopping child processes. A tray menu item restores and focuses the existing window. A separate Quit item invokes the existing supervised-process cleanup and then exits. Keep application shutdown idempotent so an OS exit or repeated close event cannot race cleanup.

### Scope

Modify the Tauri runtime commands and state reporting, the dashboard and app store, Tauri tray configuration/dependency features, desktop documentation, and focused Rust/Vitest coverage. Keep Cognee Python core, MCP protocol behavior, and memory data formats unchanged.

## Acceptance criteria

1. A virtual environment with `python.exe` but no importable `fastembed` is not presented as ready.
2. Starting the API automatically runs the locked runtime sync when the FastEmbed preflight fails, and only proceeds after the import succeeds.
3. The dashboard offers a manual repair action that reuses runtime preparation and reports completion/failure.
4. Missing default LLM credentials lead to a clear Settings link without blocking API startup or local FastEmbed use.
5. Closing the main window hides it, and the API/MCP processes remain alive.
6. The tray menu restores the window and provides an explicit Quit action that stops supervised child processes before exiting.
7. Automated coverage exercises runtime readiness/repair decisions and dashboard guidance; frontend typecheck/build and Rust compilation succeed.

## Verification

- Add a Rust unit test proving a present Python executable with a missing FastEmbed import does not qualify as runtime-ready.
- Add focused Rust coverage for dependency preflight/repair selection where practical without launching Cognee or accessing external services.
- Add Vitest coverage for the dashboard's manual repair and missing-key guidance behavior.
- Run the targeted desktop unit tests, frontend typecheck/build, and Rust checks. Manually smoke-check close-to-tray, restore, and Quit behavior on Windows because native tray interactions are platform-specific.

## Risks and mitigations

- Runtime sync needs package-index access when a dependency is absent. Keep existing progress/error reporting and do not delete the Python installation, Cognee data, or credentials during repair.
- Tray Quit intentionally stops the local API and MCP. Make that action explicit in the menu label; closing the window alone must never stop services.
- Package import checks can expose a broken or partial installation. Repair uses the checked-in lockfile so the result remains reproducible.
