# Cognee MCP FastMCP 4 Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a modern stateless Streamable HTTP MCP endpoint at `127.0.0.1:12316/mcp-new` while retaining the compatible `/mcp`, stdio, and SSE transports.

**Architecture:** Upgrade the MCP package to FastMCP 4 and its compatible MCP SDK. Keep one tool server and one HTTP app/lifespan, mount the same Streamable HTTP handler at `/mcp-new` and `/mcp`, and let FastMCP 4 negotiate the stateless 2026 protocol for modern clients and session behavior for older protocol revisions. Preserve the current Host/Origin guard, health route, and CORS behavior.

**Tech Stack:** Python 3.10–3.13 for the MCP package; FastMCP 4; compatible `mcp` SDK; Starlette; Uvicorn; pytest.

**Spec:** `docs/superpowers/specs/2026-10-13-cognee-local-mcp-desktop-design.md`

## Global Constraints

- First release target is Windows x64.
- `/mcp-new` is stateless Streamable HTTP at default `127.0.0.1:12316`.
- Keep `/mcp`, stdio, and SSE compatibility.
- Support MCP protocol version `2026-07-28` where supported by the selected FastMCP 4 release and continue negotiating older client versions.
- Do not add MCP Tasks extensions.
- Preserve tool names and input/output semantics.
- Bind local services to loopback by default and keep Host/Origin protection enabled.

## Review Focus

- Modern `server/discover` and tool requests carry protocol version and client capabilities in `params._meta`; they must work on `/mcp-new` without an initialize handshake or `Mcp-Session-Id`.
- Legacy initialize/client request must continue through `/mcp` with the protocol version it supports.
- A GET requesting the optional Streamable HTTP event stream must follow FastMCP's supported behavior; legacy SSE remains on `/sse` with its existing initialization path.
- Requests with an invalid Host or Origin must continue to be rejected on both MCP routes.
- The same registered tool schema and behavior must be exposed through both routes without duplicate tool registration or divergent API credentials.

---

### Task 1: Upgrade MCP protocol dependencies

**Files:**
- Modify: `cognee-mcp/pyproject.toml`
- Modify: `cognee-mcp/uv.lock`
- Test: `cognee-mcp/tests/test_entrypoint_args.py`

**Interfaces:**
- Consumes: Existing `src.server.main()` CLI and `src.main_mcp` console entry point.
- Produces: A locked FastMCP 4 / MCP SDK dependency set that installs with the existing Python range.

- [x] Resolve the latest stable FastMCP 4 release and compatible MCP SDK version against the package's Python `<3.14` constraint; record exact lockfile versions.
- [x] Update `fastmcp` and `mcp` constraints in `cognee-mcp/pyproject.toml`, preserving all existing tool-search and Cognee dependency requirements.
- [x] Refresh only the MCP lockfile and confirm the Cognee package and extras remain within declared constraints.
- [x] Run `uv lock --check` and import/version checks from `cognee-mcp`; require successful resolution and FastMCP major version 4.

### Task 2: Separate HTTP route construction from transport startup

**Files:**
- Modify: `cognee-mcp/src/server.py`
- Create: `cognee-mcp/tests/test_server_http.py`

**Interfaces:**
- Consumes: `mcp`, `_transport_security_kwargs(host)`, `_get_cors_origins()`.
- Produces: `_build_http_app(transport: str, host: str, path: str | None = None)` and `_build_http_server_app(host: str, port: int, log_level: str)`.

- [x] Write focused tests for app construction, CORS, and Host/Origin guard on loopback and non-loopback binds.
- [x] Run `uv run pytest tests/test_server_http.py -q` and confirm the new tests fail before implementation.
- [x] Update the HTTP construction to use the FastMCP 4 Streamable HTTP app at internal path `/`, preserving the existing SSE middleware path.
- [x] Compose one Uvicorn ASGI app with one FastMCP lifespan, root `GET /health`, and the same Streamable HTTP app mounted at `/mcp-new` and `/mcp`. Modern per-request metadata uses the sessionless protocol; older clients retain initialize/session behavior.
- [x] Apply CORS and Host/Origin policy to both mounted routes and root health; preserve the dedicated `/sse` app and stdio runner.
- [x] Run the focused tests and confirm the route table and middleware assertions pass.

### Task 3: Add the `mcp-new` CLI mode and local defaults

**Files:**
- Modify: `cognee-mcp/src/server.py`
- Modify: `cognee-mcp/entrypoint.sh`
- Modify: `cognee-mcp/Dockerfile`
- Test: `cognee-mcp/tests/test_entrypoint_args.py`

**Interfaces:**
- Consumes: Existing `--transport`, `--host`, `--port`, `--path` arguments.
- Produces: `--transport http-new` mode selecting `/mcp-new`; existing `http` selects `/mcp`; HTTP port default is `12316` for `http-new` and legacy defaults remain unchanged.

- [x] Extend argument tests to assert default host, port, and path for `http-new`, plus unchanged defaults for `http`, `sse`, and `stdio`.
- [x] Implement `http-new` dispatch to the composed app. Allow `--host`, `--port`, and `--path` overrides; default the endpoint path to `/mcp-new` and the port to `12316`.
- [x] Keep existing `http` dispatch on `/mcp`; preserve stdio/SSE defaults.
- [x] Update container environment defaults and README examples so Docker entrypoint behavior agrees with CLI behavior.
- [x] Run the focused entrypoint tests and inspect `--help` output for all transport choices.

### Task 4: Verify MCP wire compatibility and update documentation

**Files:**
- Create: `cognee-mcp/tests/test_server_protocol_compatibility.py`
- Modify: `cognee-mcp/README.md`
- Modify: `cognee-mcp/src/server.py` (only if FastMCP 4 integration requires an adapter)

**Interfaces:**
- Consumes: `http-new` and `http` endpoints from Task 3.
- Produces: Documented local URL, protocol negotiation behavior, old-client fallback, and explicit non-support for Tasks.

- [x] Add an in-process protocol test that discovers, calls `tools/list`, and calls one memory tool using per-request `2026-07-28` metadata on `/mcp-new`, with no initialize handshake or session header.
- [x] Add a legacy-client protocol test using protocol version `2025-11-25` on `/mcp` and assert the initialize/session flow can list and call the same tool.
- [x] Use legacy protocol version `2025-11-25` for the compatibility test.
- [x] Assert `/sse` remains separately available and stdio still selects `run_stdio_async`; Streamable HTTP GET behavior matches the selected FastMCP release's contract.
- [x] Add a negative test for rejected Host or Origin on the new route.
- [x] Run the focused protocol tests and resolve compatibility failures without changing tool semantics.
- [x] Document the recommended endpoint `http://127.0.0.1:12316/mcp-new`, old endpoint, stdio/SSE examples, port override, protocol support, and API-mode environment contract in `cognee-mcp/README.md`.
- [x] Run the complete MCP test suite and `uv run ruff check src tests` from `cognee-mcp`.
