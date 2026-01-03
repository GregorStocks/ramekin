#!/usr/bin/env python3
"""
Generate OpenAPI spec with smart caching.

This script:
- Calculates a hash of API source files
- Skips generation if API hasn't changed (smart caching)
- Generates OpenAPI spec by building the server in Docker
- Regenerates all API clients (Rust, TypeScript, Python)
- Runs linters on success
"""

import hashlib
import os
import subprocess
import sys
import tempfile
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


def needs_regeneration(cache_file: Path, openapi_spec: Path, current_hash: str) -> bool:
    """Check if OpenAPI spec needs to be regenerated."""
    if not cache_file.exists():
        return True

    if not openapi_spec.exists():
        return True

    cached_hash = cache_file.read_text().strip()
    return cached_hash != current_hash


def generate_openapi_spec(openapi_spec: Path) -> None:
    """Generate OpenAPI spec by building server and running --openapi flag."""
    print("Building server and generating OpenAPI spec...")

    project_root = get_project_root()
    server_target = project_root / "server/target"
    server_target.mkdir(parents=True, exist_ok=True)

    # Create temp file for error logs
    with tempfile.NamedTemporaryFile(mode="w+", delete=False) as temp_log:
        temp_log_path = temp_log.name

    try:
        # Run docker to build server and generate spec
        result = subprocess.run(
            [
                "docker",
                "run",
                "--rm",
                "-u",
                f"{os.getuid()}:{os.getgid()}",
                "-v",
                f"{project_root}:/app:z",
                "-v",
                f"{server_target}:/app/server/target:z",
                "-w",
                "/app/server",
                "rust:latest",
                "sh",
                "-c",
                "cargo build --release -q && target/release/ramekin-server --openapi",
            ],
            stdout=subprocess.PIPE,
            stderr=open(temp_log_path, "w"),
            check=False,
            timeout=300,
        )

        if result.returncode != 0:
            print("Error: Failed to generate OpenAPI spec", file=sys.stderr)
            with open(temp_log_path) as f:
                print(f.read(), file=sys.stderr)
            sys.exit(1)

        # Write spec to file
        openapi_spec.write_bytes(result.stdout)
        print(f"Generated {openapi_spec}")

    finally:
        Path(temp_log_path).unlink(missing_ok=True)


def main() -> None:
    """Main execution."""
    project_root = get_project_root()

    # Paths
    cache_dir = project_root / ".cache"
    api_dir = project_root / "api"
    openapi_spec = api_dir / "openapi.json"
    hash_file = cache_dir / "openapi-hash"

    # Ensure directories exist
    cache_dir.mkdir(exist_ok=True)
    api_dir.mkdir(exist_ok=True)

    # Calculate current API hash
    current_hash = calculate_api_hash()

    # Check if we need to regenerate
    if not needs_regeneration(hash_file, openapi_spec, current_hash):
        print(f"API unchanged ({current_hash}), skipping generation")
        return

    print(f"API changed or missing, regenerating ({current_hash})...")

    # Generate OpenAPI spec
    generate_openapi_spec(openapi_spec)

    # Generate all clients (Rust, TypeScript, Python)
    generate_clients_script = project_root / "scripts/generate-clients.sh"
    subprocess.run([str(generate_clients_script)], check=True, timeout=300)

    # Update cached hash
    hash_file.write_text(current_hash)

    # Run linters
    print("Running linters...")
    subprocess.run(["make", "lint"], cwd=project_root, check=True, timeout=300)


if __name__ == "__main__":
    main()
