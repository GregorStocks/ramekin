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
import threading
import time
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
    """Generate OpenAPI spec by building server and running --openapi flag."""
    print("Building server and generating OpenAPI spec...")
    print("(This may take several minutes on first run or in CI environments)")

    project_root = get_project_root()
    server_target = project_root / "server/target"
    server_target.mkdir(parents=True, exist_ok=True)

    # Run docker to build server and generate spec
    # Cargo output goes to stderr, OpenAPI JSON goes to stdout
    cmd = [
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
        # Show cargo build progress on stderr, OpenAPI JSON on stdout
        "cargo build --release && target/release/ramekin-server --openapi",
    ]

    process = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )

    # Stream stderr (cargo output) in real-time while capturing stdout (JSON)
    stderr_lines: list[str] = []

    def stream_stderr() -> None:
        assert process.stderr is not None
        for line in iter(process.stderr.readline, b""):
            decoded = line.decode(errors="replace")
            stderr_lines.append(decoded)
            print(decoded, end="", flush=True)
        process.stderr.close()

    stderr_thread = threading.Thread(target=stream_stderr)
    stderr_thread.start()

    try:
        # Read stdout (the OpenAPI JSON) with timeout
        start_time = time.time()
        timeout_seconds = 300
        stdout_chunks: list[bytes] = []

        assert process.stdout is not None
        while True:
            elapsed = time.time() - start_time
            if elapsed > timeout_seconds:
                process.kill()
                stderr_thread.join(timeout=5)
                print(
                    f"\nError: Build timed out after {timeout_seconds} seconds",
                    file=sys.stderr,
                )
                if stderr_lines:
                    print("Last stderr output:", file=sys.stderr)
                    # Print last 50 lines of stderr
                    for line in stderr_lines[-50:]:
                        print(line, end="", file=sys.stderr)
                sys.exit(1)

            # Check if process has finished
            if process.poll() is not None:
                # Process finished, read remaining stdout
                stdout_chunks.append(process.stdout.read())
                break

            # Small sleep to avoid busy-waiting
            time.sleep(0.1)

        stderr_thread.join(timeout=10)
        stdout = b"".join(stdout_chunks)

        if process.returncode != 0:
            print("\nError: Failed to generate OpenAPI spec", file=sys.stderr)
            if stdout:
                print("stdout:", file=sys.stderr)
                print(stdout.decode(errors="replace"), file=sys.stderr)
            sys.exit(1)

        # Parse the OpenAPI JSON from stdout
        stdout_text = stdout.decode()
        if not stdout_text.strip():
            print("Error: No OpenAPI JSON output received", file=sys.stderr)
            sys.exit(1)

        # The entire stdout should be the JSON
        openapi_spec.write_text(stdout_text)
        print(f"\nGenerated {openapi_spec}")

    except Exception as e:
        process.kill()
        stderr_thread.join(timeout=5)
        print(f"\nError during build: {e}", file=sys.stderr)
        if stderr_lines:
            print("Last stderr output:", file=sys.stderr)
            for line in stderr_lines[-50:]:
                print(line, end="", file=sys.stderr)
        sys.exit(1)


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
