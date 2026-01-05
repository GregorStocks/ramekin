#!/bin/bash
set -e

# Detect if running in Docker (script is in /usr/local/bin) or natively
if [ -d "/app/server" ]; then
    # Running in Docker
    PROJECT_ROOT="/app"
else
    # Running natively - use script location to find project root
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

    # Source test.env when running natively
    if [ -f "$PROJECT_ROOT/test.env" ]; then
        set -a  # auto-export all variables
        # shellcheck source=/dev/null
        source "$PROJECT_ROOT/test.env"
        set +a
    fi
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
    [ -n "$FIXTURE_PID" ] && kill $FIXTURE_PID 2>/dev/null || true
    [ -n "$SERVER_PID" ] && kill $SERVER_PID 2>/dev/null || true
}
trap cleanup EXIT

# Start fixture server on random port
start_fixture_server() {
    cd "$FIXTURE_DIR"
    for _ in {1..100}; do
        FIXTURE_PORT=$((RANDOM % 50000 + 10000))
        python3 -m http.server $FIXTURE_PORT > /dev/null 2>&1 &
        FIXTURE_PID=$!
        sleep 0.2
        if kill -0 $FIXTURE_PID 2>/dev/null; then
            echo "Fixture server on port $FIXTURE_PORT"
            return 0
        fi
    done
    echo "Failed to start fixture server"
    return 1
}

# Start rust server on random port
start_server() {
    cd "$SERVER_DIR"

    # Set SCRAPE_ALLOWED_HOSTS before server starts
    export SCRAPE_ALLOWED_HOSTS="localhost:$FIXTURE_PORT"

    for _ in {1..100}; do
        SERVER_PORT=$((RANDOM % 50000 + 10000))
        PORT=$SERVER_PORT cargo run --release -q &
        SERVER_PID=$!
        sleep 0.3
        if kill -0 $SERVER_PID 2>/dev/null; then
            # Wait for health check
            RETRIES=60
            while ! curl -sf "http://localhost:$SERVER_PORT/api/test/unauthed-ping" > /dev/null 2>&1; do
                RETRIES=$((RETRIES - 1))
                if [ $RETRIES -le 0 ]; then
                    kill $SERVER_PID 2>/dev/null || true
                    break
                fi
                if ! kill -0 $SERVER_PID 2>/dev/null; then
                    break
                fi
                sleep 0.3
            done
            if kill -0 $SERVER_PID 2>/dev/null; then
                echo "Server on port $SERVER_PORT"
                return 0
            fi
        fi
    done
    echo "Failed to start server"
    return 1
}

# Main
start_fixture_server
start_server

export FIXTURE_BASE_URL="http://localhost:$FIXTURE_PORT"
export API_BASE_URL="http://localhost:$SERVER_PORT"

# Build CLI and set path
cd "$PROJECT_ROOT/cli"
if [ "$PROJECT_ROOT" = "/app" ]; then
    # In Docker: use separate target dir to avoid arch conflicts with mounted host target
    CARGO_TARGET_DIR=/tmp/cli-target cargo build -q
    export CLI_PATH="/tmp/cli-target/debug/ramekin-cli"
else
    cargo build -q
    export CLI_PATH="$PROJECT_ROOT/cli/target/debug/ramekin-cli"
fi

cd "$PROJECT_ROOT"
pytest tests -v
