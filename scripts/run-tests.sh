#!/bin/bash
set -e

# Find project root from script location
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Source test.env if present
if [ -f "$PROJECT_ROOT/test.env" ]; then
    set -a  # auto-export all variables
    # shellcheck source=/dev/null
    source "$PROJECT_ROOT/test.env"
    set +a
fi

# Configuration with defaults
FIXTURE_DIR="${FIXTURE_DIR:-$PROJECT_ROOT/tests/scrape_fixtures}"
SERVER_DIR="${SERVER_DIR:-$PROJECT_ROOT/server}"
TESTS_DIR="${TESTS_DIR:-$PROJECT_ROOT/tests}"

# Track PIDs for cleanup
FIXTURE_PID=""
SERVER_PID=""

cleanup() {
    echo "Cleaning up..."
    if [ -n "$FIXTURE_PID" ]; then
        kill "$FIXTURE_PID" 2>/dev/null || true
    fi
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
}
trap cleanup EXIT

# Get a port - use provided value or find a random available one
get_port() {
    local provided="$1"
    if [ -n "$provided" ]; then
        echo "$provided"
    else
        # Find a random port that's not currently in use
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

# Start fixture server
FIXTURE_PORT=$(get_port "$FIXTURE_PORT")
cd "$FIXTURE_DIR"
python3 -m http.server "$FIXTURE_PORT" > /dev/null 2>&1 &
FIXTURE_PID=$!
sleep 0.2
if ! kill -0 "$FIXTURE_PID" 2>/dev/null; then
    echo "Failed to start fixture server on port $FIXTURE_PORT"
    exit 1
fi
echo "Fixture server on port $FIXTURE_PORT"

# Start rust server (PORT comes from test.env if set)
SERVER_PORT=$(get_port "${PORT:-}")
cd "$SERVER_DIR"
export SCRAPE_ALLOWED_HOSTS="localhost:$FIXTURE_PORT"
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

export FIXTURE_BASE_URL="http://localhost:$FIXTURE_PORT"
export API_BASE_URL="http://localhost:$SERVER_PORT"

# Build CLI and set path
cd "$PROJECT_ROOT/cli"
cargo build -q
export CLI_PATH="$PROJECT_ROOT/cli/target/debug/ramekin-cli"

cd "$PROJECT_ROOT"
pytest tests -v
