#!/bin/bash
# Setup script for Conductor workspaces (local multi-workspace development on Mac)
# This script runs when a new Conductor workspace is created.
# It generates workspace-specific env files, creates databases, and installs dependencies.
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

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
echo "Creating databases..."
createdb -h localhost -p 54321 -U ramekin "$DEV_DB"
createdb -h localhost -p 54321 -U ramekin "$TEST_DB"

# Install npm dependencies
echo "Installing npm dependencies..."
cd "$PROJECT_ROOT/ramekin-ui"
npm install

# Build cargo
echo "Building server..."
cd "$PROJECT_ROOT/server"
cargo build

echo "Workspace setup complete!"
