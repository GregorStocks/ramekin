#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# ///
"""Generate worktree-specific dev.env and test.env files."""

from __future__ import annotations

import argparse
import hashlib
import os
import re
import socket
import subprocess
from dataclasses import dataclass
from pathlib import Path

MAX_PG_IDENTIFIER_LENGTH = 63
DEFAULT_DATABASE_HOST = "localhost"
DEFAULT_DATABASE_PORT = 54321
PORT_ASSIGNMENT_ORDER = (
    "dev_port",
    "dev_ui_port",
    "test_port",
    "test_fixture_port",
    "test_mock_openrouter_port",
    "test_process_compose_port",
    "dev_ui_port_http",
    "test_ui_port",
    "dev_process_compose_port",
)


@dataclass(frozen=True)
class GeneratedConfig:
    """Resolved per-worktree settings."""

    workspace_name: str
    dev_database_name: str
    test_database_name: str
    dev_port: int
    dev_ui_port: int
    dev_ui_port_http: int
    dev_process_compose_port: int
    test_port: int
    test_fixture_port: int
    test_ui_port: int
    test_mock_openrouter_port: int
    test_process_compose_port: int


def parse_args() -> argparse.Namespace:
    """Parse CLI arguments."""
    parser = argparse.ArgumentParser(
        description="Generate dev.env and test.env for a new worktree.",
    )
    parser.add_argument(
        "--workspace-name",
        help=(
            "Workspace name to encode into database names. "
            "Defaults to the repo directory name."
        ),
    )
    parser.add_argument(
        "--base-port",
        type=int,
        help="Use this reserved port block instead of random free ports.",
    )
    parser.add_argument(
        "--database-host",
        default=DEFAULT_DATABASE_HOST,
        help=(
            "Database host to write into DATABASE_URL "
            f"(default: {DEFAULT_DATABASE_HOST})."
        ),
    )
    parser.add_argument(
        "--database-port",
        type=int,
        default=DEFAULT_DATABASE_PORT,
        help=(
            "Database port to write into DATABASE_URL "
            f"(default: {DEFAULT_DATABASE_PORT})."
        ),
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing dev.env and test.env files.",
    )
    return parser.parse_args()


def project_root() -> Path:
    """Return the repository root."""
    return Path(__file__).resolve().parent.parent


def sanitize_workspace_name(workspace_name: str) -> str:
    """Convert the workspace name into a postgres-safe suffix."""
    sanitized = re.sub(r"[^a-z0-9]+", "_", workspace_name.lower()).strip("_")
    return sanitized or "worktree"


def limit_postgres_identifier(name: str) -> str:
    """Clamp postgres identifiers to 63 bytes with a stable hash suffix."""
    if len(name) <= MAX_PG_IDENTIFIER_LENGTH:
        return name

    digest = hashlib.sha1(name.encode("utf-8")).hexdigest()[:8]
    prefix_length = MAX_PG_IDENTIFIER_LENGTH - len(digest) - 1
    truncated = name[:prefix_length].rstrip("_")
    if not truncated:
        truncated = name[:prefix_length]
    return f"{truncated}_{digest}"


def database_names_for_workspace(workspace_name: str) -> tuple[str, str]:
    """Derive workspace-specific dev/test database names."""
    suffix = sanitize_workspace_name(workspace_name)
    dev_name = limit_postgres_identifier(f"ramekin_{suffix}")
    test_name = limit_postgres_identifier(f"ramekin_{suffix}_test")
    return dev_name, test_name


def is_port_available(port: int) -> bool:
    """Return whether a TCP port is currently free on all interfaces."""
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.bind(("", port))
    except OSError:
        return False
    return True


