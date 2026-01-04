import os
import subprocess
from pathlib import Path


def get_cli_path() -> str:
    """Get the path to the CLI binary."""
    # Check environment variable first
    if cli_path := os.environ.get("CLI_PATH"):
        return cli_path
    # Docker path
    docker_path = "/app/cli/target/debug/ramekin-cli"
    if Path(docker_path).exists():
        return docker_path
    # Local release build
    project_root = Path(__file__).parent.parent
    local_release = project_root / "cli" / "target" / "release" / "ramekin-cli"
    if local_release.exists():
        return str(local_release)
    # Local debug build
    local_debug = project_root / "cli" / "target" / "debug" / "ramekin-cli"
    if local_debug.exists():
        return str(local_debug)
    raise FileNotFoundError("CLI binary not found. Build with: cd cli && cargo build")


def test_cli_ping():
    """Test CLI ping command against the server."""
    server_url = os.environ.get("API_BASE_URL", "http://localhost:3000")
    cli_path = get_cli_path()

    result = subprocess.run(
        [
            cli_path,
            "ping",
            "--server",
            server_url,
        ],
        capture_output=True,
        text=True,
    )

    assert result.returncode == 0, f"CLI failed: {result.stderr}"
    assert "unauthed-ping" in result.stdout
