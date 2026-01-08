#!/bin/bash
set -e

# Find project root from script location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Track PIDs for cleanup
SERVER_PID=""
UI_PID=""

cleanup() {
    echo "Cleaning up..."
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
    if [ -n "$UI_PID" ]; then
        kill "$UI_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Get a port - use provided value or find a random available one
get_port() {
    local provided="$1"
    if [ -n "$provided" ]; then
        echo "$provided"
    else
        local port
        for _ in {1..100}; do
            port=$((RANDOM % 50000 + 10000))
            if ! lsof -i :"$port" > /dev/null 2>&1; then
                echo "$port"
                return 0
            fi
        done
        echo "Could not find available port" >&2
        exit 1
    fi
}

# DATABASE_URL must be set
if [ -z "${DATABASE_URL:-}" ]; then
    echo "DATABASE_URL environment variable required"
    exit 1
fi

# Start API server
SERVER_PORT=$(get_port "${PORT:-}")
cd "$PROJECT_ROOT/server"
PORT="$SERVER_PORT" cargo run --release -q &
SERVER_PID=$!

# Wait for health check
RETRIES=60
while ! curl -sf "http://localhost:$SERVER_PORT/api/test/unauthed-ping" > /dev/null 2>&1; do
    RETRIES=$((RETRIES - 1))
    if [ $RETRIES -le 0 ]; then
        echo "Failed to start server on port $SERVER_PORT (health check timeout)"
        exit 1
    fi
    if ! kill -0 "$SERVER_PID" 2>/dev/null; then
        echo "Failed to start server on port $SERVER_PORT (process died)"
        exit 1
    fi
    sleep 0.3
done
echo "Server on port $SERVER_PORT"

export API_BASE_URL="http://localhost:$SERVER_PORT"

# Start UI dev server
UI_PORT=$(get_port "${UI_PORT:-}")
cd "$PROJECT_ROOT/ramekin-ui"
npm run dev -- --port "$UI_PORT" > /dev/null 2>&1 &
UI_PID=$!

# Wait for UI to be ready
RETRIES=60
while ! curl -sf "http://localhost:$UI_PORT" > /dev/null 2>&1; do
    RETRIES=$((RETRIES - 1))
    if [ $RETRIES -le 0 ]; then
        echo "Failed to start UI on port $UI_PORT (timeout)"
        exit 1
    fi
    if ! kill -0 "$UI_PID" 2>/dev/null; then
        echo "Failed to start UI on port $UI_PORT (process died)"
        exit 1
    fi
    sleep 0.3
done
echo "UI server on port $UI_PORT"

export UI_BASE_URL="http://localhost:$UI_PORT"

# Seed test data
echo "Seeding test data..."
cd "$PROJECT_ROOT/cli"
cargo run -q -- seed --server-url "$API_BASE_URL" --username t --password t ../data/dev/seed.paprikarecipes

# Run UI tests
cd "$PROJECT_ROOT"
pytest tests/ui -v
