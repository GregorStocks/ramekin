#!/bin/bash
set -e

# Setup script for Claude Code for Web environment
# This script is a no-op unless CLAUDE_CODE_REMOTE=true

if [ "$CLAUDE_CODE_REMOTE" != "true" ]; then
    exit 0
fi

echo "Setting up Claude Code for Web environment..."

# Install libpq-dev if not present (needed for diesel/postgres)
if ! dpkg -l libpq-dev >/dev/null 2>&1; then
    echo "Installing libpq-dev..."
    apt-get update -qq && apt-get install -y -qq libpq-dev
fi

# Install diesel_cli if not present
if ! command -v diesel >/dev/null 2>&1; then
    echo "Installing diesel_cli..."
    cargo install diesel_cli --no-default-features --features postgres
fi

# Create test.env from example if it doesn't exist
if [ ! -f test.env ]; then
    echo "Creating test.env from test.env.example..."
    cp test.env.example test.env
    # Use port 5432 (system postgres) instead of 54321 (docker)
    sed -i 's/:54321/:5432/' test.env
fi

# Start postgres if not running
if ! pg_isready -q 2>/dev/null; then
    echo "Starting PostgreSQL..."
    service postgresql start || pg_ctlcluster 16 main start
    sleep 2
fi

# Create user and database if they don't exist
if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_roles WHERE rolname='ramekin'" | grep -q 1; then
    echo "Creating postgres user 'ramekin'..."
    sudo -u postgres psql -c "CREATE USER ramekin WITH PASSWORD 'ramekin' CREATEDB;"
fi

if ! sudo -u postgres psql -tAc "SELECT 1 FROM pg_database WHERE datname='ramekin_test'" | grep -q 1; then
    echo "Creating database 'ramekin_test'..."
    sudo -u postgres psql -c "CREATE DATABASE ramekin_test OWNER ramekin;"
fi

echo "Claude Code for Web environment ready"