def validate_base_port(base_port: int) -> None:
    """Validate the supplied base port block."""
    highest_port = base_port + len(PORT_ASSIGNMENT_ORDER) - 1
    if base_port <= 0 or highest_port > 65535:
        raise SystemExit(
            f"--base-port {base_port} does not leave enough room "
            "for the required port block."
        )

    unavailable_ports = [
        str(port)
        for port in range(base_port, highest_port + 1)
        if not is_port_available(port)
    ]
    if unavailable_ports:
        joined = ", ".join(unavailable_ports)
        raise SystemExit(f"Requested base port block is not free: {joined}")


def reserve_random_ports(count: int) -> list[int]:
    """Reserve a set of unique random ports long enough to collect their values."""
    sockets: list[socket.socket] = []
    try:
        for _ in range(count):
            sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            sock.bind(("", 0))
            sockets.append(sock)
        return [sock.getsockname()[1] for sock in sockets]
    finally:
        for sock in sockets:
            sock.close()


def generate_config(workspace_name: str, base_port: int | None) -> GeneratedConfig:
    """Build all database names and port assignments."""
    dev_database_name, test_database_name = database_names_for_workspace(workspace_name)

    if base_port is not None:
        validate_base_port(base_port)
        assigned_ports = list(range(base_port, base_port + len(PORT_ASSIGNMENT_ORDER)))
    else:
        assigned_ports = reserve_random_ports(len(PORT_ASSIGNMENT_ORDER))

    ports = dict(zip(PORT_ASSIGNMENT_ORDER, assigned_ports, strict=True))

    return GeneratedConfig(
        workspace_name=workspace_name,
        dev_database_name=dev_database_name,
        test_database_name=test_database_name,
        dev_port=ports["dev_port"],
        dev_ui_port=ports["dev_ui_port"],
        dev_ui_port_http=ports["dev_ui_port_http"],
        dev_process_compose_port=ports["dev_process_compose_port"],
        test_port=ports["test_port"],
        test_fixture_port=ports["test_fixture_port"],
        test_ui_port=ports["test_ui_port"],
        test_mock_openrouter_port=ports["test_mock_openrouter_port"],
        test_process_compose_port=ports["test_process_compose_port"],
    )


def apply_overrides(template: str, overrides: dict[str, str]) -> str:
    """Replace env assignments in a template while preserving comments."""
    rendered_lines: list[str] = []
    applied_keys: set[str] = set()

    for line in template.splitlines():
        match = re.match(r"^([A-Z0-9_]+)=(.*)$", line)
        if match and match.group(1) in overrides:
            key = match.group(1)
            rendered_lines.append(f"{key}={overrides[key]}")
            applied_keys.add(key)
            continue

        rendered_lines.append(line)

    missing_keys = [key for key in overrides if key not in applied_keys]
    if missing_keys:
        rendered_lines.append("")
        rendered_lines.extend(f"{key}={overrides[key]}" for key in missing_keys)

    return "\n".join(rendered_lines) + "\n"


def ensure_outputs_are_writable(output_paths: tuple[Path, ...], force: bool) -> None:
    """Fail before writing if any target env file already exists."""
    if force:
        return

    existing_paths = [path.name for path in output_paths if path.exists()]
    if existing_paths:
        joined = ", ".join(existing_paths)
        raise SystemExit(
            f"{joined} already exist. Re-run with --force to overwrite them."
        )


def write_env_file(
    template_path: Path, output_path: Path, overrides: dict[str, str]
) -> None:
    """Write one env file from its example template."""
    template = template_path.read_text(encoding="utf-8")
    rendered = apply_overrides(template, overrides)
    output_path.write_text(rendered, encoding="utf-8")


def postgres_is_ready(database_host: str, database_port: int) -> bool:
    """Return whether postgres is reachable on the configured host/port."""
    try:
        result = subprocess.run(
            [
                "pg_isready",
                "-h",
                database_host,
                "-p",
                str(database_port),
                "-U",
                "ramekin",
            ],
            check=False,
            capture_output=True,
            text=True,
        )
    except FileNotFoundError:
        return False

    return result.returncode == 0


