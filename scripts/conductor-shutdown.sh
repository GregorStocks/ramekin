#!/bin/bash
# Shutdown script for Conductor workspaces. The workspace's SQLite databases
# live in its .db/ directory and go away with the workspace.

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$PROJECT_ROOT"

make dev-down
