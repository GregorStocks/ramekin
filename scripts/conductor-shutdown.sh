#!/bin/bash
# Shutdown script for Conductor workspaces
# Drops workspace-specific databases when archiving

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

# Stop dev processes first
make dev-down

DEV_DB=""
TEST_DB=""

if [ -f dev.env ]; then
    DEV_DB=$(grep '^DATABASE_URL=' dev.env | sed 's|.*/||')
fi

if [ -f test.env ]; then
    TEST_DB=$(grep '^DATABASE_URL=' test.env | sed 's|.*/||')
fi

if [ -z "$DEV_DB" ] || [ -z "$TEST_DB" ]; then
    if [ -z "$CONDUCTOR_WORKSPACE_NAME" ]; then
        echo "Warning: CONDUCTOR_WORKSPACE_NAME not set and env files missing, skipping database cleanup"
        exit 0
    fi

    DB_SUFFIX=$(echo "$CONDUCTOR_WORKSPACE_NAME" | tr '-' '_' | tr '[:upper:]' '[:lower:]')
    DEV_DB="ramekin_${DB_SUFFIX}"
    TEST_DB="ramekin_${DB_SUFFIX}_test"
fi

echo "Dropping databases: $DEV_DB, $TEST_DB"

# Drop databases (ignore errors if they don't exist)
PGPASSWORD=ramekin dropdb -h localhost -p 54321 -U ramekin --if-exists "$DEV_DB" || true
PGPASSWORD=ramekin dropdb -h localhost -p 54321 -U ramekin --if-exists "$TEST_DB" || true

echo "Database cleanup complete"
