#!/usr/bin/env python3
"""
Run end-to-end tests locally without Docker.

This script:
- Ensures PostgreSQL is running and sets up the test database
- Runs database migrations
- Starts the fixture server for scrape tests
- Builds and runs the Rust server
- Runs pytest via uv
- Cleans up background processes
"""

import os
import signal
import socket
import subprocess
import sys
import time
from pathlib import Path


def get_project_root() -> Path:
    """Get the project root directory."""
    return Path(__file__).parent.parent


# Configuration
TEST_DB_NAME = "ramekin_test"
TEST_DB_USER = "ramekin"
TEST_DB_PASSWORD = "ramekin"
FIXTURE_PORT = 8888
SERVER_PORT = 3000


def check_port_available(port: int) -> bool:
    """Check if a port is available."""
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        return s.connect_ex(("localhost", port)) != 0


def wait_for_port(port: int, timeout: float = 60.0) -> bool:
    """Wait for a port to become available."""
    start = time.time()
    while time.time() - start < timeout:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
            if s.connect_ex(("localhost", port)) == 0:
                return True
        time.sleep(0.1)
    return False


def ensure_postgres() -> None:
    """Ensure PostgreSQL is running."""
    result = subprocess.run(
        ["pg_isready", "-h", "localhost"],
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        print("PostgreSQL is not running. Attempting to start...")
        subprocess.run(["service", "postgresql", "start"], check=False)
        time.sleep(2)
        result = subprocess.run(
            ["pg_isready", "-h", "localhost"],
            capture_output=True,
            check=False,
        )
        if result.returncode != 0:
            print("ERROR: Could not start PostgreSQL", file=sys.stderr)
            sys.exit(1)
    print("PostgreSQL is running")


def setup_test_database() -> str:
    """Set up the test database and return the DATABASE_URL."""
    # Check if user exists, create if not
    result = subprocess.run(
        [
            "sudo",
            "-u",
            "postgres",
            "psql",
            "-tAc",
            f"SELECT 1 FROM pg_roles WHERE rolname='{TEST_DB_USER}'",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if "1" not in result.stdout:
        print(f"Creating database user '{TEST_DB_USER}'...")
        create_user_sql = (
            f"CREATE USER {TEST_DB_USER} WITH PASSWORD '{TEST_DB_PASSWORD}' CREATEDB;"
        )
        subprocess.run(
            ["sudo", "-u", "postgres", "psql", "-c", create_user_sql],
            check=False,
            capture_output=True,
        )

    # Drop and recreate test database for clean slate
    print(f"Recreating test database '{TEST_DB_NAME}'...")
    subprocess.run(
        [
            "sudo",
            "-u",
            "postgres",
            "psql",
            "-c",
            f"DROP DATABASE IF EXISTS {TEST_DB_NAME};",
        ],
        check=False,
        capture_output=True,
    )
    subprocess.run(
        [
            "sudo",
            "-u",
            "postgres",
            "psql",
            "-c",
            f"CREATE DATABASE {TEST_DB_NAME} OWNER {TEST_DB_USER};",
        ],
        check=True,
        capture_output=True,
    )

    return f"postgres://{TEST_DB_USER}:{TEST_DB_PASSWORD}@localhost:5432/{TEST_DB_NAME}"


def run_migrations(project_root: Path, database_url: str) -> None:
    """Run diesel migrations."""
    print("Running database migrations...")
    result = subprocess.run(
        ["diesel", "migration", "run"],
        cwd=project_root,
        env={**os.environ, "DATABASE_URL": database_url},
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        print(f"ERROR: Migrations failed:\n{result.stderr}", file=sys.stderr)
        sys.exit(1)
    print("Migrations complete")


def start_fixture_server(project_root: Path) -> subprocess.Popen:
    """Start the fixture server for scrape tests."""
    fixtures_dir = project_root / "tests" / "scrape_fixtures"
    print(f"Starting fixture server on port {FIXTURE_PORT}...")

    proc = subprocess.Popen(
        [sys.executable, "-m", "http.server", str(FIXTURE_PORT)],
        cwd=fixtures_dir,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    )

    if not wait_for_port(FIXTURE_PORT, timeout=5.0):
        proc.kill()
        print("ERROR: Fixture server failed to start", file=sys.stderr)
        sys.exit(1)

    print(f"Fixture server running on port {FIXTURE_PORT}")
    return proc


def build_cli(project_root: Path) -> None:
    """Build the Rust CLI."""
    print("Building CLI...")
    cli_dir = project_root / "cli"
    result = subprocess.run(
        ["cargo", "build", "--release"],
        cwd=cli_dir,
        check=False,
    )
    if result.returncode != 0:
        print("ERROR: CLI build failed", file=sys.stderr)
        sys.exit(1)
    print("CLI built successfully")


def build_server(project_root: Path) -> None:
    """Build the Rust server."""
    print("Building server...")
    server_dir = project_root / "server"
    result = subprocess.run(
        ["cargo", "build", "--release"],
        cwd=server_dir,
        check=False,
    )
    if result.returncode != 0:
        print("ERROR: Server build failed", file=sys.stderr)
        sys.exit(1)
    print("Server built successfully")


def start_server(project_root: Path, database_url: str) -> subprocess.Popen:
    """Start the Rust server."""
    print(f"Starting server on port {SERVER_PORT}...")
    server_dir = project_root / "server"
    server_binary = server_dir / "target" / "release" / "ramekin-server"

    env = {
        **os.environ,
        "DATABASE_URL": database_url,
        "RUST_LOG": "info",
        "INSECURE_PASSWORD_HASHING": "1",
        "SCRAPE_ALLOWED_HOSTS": f"localhost:{FIXTURE_PORT}",
    }

    proc = subprocess.Popen(
        [str(server_binary)],
        cwd=server_dir,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )

    if not wait_for_port(SERVER_PORT, timeout=30.0):
        stderr_output = ""
        if proc.stderr:
            stderr_output = proc.stderr.read().decode()
        proc.kill()
        print("ERROR: Server failed to start", file=sys.stderr)
        if stderr_output:
            print(f"Server stderr: {stderr_output}", file=sys.stderr)
        sys.exit(1)

    print(f"Server running on port {SERVER_PORT}")
    return proc


def run_tests(project_root: Path) -> int:
    """Run pytest tests."""
    print("\nRunning tests...\n")
    tests_dir = project_root / "tests"

    env = {
        **os.environ,
        "API_BASE_URL": f"http://localhost:{SERVER_PORT}",
        "FIXTURE_BASE_URL": f"http://localhost:{FIXTURE_PORT}",
    }

    result = subprocess.run(
        [
            "uv",
            "run",
            "--with",
            "pytest",
            "--with",
            "urllib3",
            "--with",
            "python-dateutil",
            "--with",
            "pydantic",
            "--with",
            "typing-extensions",
            "--with",
            "requests",
            "pytest",
            str(tests_dir),
            "-v",
        ],
        cwd=project_root,
        env=env,
        check=False,
    )

    return result.returncode


def cleanup(processes: list[subprocess.Popen]) -> None:
    """Clean up background processes."""
    print("\nCleaning up...")
    for proc in processes:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=5)
            except subprocess.TimeoutExpired:
                proc.kill()


def main() -> None:
    """Main execution."""
    project_root = get_project_root()
    processes: list[subprocess.Popen] = []

    # Set up signal handler for cleanup
    def signal_handler(sig, frame):
        cleanup(processes)
        sys.exit(1)

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    try:
        # Check port availability
        for port, name in [(FIXTURE_PORT, "fixture"), (SERVER_PORT, "server")]:
            if not check_port_available(port):
                print(f"ERROR: Port {port} ({name}) is already in use", file=sys.stderr)
                sys.exit(1)

        # Set up environment
        ensure_postgres()
        database_url = setup_test_database()
        run_migrations(project_root, database_url)

        # Start services
        fixture_proc = start_fixture_server(project_root)
        processes.append(fixture_proc)

        build_cli(project_root)
        build_server(project_root)
        server_proc = start_server(project_root, database_url)
        processes.append(server_proc)

        # Run tests
        exit_code = run_tests(project_root)

        # Cleanup and exit
        cleanup(processes)
        sys.exit(exit_code)

    except Exception as e:
        print(f"ERROR: {e}", file=sys.stderr)
        cleanup(processes)
        sys.exit(1)


if __name__ == "__main__":
    main()
