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
    MOCK_OPENROUTER_PORT=$((BASE_PORT + 4))
    PROCESS_COMPOSE_PORT=$((BASE_PORT + 5))

    # Derive database names (sanitize workspace name for postgres)
    DB_SUFFIX=$(echo "$WORKSPACE_NAME" | tr '-' '_' | tr '[:upper:]' '[:lower:]')
    DEV_DB="ramekin_${DB_SUFFIX}"
    TEST_DB="ramekin_${DB_SUFFIX}_test"

    echo "Ports: server=$DEV_PORT, ui=$DEV_UI_PORT, test=$TEST_PORT, fixture=$TEST_FIXTURE_PORT, mock_openrouter=$MOCK_OPENROUTER_PORT, process_compose=$PROCESS_COMPOSE_PORT"
    echo "Databases: dev=$DEV_DB, test=$TEST_DB"

    # Generate dev.env
    cat > dev.env << EOF
DATABASE_URL=postgres://ramekin:ramekin@localhost:54321/${DEV_DB}
PORT=${DEV_PORT}
UI_PORT=${DEV_UI_PORT}
RUST_LOG=info
INSECURE_PASSWORD_HASHING=1
EOF
    echo "Created dev.env"

    # Generate test.env
    cat > test.env << EOF
DATABASE_URL=postgres://ramekin:ramekin@localhost:54321/${TEST_DB}
PORT=${TEST_PORT}
FIXTURE_PORT=${TEST_FIXTURE_PORT}
MOCK_OPENROUTER_PORT=${MOCK_OPENROUTER_PORT}
PROCESS_COMPOSE_PORT=${PROCESS_COMPOSE_PORT}
RUST_LOG=error
INSECURE_PASSWORD_HASHING=1
EOF
    echo "Created test.env"

    # Sync keys from source directory (if available)
    SOURCE_DIR="$HOME/code/ramekin"
    if [ -d "$SOURCE_DIR" ]; then
        echo ""
        echo "Syncing keys from $SOURCE_DIR..."

        # Create cli.env with OPENROUTER_API_KEY
        if [ -f "$SOURCE_DIR/cli.env" ]; then
            OPENROUTER_KEY=$(grep '^OPENROUTER_API_KEY=' "$SOURCE_DIR/cli.env" | head -1)
            if [ -n "$OPENROUTER_KEY" ]; then
                echo "$OPENROUTER_KEY" > cli.env
                echo "Created cli.env with OPENROUTER_API_KEY"
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

    # Create Claude settings with lint hook
    echo ""
    echo "Creating Claude settings..."
    if [ -f .claude/settings.local.json ]; then
        echo "Error: .claude/settings.local.json already exists"
        exit 1
    fi
    mkdir -p .claude
    cat > .claude/settings.local.json << 'EOF'
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Write|Edit",
        "hooks": [
          {
            "type": "command",
            "command": "make lint"
          }
        ]
      }
    ]
  }
}
EOF
    echo "Created .claude/settings.local.json"

    # Create databases (requires postgres running on port 54321)
    echo ""
    echo "Creating databases..."
    PGPASSWORD=ramekin createdb -h localhost -p 54321 -U ramekin --no-password "$DEV_DB"
    PGPASSWORD=ramekin createdb -h localhost -p 54321 -U ramekin --no-password "$TEST_DB"

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
