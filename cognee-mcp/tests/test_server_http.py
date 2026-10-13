from src import server
from starlette.routing import Mount
from starlette.testclient import TestClient


def test_composed_http_app_mounts_one_mcp_app_at_both_paths():
    app = server._build_http_server_app(host="127.0.0.1", port=12316, log_level="info")

    mounts = {route.path: route.app for route in app.routes if isinstance(route, Mount)}

    assert set(mounts) == {"/mcp-new", "/mcp"}
    assert mounts["/mcp-new"] is mounts["/mcp"]
    assert any(getattr(route, "path", None) == "/health" for route in app.routes)


def test_composed_http_app_health_has_cors_and_rejects_foreign_host():
    app = server._build_http_server_app(host="127.0.0.1", port=12316, log_level="info")

    with TestClient(app, base_url="http://127.0.0.1:12316") as client:
        response = client.get("/health", headers={"Origin": "http://localhost:3000"})
        assert response.status_code == 200
        assert response.headers["access-control-allow-origin"] == "http://localhost:3000"

        rejected = client.get("/health", headers={"Host": "attacker.example"})
        assert rejected.status_code == 421


def test_composed_http_app_rejects_foreign_origin_on_both_mcp_mounts():
    app = server._build_http_server_app(host="0.0.0.0", port=12316, log_level="info")

    with TestClient(app, base_url="http://127.0.0.1:12316") as client:
        for path in ("/mcp-new", "/mcp"):
            response = client.post(
                path,
                headers={
                    "Origin": "http://attacker.example",
                    "Content-Type": "application/json",
                    "Accept": "application/json, text/event-stream",
                },
                json={"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}},
            )
            assert response.status_code == 403
