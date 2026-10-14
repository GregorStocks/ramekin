#!/bin/bash
set -e

# Setup script for Claude Code for Web environment (Anthropic's VMs)
# This script is a no-op unless CLAUDE_CODE_REMOTE=true
#
# NOTE: This is NOT the Conductor setup script. For local multi-workspace
# development with Conductor on Mac, see conductor-setup.sh instead.

if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
    exit 0
fi

echo "Setting up Claude Code for Web environment..."

echo "Installing shared agent-issues hooks..."
uv tool install -U git+https://github.com/GregorStocks/agent-issues.git

# Install system dependencies if not present
if ! command -v shellcheck >/dev/null 2>&1 || ! command -v gh >/dev/null 2>&1 || ! command -v mkcert >/dev/null 2>&1; then
    echo "Installing system dependencies..."
    apt-get update -qq && apt-get install -y -qq shellcheck gh mkcert
fi

# Initialize mkcert if installed but not initialized
if command -v mkcert >/dev/null 2>&1; then
    # mkcert -install sets up the local CA (idempotent, safe to run multiple times)
    mkcert -install 2>/dev/null || true
fi

# Install cargo-watch if not present
if ! command -v cargo-watch >/dev/null 2>&1; then
    echo "Installing cargo-watch..."
    cargo install cargo-watch
fi

# Install ast-grep if not present (used by linter)
if ! command -v ast-grep >/dev/null 2>&1; then
    echo "Installing ast-grep..."
    cargo install ast-grep
fi

# Install process-compose if not present
if ! command -v process-compose >/dev/null 2>&1; then
    echo "Installing process-compose..."
    # Use go install to get process-compose (downloads through Go proxy which is allowed)
    go install github.com/f1bonacc1/process-compose@v1.87.0
    # Link from GOPATH/bin to /usr/local/bin so it's in PATH
    ln -sf "$(go env GOPATH)/bin/process-compose" /usr/local/bin/process-compose
fi

# Create the env files with the worktree's own SQLite paths and ports.
if [ ! -f test.env ] || [ ! -f dev.env ]; then
    echo "Creating dev.env and test.env..."
    uv run scripts/worktree-setup.py --force
fi

echo "Claude Code for Web environment ready"
