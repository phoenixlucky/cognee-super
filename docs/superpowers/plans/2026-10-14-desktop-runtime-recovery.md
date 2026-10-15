# Cognee Desktop Runtime Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect an incomplete keyless embedding runtime and repair it from the bundled lockfile before API startup.

**Architecture:** Rust checks the runtime's FastEmbed import and reuses `prepare_runtime`'s `uv sync --locked` path for repair. The dashboard exposes manual repair and links users to existing Settings when the default LLM key is missing.

**Tech Stack:** Rust, Tauri 2, Vue 3, TypeScript, Pinia, Vitest, uv.

**Spec:** `docs/superpowers/specs/2026-10-14-desktop-runtime-recovery-tray-design.md`

## Global Constraints

- First release supports Windows x64 only.
- Prepare the locked Python runtime in app data; never install into the user's system Python.
- Use the bundled runtime project and lockfile for dependency repair.
- Preserve application memory data and provider credentials during runtime repair.
- Missing LLM credentials do not disable keyless FastEmbed operation.
- The Python runtime and package wheels remain first-run downloads; this plan does not create an offline installer.

## Review Focus

- Existing `python.exe` with missing FastEmbed must not report ready; test readiness using a temporary runtime directory.
- A broken FastEmbed import must trigger locked sync before `start_api` spawns the service; test the repair decision independently from launching Cognee.
- Failed sync or failed post-sync import must return an actionable error and must not leave API state starting.
- Manual repair must reuse the existing progress and busy state instead of duplicating bootstrap code.
- Missing default LLM credentials must show a Settings link without blocking startup; test the no-secret state and the configured-secret state.

---

### Task 1: Detect and repair incomplete Python runtimes

**Files:**
- Modify: `cognee-desktop/src-tauri/src/commands.rs`
- Test: `cognee-desktop/src-tauri/src/commands.rs` unit tests

**Interfaces:**
- Produces: `fn runtime_has_fastembed(runtime_dir: &Path) -> bool`, checking `.venv/Scripts/python.exe` and `.venv/Lib/site-packages/fastembed/__init__.py`.
- Produces: `async fn fastembed_importable(python: &Path) -> bool`, running the venv interpreter with `-c "import fastembed"`.
- Consumes: existing `prepare_runtime(app, state)` as the sole locked sync implementation.

- [ ] **Step 1: Write the failing readiness test**

Add `runtime_is_not_ready_when_fastembed_is_missing` to create a temporary `.venv/Scripts/python.exe` and assert `runtime_has_fastembed` returns false without the package file.

- [ ] **Step 2: Run the test and confirm the expected failure**

Run from `cognee-desktop`: `cargo test --manifest-path src-tauri/Cargo.toml runtime_is_not_ready_when_fastembed_is_missing --lib`.

Expected: FAIL because the runtime-readiness helper is absent.

- [ ] **Step 3: Implement runtime readiness and startup repair**

Use `runtime_has_fastembed` in `get_app_status`. In `start_api`, run `fastembed_importable`; if false, call `prepare_runtime`, recompute runtime paths, and verify importability again before spawning the API process. Return a repair-specific error if sync succeeds but import still fails. Verify FastEmbed after a successful `prepare_runtime` sync so the dashboard cannot report a false success.

- [ ] **Step 4: Add the ready-runtime test and run focused Rust coverage**

Add `runtime_is_ready_when_python_and_fastembed_are_present`, then run the same `cargo test` filter and confirm both tests pass.

- [ ] **Step 5: Commit the Rust change**

```bash
git add cognee-desktop/src-tauri/src/commands.rs
git commit -m "fix(desktop): repair missing fastembed runtime"
```

### Task 2: Expose runtime repair and LLM-key guidance

**Files:**
- Modify: `cognee-desktop/src/views/DashboardView.vue`
- Test: `cognee-desktop/src/views/DashboardView.test.ts`
- Modify: `cognee-desktop/README.md`

**Interfaces:**
- Consumes: existing `app.prepareRuntime()`, `app.busy`, `app.runtimeStatus`, `app.secrets.llm`, and `app.config.llmProvider`.
- Produces: a dashboard repair action that stays available when runtime status is `ready`; an optional, non-blocking Settings link when the default OpenAI provider has no stored LLM key.

- [ ] **Step 1: Write failing dashboard behavior tests**

Test that a ready runtime renders a repair action, that activating it calls the existing `prepareRuntime` action, and that the default provider with no stored LLM key shows a Settings link while a stored key hides the hint.

- [ ] **Step 2: Run the focused Vitest file and confirm failure**

Run from `cognee-desktop`: `npm run test:unit -- src/views/DashboardView.test.ts`.

Expected: FAIL because the repair action and key guidance are absent.

- [ ] **Step 3: Implement the dashboard actions and guidance**

Keep the repair action inside the existing runtime card and route it to `app.prepareRuntime()`. Add concise guidance linking to `/settings`; do not block API startup or require credentials for local embeddings.

- [ ] **Step 4: Run focused frontend tests and typecheck**

Run `npm run test:unit -- src/views/DashboardView.test.ts` and `npm run typecheck` from `cognee-desktop`; expect both to pass.

- [ ] **Step 5: Document the runtime repair behavior and commit**

Document that setup/repair downloads packages when absent and requires network access. Commit the dashboard, tests, and README together.

```bash
git add cognee-desktop/src/views/DashboardView.vue cognee-desktop/src/views/DashboardView.test.ts cognee-desktop/README.md
git commit -m "feat(desktop): expose runtime repair guidance"
```

### Task 3: Verify the runtime-recovery slice

- [ ] Run `npm run build` from `cognee-desktop`.
- [ ] Run `cargo check --manifest-path src-tauri/Cargo.toml` from `cognee-desktop`.
- [ ] Review `git diff --check` and confirm no user memory data or credentials paths are modified by repair.
