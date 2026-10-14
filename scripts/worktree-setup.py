#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.10"
# ///
"""Generate worktree-specific dev.env and test.env files."""

from __future__ import annotations

import argparse
import re
import socket
from dataclasses import dataclass
from pathlib import Path

# SQLite files, inside the worktree so each one has its own.
DATABASE_DIR = ".db"
PORT_ASSIGNMENT_ORDER = (
    "dev_port",
    "dev_ui_port",
    "test_port",
    "test_fixture_port",
    "test_mock_openrouter_port",
    "test_process_compose_port",
    "dev_ui_port_http",
    "test_ui_port",
    "test_ui_port_http",
    "dev_process_compose_port",
)


@dataclass(frozen=True)
class GeneratedConfig:
    """Resolved per-worktree settings."""

    workspace_name: str
    dev_port: int
    dev_ui_port: int
    dev_ui_port_http: int
    dev_process_compose_port: int
    test_port: int
    test_fixture_port: int
    test_ui_port: int
    test_ui_port_http: int
    test_mock_openrouter_port: int
    test_process_compose_port: int


def parse_args() -> argparse.Namespace:
    """Parse CLI arguments."""
    parser = argparse.ArgumentParser(
        description="Generate dev.env and test.env for a new worktree.",
    )
    parser.add_argument(
        "--workspace-name",
        help="Workspace name to report. Defaults to the repo directory name.",
    )
    parser.add_argument(
        "--base-port",
        type=int,
        help="Use this reserved port block instead of random free ports.",
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
    """Build all port assignments."""
    if base_port is not None:
        validate_base_port(base_port)
        assigned_ports = list(range(base_port, base_port + len(PORT_ASSIGNMENT_ORDER)))
    else:
        assigned_ports = reserve_random_ports(len(PORT_ASSIGNMENT_ORDER))

    ports = dict(zip(PORT_ASSIGNMENT_ORDER, assigned_ports, strict=True))

    return GeneratedConfig(
        workspace_name=workspace_name,
        dev_port=ports["dev_port"],
        dev_ui_port=ports["dev_ui_port"],
        dev_ui_port_http=ports["dev_ui_port_http"],
        dev_process_compose_port=ports["dev_process_compose_port"],
        test_port=ports["test_port"],
        test_fixture_port=ports["test_fixture_port"],
        test_ui_port=ports["test_ui_port"],
        test_ui_port_http=ports["test_ui_port_http"],
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


def database_paths(root: Path) -> tuple[Path, Path]:
    """The dev and test SQLite database files for a worktree."""
    return root / DATABASE_DIR / "dev.sqlite", root / DATABASE_DIR / "test.sqlite"


def print_summary(
    config: GeneratedConfig, dev_database_url: Path, test_database_url: Path
) -> None:
    """Print the generated configuration."""

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
        f"ui={config.test_ui_port} ui_http={config.test_ui_port_http} "
        f"mock_openrouter={config.test_mock_openrouter_port} "
        f"process_compose={config.test_process_compose_port}"
    )


def main() -> None:
    """Generate dev.env and test.env in the repository root."""
    args = parse_args()
    root = project_root()
    workspace_name = args.workspace_name or root.name
    config = generate_config(workspace_name, args.base_port)

    dev_database, test_database = database_paths(root)
    # Dev: `make dev` runs setup-certs.sh before launching vite, so UI_PORT serves
    # HTTPS. External links point at that.
    # Test: vite only has certs when a prior dev run generated them, so cert state
    # is unpredictable. Point at the HTTP mirror (UI_PORT_HTTP), which vite always
    # serves via httpMirrorPlugin regardless of cert presence.
    dev_overrides = {
        "DATABASE_URL": str(dev_database),
        "PORT": str(config.dev_port),
        "UI_PORT": str(config.dev_ui_port),
        "UI_PORT_HTTP": str(config.dev_ui_port_http),
        "PROCESS_COMPOSE_PORT": str(config.dev_process_compose_port),
        "RAMEKIN_SELF_SIGNED_URL": f"https://localhost:{config.dev_ui_port}",
        "RAMEKIN_EXTERNAL_URL": f"https://localhost:{config.dev_ui_port}",
    }
    test_overrides = {
        "DATABASE_URL": str(test_database),
        "PORT": str(config.test_port),
        "FIXTURE_PORT": str(config.test_fixture_port),
        "UI_PORT": str(config.test_ui_port),
        "UI_PORT_HTTP": str(config.test_ui_port_http),
        "MOCK_OPENROUTER_PORT": str(config.test_mock_openrouter_port),
        "PROCESS_COMPOSE_PORT": str(config.test_process_compose_port),
        "RAMEKIN_SELF_SIGNED_URL": f"http://localhost:{config.test_ui_port}",
        "RAMEKIN_EXTERNAL_URL": f"http://localhost:{config.test_ui_port_http}",
    }

    output_paths = (root / "dev.env", root / "test.env")
    ensure_outputs_are_writable(output_paths, args.force)
    write_env_file(root / "dev.env.example", output_paths[0], dev_overrides)
    write_env_file(root / "test.env.example", output_paths[1], test_overrides)
    print_summary(config, dev_database, test_database)


if __name__ == "__main__":
    main()
