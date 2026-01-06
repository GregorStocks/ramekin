#!/bin/bash
# Setup script for Conductor workspaces (local multi-workspace development on Mac)
# This script runs when a new Conductor workspace is created.
# It generates workspace-specific env files, creates databases, and installs dependencies.
set -e
set -o pipefail

TIMEOUT_SECONDS=300  # 5 minutes

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

do_setup() {
    cd "$PROJECT_ROOT"

    echo ""
    echo "=========================================="
    echo "Setup started at $(date)"
    echo "=========================================="

    # Require Conductor-provided values
    if [ -z "$CONDUCTOR_PORT" ] || [ -z "$CONDUCTOR_WORKSPACE_NAME" ]; then
        echo "Error: CONDUCTOR_PORT and CONDUCTOR_WORKSPACE_NAME must be set"
        echo "This script is meant to be run by Conductor, not manually."
        exit 1
    fi

    BASE_PORT="$CONDUCTOR_PORT"
    WORKSPACE_NAME="$CONDUCTOR_WORKSPACE_NAME"

    echo "Setting up workspace: $WORKSPACE_NAME (base port: $BASE_PORT)"

    # Derive ports (Conductor provides 10 consecutive ports starting at CONDUCTOR_PORT)
    DEV_PORT=$BASE_PORT
    DEV_UI_PORT=$((BASE_PORT + 1))
    TEST_PORT=$((BASE_PORT + 2))
    TEST_FIXTURE_PORT=$((BASE_PORT + 3))

    # Derive database names (sanitize workspace name for postgres)
    DB_SUFFIX=$(echo "$WORKSPACE_NAME" | tr '-' '_' | tr '[:upper:]' '[:lower:]')
    DEV_DB="ramekin_${DB_SUFFIX}"
    TEST_DB="ramekin_${DB_SUFFIX}_test"

    echo "Ports: server=$DEV_PORT, ui=$DEV_UI_PORT, test=$TEST_PORT, fixture=$TEST_FIXTURE_PORT"
    echo "Databases: dev=$DEV_DB, test=$TEST_DB"

    # Generate dev.env
    cat > dev.env << EOF
DATABASE_URL=postgres://ramekin:ramekin@localhost:54321/${DEV_DB}
PORT=${DEV_PORT}
UI_PORT=${DEV_UI_PORT}
RUST_LOG=debug
INSECURE_PASSWORD_HASHING=1
EOF
    echo "Created dev.env"

    # Generate test.env
    cat > test.env << EOF
DATABASE_URL=postgres://ramekin:ramekin@localhost:54321/${TEST_DB}
PORT=${TEST_PORT}
FIXTURE_PORT=${TEST_FIXTURE_PORT}
RUST_LOG=error
INSECURE_PASSWORD_HASHING=1
EOF
    echo "Created test.env"

    # Create databases (requires postgres running on port 54321)
    echo ""
    echo "Creating databases..."
    if ! PGPASSWORD=ramekin createdb -h localhost -p 54321 -U ramekin --no-password "$DEV_DB" 2>&1; then
        # Check if database already exists (this is fine, not an error)
        if PGPASSWORD=ramekin psql -h localhost -p 54321 -U ramekin -lqt 2>/dev/null | grep -q "$DEV_DB"; then
            echo "Database $DEV_DB already exists"
        else
            echo "ERROR: Failed to create database $DEV_DB" >&2
            exit 1
        fi
    fi
    if ! PGPASSWORD=ramekin createdb -h localhost -p 54321 -U ramekin --no-password "$TEST_DB" 2>&1; then
        # Check if database already exists (this is fine, not an error)
        if PGPASSWORD=ramekin psql -h localhost -p 54321 -U ramekin -lqt 2>/dev/null | grep -q "$TEST_DB"; then
            echo "Database $TEST_DB already exists"
        else
            echo "ERROR: Failed to create database $TEST_DB" >&2
            exit 1
        fi
    fi

    # Install npm dependencies
    echo ""
    echo "Installing npm dependencies..."
    cd "$PROJECT_ROOT/ramekin-ui"
    npm install --loglevel verbose

    # Build cargo
    echo ""
    echo "Building server (this may take a while if not cached)..."
    cd "$PROJECT_ROOT/server"
    cargo build

    echo ""
    echo "Workspace setup complete!"
}

# Run setup with timeout, piping all output through timestamp wrapper and to log file
mkdir -p "$PROJECT_ROOT/logs"

# Capture this script's PID for the watchdog to use
MAIN_PID=$$

# Start a background watchdog that kills the entire process group after timeout
(
    sleep "$TIMEOUT_SECONDS"
    echo "ERROR: Setup timed out after $TIMEOUT_SECONDS seconds" >&2
    kill -TERM 0  # Kill all processes in the current process group
) &
WATCHDOG_PID=$!

# Ensure watchdog is killed when we exit (success or failure)
trap "kill $WATCHDOG_PID 2>/dev/null" EXIT

do_setup 2>&1 | "$SCRIPT_DIR/ts" | tee -a "$PROJECT_ROOT/logs/conductor-setup.log"
