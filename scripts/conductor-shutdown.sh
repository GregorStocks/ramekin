#!/bin/bash
# Shutdown script for Conductor workspaces
# Drops workspace-specific databases when archiving

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

# Stop dev processes first
make dev-down

# Require workspace name to derive database names
if [ -z "$CONDUCTOR_WORKSPACE_NAME" ]; then
    echo "Warning: CONDUCTOR_WORKSPACE_NAME not set, skipping database cleanup"
    exit 0
fi

# Derive database names (same logic as setup)
DB_SUFFIX=$(echo "$CONDUCTOR_WORKSPACE_NAME" | tr '-' '_' | tr '[:upper:]' '[:lower:]')
DEV_DB="ramekin_${DB_SUFFIX}"
TEST_DB="ramekin_${DB_SUFFIX}_test"

echo "Dropping databases: $DEV_DB, $TEST_DB"

# Drop databases (ignore errors if they don't exist)
PGPASSWORD=ramekin dropdb -h localhost -p 54321 -U ramekin --if-exists "$DEV_DB" || true
PGPASSWORD=ramekin dropdb -h localhost -p 54321 -U ramekin --if-exists "$TEST_DB" || true

echo "Database cleanup complete"
