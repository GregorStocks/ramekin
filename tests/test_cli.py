import os
import subprocess


def test_cli_ping():
    """Test CLI ping command against the server."""
    server_url = os.environ.get("API_BASE_URL", "http://server:3000")
    cli_path = os.environ.get("CLI_PATH", "/app/cli/target/debug/ramekin-cli")

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
