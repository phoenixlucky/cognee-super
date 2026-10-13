import asyncio
import json
import sys
from types import SimpleNamespace
from unittest.mock import AsyncMock

from src import server
from starlette.routing import Route
from starlette.testclient import TestClient


def _request_meta(version: str) -> dict:
    return {
        "io.modelcontextprotocol/protocolVersion": version,
        "io.modelcontextprotocol/clientCapabilities": {},
        "io.modelcontextprotocol/clientInfo": {"name": "protocol-test", "version": "1.0"},
    }


def _modern_request(client, method: str, request_id: int, params: dict):
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
        "MCP-Protocol-Version": "2026-07-28",
        "Mcp-Method": method,
    }
    if method == "tools/call":
        headers["Mcp-Name"] = params["name"]
    response = client.post(
        "/mcp-new",
        headers=headers,
        json={"jsonrpc": "2.0", "id": request_id, "method": method, "params": params},
        follow_redirects=False,
    )
    assert response.status_code == 200
    assert "mcp-session-id" not in response.headers
    return response.json()


def _legacy_response(response) -> dict:
    data = next(line[6:] for line in response.text.splitlines() if line.startswith("data: "))
    return json.loads(data)


def test_agent_scope_reads_modern_client_info_metadata():
    context = SimpleNamespace(
        request_context=SimpleNamespace(
            meta={
                "io.modelcontextprotocol/clientInfo": {
                    "name": "Cursor VSCode",
                    "version": "1.0",
                }
            },
            session=SimpleNamespace(client_params=None),
        )
    )

    assert server._agent_scoped_default_dataset(context) == "cursor_vscode_memory"


def test_agent_scope_reads_legacy_initialize_client_info():
    context = SimpleNamespace(
        request_context=SimpleNamespace(
            meta=None,
            session=SimpleNamespace(
                client_params=SimpleNamespace(
                    client_info=SimpleNamespace(name="Claude Desktop")
                )
            ),
        )
    )

    assert server._agent_scoped_default_dataset(context) == "claude_desktop_memory"


def test_modern_protocol_lists_and_calls_memory_tool_without_session():
    app = server._build_http_server_app(host="127.0.0.1", port=12316, log_level="info")

    with TestClient(app, base_url="http://127.0.0.1:12316") as client:
        discovery = _modern_request(
            client,
            "server/discover",
            1,
            {"_meta": _request_meta("2026-07-28")},
        )
        assert "2026-07-28" in discovery["result"]["supportedVersions"]

        tools_result = _modern_request(
            client,
            "tools/list",
            2,
            {"_meta": _request_meta("2026-07-28")},
        )
        assert any(tool["name"] == "remember" for tool in tools_result["result"]["tools"])

        call_result = _modern_request(
            client,
            "tools/call",
            3,
            {"_meta": _request_meta("2026-07-28"), "name": "remember", "arguments": {}},
        )
        assert call_result["result"]["resultType"] == "complete"
        assert "provide `data`" in call_result["result"]["content"][0]["text"]


def test_legacy_protocol_negotiates_session_lists_and_calls_memory_tool():
    app = server._build_http_server_app(host="127.0.0.1", port=12316, log_level="info")
    base_headers = {
        "Content-Type": "application/json",
        "Accept": "application/json, text/event-stream",
    }

    with TestClient(app, base_url="http://127.0.0.1:12316") as client:
        initialize = client.post(
            "/mcp",
            headers=base_headers,
            json={
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {},
                    "clientInfo": {"name": "legacy-test", "version": "1.0"},
                },
            },
        )
        assert initialize.status_code == 200
        session_id = initialize.headers["mcp-session-id"]
        assert _legacy_response(initialize)["result"]["protocolVersion"] == "2025-11-25"

        session_headers = {
            **base_headers,
            "Mcp-Session-Id": session_id,
            "MCP-Protocol-Version": "2025-11-25",
        }
        initialized = client.post(
            "/mcp",
            headers=session_headers,
            json={"jsonrpc": "2.0", "method": "notifications/initialized"},
        )
        assert initialized.status_code == 202

        tools = client.post(
            "/mcp",
            headers={**session_headers, "Mcp-Method": "tools/list"},
            json={"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}},
        )
        assert tools.status_code == 200
        assert any(tool["name"] == "remember" for tool in _legacy_response(tools)["result"]["tools"])

        call = client.post(
            "/mcp",
            headers={
                **session_headers,
                "Mcp-Method": "tools/call",
                "Mcp-Name": "remember",
            },
            json={
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "remember", "arguments": {}},
            },
        )
        assert call.status_code == 200
        assert "provide `data`" in _legacy_response(call)["result"]["content"][0]["text"]


def test_sse_route_remains_available():
    app = server._build_http_app(transport="sse", host="127.0.0.1")

    assert any(isinstance(route, Route) and route.path == "/sse" for route in app.routes)


def test_streamable_http_get_without_legacy_session_follows_fastmcp_contract():
    app = server._build_http_server_app(host="127.0.0.1", port=12316, log_level="info")

    with TestClient(app, base_url="http://127.0.0.1:12316") as client:
        response = client.get(
            "/mcp-new", headers={"Accept": "text/event-stream"}, follow_redirects=False
        )

    assert response.status_code == 400
    assert "Missing session ID" in response.json()["error"]["message"]


def test_stdio_still_uses_fastmcp_stdio_runner(monkeypatch):
    class FakeClient:
        async def close(self):
            return None

    runner = AsyncMock()
    monkeypatch.setattr(server, "CogneeClient", lambda **kwargs: FakeClient())
    monkeypatch.setattr(server.mcp, "run_stdio_async", runner)
    monkeypatch.setattr(sys, "argv", ["server.py", "--transport", "stdio", "--no-migration"])

    asyncio.run(server.main())

    runner.assert_awaited_once_with(show_banner=False)