def ensure_database_exists(
    database_host: str, database_port: int, database_name: str
) -> None:
    """Create the database unless it already exists."""
    result = subprocess.run(
        [
            "createdb",
            "-h",
            database_host,
            "-p",
            str(database_port),
            "-U",
            "ramekin",
            "--no-password",
            database_name,
        ],
        check=False,
        capture_output=True,
        text=True,
        env={**os.environ, "PGPASSWORD": "ramekin"},
    )
    if result.returncode == 0:
        print(f"Created database: {database_name}")
        return

    stderr = result.stderr.strip()
    if "already exists" in stderr.lower():
        print(f"Database already exists: {database_name}")
        return

    detail = stderr or "createdb exited without error output"
    raise SystemExit(f"Failed to create database {database_name}: {detail}")


def create_workspace_databases_if_available(
    config: GeneratedConfig, database_host: str, database_port: int
) -> None:
    """Create workspace databases when postgres is already running."""
    if not postgres_is_ready(database_host, database_port):
        print(
            "Postgres not reachable at "
            f"{database_host}:{database_port}; skipped workspace database creation."
        )
        return

    ensure_database_exists(database_host, database_port, config.dev_database_name)
    ensure_database_exists(database_host, database_port, config.test_database_name)


def print_summary(
    config: GeneratedConfig, database_host: str, database_port: int
) -> None:
    """Print the generated configuration."""
    dev_database_url = (
        f"postgres://ramekin:ramekin@{database_host}:{database_port}/"
        f"{config.dev_database_name}"
    )
    test_database_url = (
        f"postgres://ramekin:ramekin@{database_host}:{database_port}/"
        f"{config.test_database_name}"
    )

    print(f"Configured worktree: {config.workspace_name}")
    print(
        "  dev: "
        f"db={dev_database_url} "
        f"server={config.dev_port} ui={config.dev_ui_port} "
        f"ui_http={config.dev_ui_port_http} "
        f"process_compose={config.dev_process_compose_port}"
    )
    print(
        "  test: "
        f"db={test_database_url} "
        f"server={config.test_port} fixture={config.test_fixture_port} "
        f"ui={config.test_ui_port} "
        f"mock_openrouter={config.test_mock_openrouter_port} "
        f"process_compose={config.test_process_compose_port}"
    )


def main() -> None:
    """Generate dev.env and test.env in the repository root."""
    args = parse_args()
    root = project_root()
    workspace_name = args.workspace_name or root.name
    config = generate_config(workspace_name, args.base_port)

    database_prefix = (
        f"postgres://ramekin:ramekin@{args.database_host}:{args.database_port}"
    )
    dev_overrides = {
        "DATABASE_URL": f"{database_prefix}/{config.dev_database_name}",
        "PORT": str(config.dev_port),
        "UI_PORT": str(config.dev_ui_port),
        "UI_PORT_HTTP": str(config.dev_ui_port_http),
        "PROCESS_COMPOSE_PORT": str(config.dev_process_compose_port),
    }
    test_overrides = {
        "DATABASE_URL": f"{database_prefix}/{config.test_database_name}",
        "PORT": str(config.test_port),
        "FIXTURE_PORT": str(config.test_fixture_port),
        "UI_PORT": str(config.test_ui_port),
        "MOCK_OPENROUTER_PORT": str(config.test_mock_openrouter_port),
        "PROCESS_COMPOSE_PORT": str(config.test_process_compose_port),
    }

    output_paths = (root / "dev.env", root / "test.env")
    ensure_outputs_are_writable(output_paths, args.force)
    write_env_file(root / "dev.env.example", output_paths[0], dev_overrides)
    write_env_file(root / "test.env.example", output_paths[1], test_overrides)
    create_workspace_databases_if_available(
        config, args.database_host, args.database_port
    )
    print_summary(config, args.database_host, args.database_port)


if __name__ == "__main__":
    main()
