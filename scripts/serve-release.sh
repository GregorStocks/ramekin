#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER_DIR="$PROJECT_ROOT/server"

: "${PORT:?PORT environment variable required}"

if ! command -v systemfd >/dev/null 2>&1; then
    echo "systemfd is required for make serve. Install the latest release from https://github.com/mitsuhiko/systemfd/releases/latest." >&2
    exit 1
fi

if ! cargo watch --version >/dev/null 2>&1; then
    echo "cargo-watch is required for make serve." >&2
    exit 1
fi

MEMORY_MAX_MB="${SERVER_MEMORY_MAX_MB:-2048}"

cd "$SERVER_DIR"

if [ "$MEMORY_MAX_MB" -gt 0 ]; then
    MEMORY_MAX_KB=$((MEMORY_MAX_MB * 1024))
    echo "Starting release server on port ${PORT} with socket activation and MemoryMax=${MEMORY_MAX_MB}MiB"
    exec bash -lc "ulimit -v ${MEMORY_MAX_KB}; exec systemfd --no-pid -s http::${PORT} -- cargo watch -x 'run --release -q'"
fi

echo "Starting release server on port ${PORT} with socket activation and no memory cap"
exec systemfd --no-pid -s http::"${PORT}" -- cargo watch -x "run --release -q"
