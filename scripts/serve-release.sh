#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SERVER_DIR="$PROJECT_ROOT/server"
SERVER_BIN="$SERVER_DIR/target/release/ramekin-server"

: "${PORT:?PORT environment variable required}"

if ! command -v systemfd >/dev/null 2>&1; then
    echo "systemfd is required for make serve. Install the latest release from https://github.com/mitsuhiko/systemfd/releases/latest." >&2
    exit 1
fi

if [ ! -x "$SERVER_BIN" ]; then
    echo "Release binary missing at $SERVER_BIN. Run 'make server-release-build' first." >&2
    exit 1
fi

MEMORY_MAX_MB="${SERVER_MEMORY_MAX_MB:-4096}"

cd "$SERVER_DIR"

if [ "$MEMORY_MAX_MB" -gt 0 ]; then
    MEMORY_MAX_KB=$((MEMORY_MAX_MB * 1024))
    echo "Starting release server on port ${PORT} with socket activation and MemoryMax=${MEMORY_MAX_MB}MiB"
    exec bash -lc "ulimit -v ${MEMORY_MAX_KB}; exec systemfd --no-pid -s http::0.0.0.0:${PORT} -- ${SERVER_BIN}"
fi

echo "Starting release server on port ${PORT} with socket activation and no memory cap"
exec systemfd --no-pid -s http::0.0.0.0:"${PORT}" -- "${SERVER_BIN}"
