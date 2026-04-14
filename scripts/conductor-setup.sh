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

    ./scripts/worktree-setup.py \
        --workspace-name "$WORKSPACE_NAME" \
        --base-port "$BASE_PORT" \
        --force

    DEV_DB=$(grep '^DATABASE_URL=' dev.env | sed 's|.*/||')
    TEST_DB=$(grep '^DATABASE_URL=' test.env | sed 's|.*/||')
    echo "Created dev.env and test.env"

    # Sync keys from source directory (if available)
    SOURCE_DIR="$HOME/code/ramekin"
    if [ -d "$SOURCE_DIR" ]; then
        echo ""
        echo "Syncing keys from $SOURCE_DIR..."

        # Create cli.env with OPENROUTER_API_KEY and also add to dev.env for server
        if [ -f "$SOURCE_DIR/cli.env" ]; then
            OPENROUTER_KEY=$(grep '^OPENROUTER_API_KEY=' "$SOURCE_DIR/cli.env" | head -1)
            if [ -n "$OPENROUTER_KEY" ]; then
                echo "$OPENROUTER_KEY" > cli.env
                echo "Created cli.env with OPENROUTER_API_KEY"
                TMP_DEV_ENV=$(mktemp)
                awk -v openrouter_key="$OPENROUTER_KEY" '
                    BEGIN { replaced = 0 }
                    /^OPENROUTER_API_KEY=/ {
                        if (!replaced) {
                            print openrouter_key
                            replaced = 1
                        }
                        next
                    }
                    { print }
                    END {
                        if (!replaced) {
                            if (NR > 0) {
                                print ""
                            }
                            print "# AI enrichment (synced from source)"
                            print openrouter_key
                        }
                    }
                ' dev.env > "$TMP_DEV_ENV"
                mv "$TMP_DEV_ENV" dev.env
                echo "Updated OPENROUTER_API_KEY in dev.env"
            fi
        fi

        # Append OTEL config to dev.env
        if [ -f "$SOURCE_DIR/dev.env" ]; then
            OTEL_ENDPOINT=$(grep '^OTEL_EXPORTER_OTLP_ENDPOINT=' "$SOURCE_DIR/dev.env" | head -1)
            OTEL_HEADERS=$(grep '^OTEL_EXPORTER_OTLP_HEADERS=' "$SOURCE_DIR/dev.env" | head -1)
            OTEL_SERVICE=$(grep '^OTEL_SERVICE_NAME=' "$SOURCE_DIR/dev.env" | head -1)

            if [ -n "$OTEL_ENDPOINT" ]; then
                {
                    echo ""
                    echo "# OpenTelemetry (synced from source)"
                    echo "$OTEL_ENDPOINT"
                    [ -n "$OTEL_HEADERS" ] && echo "$OTEL_HEADERS"
                    [ -n "$OTEL_SERVICE" ] && echo "$OTEL_SERVICE"
                } >> dev.env
                echo "Appended OTEL config to dev.env"
            fi
        fi
    else
        echo "Note: $SOURCE_DIR not found, skipping key sync"
    fi

    # Create databases (requires postgres running on port 54321)
    echo ""
    echo "Creating databases..."
    PGPASSWORD=ramekin createdb -h localhost -p 54321 -U ramekin --no-password "$DEV_DB"
    PGPASSWORD=ramekin createdb -h localhost -p 54321 -U ramekin --no-password "$TEST_DB"

    # Install npm dependencies
    echo ""
    echo "Installing npm dependencies..."
    cd "$PROJECT_ROOT/ramekin-ui"
    npm ci --loglevel verbose

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

# Start a background watchdog that kills the entire process group after timeout
(
    sleep "$TIMEOUT_SECONDS"
    echo "ERROR: Setup timed out after $TIMEOUT_SECONDS seconds" >&2
    kill -TERM 0  # Kill all processes in the current process group
) &
WATCHDOG_PID=$!

# Ensure watchdog is killed when we exit (success or failure)
# Note: We intentionally expand WATCHDOG_PID now (not at signal time)
trap 'kill '"$WATCHDOG_PID"' 2>/dev/null' EXIT

do_setup 2>&1 | "$SCRIPT_DIR/ts" | tee -a "$PROJECT_ROOT/logs/conductor-setup.log"
