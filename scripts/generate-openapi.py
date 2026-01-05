#!/usr/bin/env python3
"""
Generate OpenAPI spec with smart caching.

This script:
- Calculates a hash of API source files
- Skips generation if API hasn't changed (smart caching)
- Generates OpenAPI spec by building the server
- Regenerates all API clients (Rust, TypeScript, Python)
- Runs linters on success
"""

import hashlib
import subprocess
import sys
from pathlib import Path


def get_project_root() -> Path:
    """Get the project root directory."""
    return Path(__file__).parent.parent


def calculate_api_hash() -> str:
    """Calculate hash of all API source files."""
    project_root = get_project_root()
    api_sources = [
        project_root / "server/src/api",
        project_root / "server/src/models.rs",
        project_root / "server/src/schema.rs",
    ]

    # Collect all file hashes
    file_hashes = []
    for source in api_sources:
        if source.is_dir():
            for file_path in sorted(source.rglob("*")):
                if file_path.is_file():
                    with open(file_path, "rb") as f:
                        file_hash = hashlib.md5(f.read()).hexdigest()
                        file_hashes.append(f"{file_hash}  {file_path}")
        elif source.is_file():
            with open(source, "rb") as f:
                file_hash = hashlib.md5(f.read()).hexdigest()
                file_hashes.append(f"{file_hash}  {source}")

    # Sort and hash the combined hashes
    combined = "\n".join(sorted(file_hashes))
    return hashlib.md5(combined.encode()).hexdigest()


def clients_exist(project_root: Path) -> bool:
    """Check if all generated clients exist."""
    client_markers = [
        project_root / "cli/generated/ramekin-client/Cargo.toml",
        project_root / "ramekin-ui/generated-client/dist/index.js",
        project_root / "tests/generated/ramekin_client/__init__.py",
    ]
    return all(marker.exists() for marker in client_markers)


def needs_regeneration(cache_file: Path, openapi_spec: Path, current_hash: str) -> bool:
    """Check if OpenAPI spec needs to be regenerated."""
    if not cache_file.exists():
        return True

    if not openapi_spec.exists():
        return True

    cached_hash = cache_file.read_text().strip()
    return cached_hash != current_hash


def generate_openapi_spec(openapi_spec: Path) -> None:
    """Generate OpenAPI spec by building server."""
    print("Building server and generating OpenAPI spec...")

    project_root = get_project_root()
    server_dir = project_root / "server"

    # Build the server (10 min timeout for uncached CI builds)
    build_result = subprocess.run(
        ["cargo", "build", "--release", "-q"],
        cwd=server_dir,
        check=False,
        timeout=600,
    )

    if build_result.returncode != 0:
        print("Error: Failed to build server", file=sys.stderr)
        sys.exit(1)

    # Generate OpenAPI spec
    result = subprocess.run(
        ["target/release/ramekin-server", "--openapi"],
        cwd=server_dir,
        stdout=subprocess.PIPE,
        check=False,
        timeout=30,
    )

    if result.returncode != 0:
        print("Error: Failed to generate OpenAPI spec", file=sys.stderr)
        sys.exit(1)

    openapi_spec.write_bytes(result.stdout)
    print(f"Generated {openapi_spec}")


def main() -> None:
    """Main execution."""
    project_root = get_project_root()

    # Paths
    cache_dir = project_root / ".cache"
    api_dir = project_root / "api"
    openapi_spec = api_dir / "openapi.json"
    hash_file = api_dir / "openapi-hash"

    # Ensure directories exist
    cache_dir.mkdir(exist_ok=True)
    api_dir.mkdir(exist_ok=True)

    # Calculate current API hash
    current_hash = calculate_api_hash()
    needs_spec = needs_regeneration(hash_file, openapi_spec, current_hash)
    has_clients = clients_exist(project_root)

    if not needs_spec and has_clients:
        print(f"API unchanged ({current_hash}), skipping generation")
        return

    if needs_spec:
        print(f"API changed or missing, regenerating ({current_hash})...")
        generate_openapi_spec(openapi_spec)
    else:
        print("Clients missing, regenerating from existing spec...")

    generate_clients_script = project_root / "scripts/generate-clients.sh"
    subprocess.run([str(generate_clients_script)], check=True, timeout=300)

    if needs_spec:
        hash_file.write_text(current_hash)

    print("Running linters...")
    lint_script = project_root / "scripts/lint.py"
    subprocess.run([str(lint_script)], check=True, timeout=300)


if __name__ == "__main__":
    main()
