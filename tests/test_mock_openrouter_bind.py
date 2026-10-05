import importlib.util
import socket
import threading
from pathlib import Path

SCRIPT = Path(__file__).with_name("mock_openrouter.py")


def _load_mock():
    spec = importlib.util.spec_from_file_location("mock_openrouter", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _has_ipv6_loopback():
    try:
        with socket.socket(socket.AF_INET6) as s:
            s.bind(("::1", 0))
        return True
    except OSError:
        return False


def test_mock_accepts_ipv4_and_ipv6_loopback():
    server = _load_mock().make_server(0)
    port = server.server_address[1]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        hosts = ["127.0.0.1"] + (["::1"] if _has_ipv6_loopback() else [])
        for host in hosts:
            socket.create_connection((host, port), timeout=5).close()
    finally:
        server.shutdown()
        server.server_close()
