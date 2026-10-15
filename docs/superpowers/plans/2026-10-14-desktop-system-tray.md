# Cognee Desktop System Tray Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let Cognee Desktop hide to the Windows notification area while its supervised API and MCP services continue running.

**Architecture:** Enable Tauri's native tray support and route tray menu actions through a small testable action enum. Window close hides the window; an explicit tray Quit action stops supervised services and exits.

**Tech Stack:** Rust, Tauri 2 tray icon, Windows x64.

**Spec:** `docs/superpowers/specs/2026-10-14-desktop-runtime-recovery-tray-design.md`

## Global Constraints

- First release supports Windows x64 only.
- Local API and MCP services remain supervised child processes bound to loopback.
- Closing the main window must not stop API or MCP.
- Explicit Quit must stop API and MCP before the desktop process exits.
- Do not expand Tauri capabilities to arbitrary shell or filesystem access.

## Review Focus

- Repeated close events must not start duplicate shutdowns; test tray action selection and keep shutdown guarded by the existing atomic flag.
- Restoring from tray must show and focus the existing main window rather than create a second window; cover by a Windows manual smoke check.
- Tray Quit must stop both children before app exit; cover the handler wiring in code review and a Windows manual smoke check.
- The native tray must use the bundled app icon and compile through the NSIS build configuration.
- A close-to-tray session must leave memory and credentials untouched; manually confirm data survives hide/restore/quit.

---

### Task 1: Add tray menu action mapping

**Files:**
- Modify: `cognee-desktop/src-tauri/src/lib.rs`
- Modify: `cognee-desktop/src-tauri/Cargo.toml`
- Modify: `cognee-desktop/src-tauri/Cargo.lock`

**Interfaces:**
- Produces: `TrayAction::{Show, Quit, Ignore}` and `fn tray_action(id: &str) -> TrayAction` for stable menu IDs `show-window` and `quit`.
- Consumes: Tauri 2 tray, menu, and default-window-icon APIs with the `tray-icon` feature enabled.

- [ ] **Step 1: Write failing tray action tests**

Add tests asserting `show-window` maps to `Show`, `quit` maps to `Quit`, and unknown IDs map to `Ignore`.

- [ ] **Step 2: Run the focused Rust test and confirm failure**

Run from `cognee-desktop`: `cargo test --manifest-path src-tauri/Cargo.toml tray_action --lib`.

Expected: FAIL because the action enum and mapper are absent.

- [ ] **Step 3: Add the mapper and native tray menu**

Enable Tauri's `tray-icon` feature. Build one tray icon using the bundled default app icon and menu entries `打开 Cognee Desktop` and `退出`. The Show handler shows and focuses the existing `main` window.

- [ ] **Step 4: Run focused Rust tests and compile**

Run `cargo test --manifest-path src-tauri/Cargo.toml tray_action --lib` and `cargo check --manifest-path src-tauri/Cargo.toml`; expect both to pass.

- [ ] **Step 5: Commit tray creation and menu mapping**

```bash
git add cognee-desktop/src-tauri/src/lib.rs cognee-desktop/src-tauri/Cargo.toml cognee-desktop/src-tauri/Cargo.lock
git commit -m "feat(desktop): add native tray menu"
```

### Task 2: Keep services alive on window close and exit cleanly from tray

**Files:**
- Modify: `cognee-desktop/src-tauri/src/lib.rs`
- Modify: `cognee-desktop/README.md`

**Interfaces:**
- Consumes: Task 1's `tray_action` mapping and `commands::stop_all`.
- Produces: close-to-tray behavior; tray Quit sets the existing shutdown guard, awaits `stop_all`, then calls `app.exit(0)`.

- [ ] **Step 1: Implement close and Quit lifecycle handlers**

Change `CloseRequested` to prevent close and hide `main` without calling `stop_all`. In the tray Quit handler, run `stop_all` once and exit after cleanup. Ignore subsequent close or Quit events after shutdown begins.

- [ ] **Step 2: Document close-to-tray and Quit behavior**

State that closing the window leaves services running and that Quit in the tray menu stops API/MCP.

- [ ] **Step 3: Verify and commit lifecycle behavior**

Run `cargo check --manifest-path src-tauri/Cargo.toml`, then build the Windows NSIS installer with `npm run tauri:build` when the Windows Rust toolchain is available. Commit the lifecycle and documentation changes.

```bash
git add cognee-desktop/src-tauri/src/lib.rs cognee-desktop/README.md
git commit -m "feat(desktop): keep services running in tray"
```

### Task 3: Manually verify tray behavior on Windows

- [ ] Start API and MCP, close the main window, and confirm both services remain reachable.
- [ ] Choose Open in the tray menu and confirm the same dashboard window is restored and focused.
- [ ] Choose Quit and confirm the window exits and both supervised processes stop.
- [ ] Restart the app and confirm datasets and credentials remain available.
