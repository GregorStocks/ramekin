#!/bin/bash
# Check for required test dependencies

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

MISSING=()

# Check commands exist
command -v cargo >/dev/null 2>&1 || MISSING+=("cargo (install Rust: https://rustup.rs)")
command -v diesel >/dev/null 2>&1 || MISSING+=("diesel CLI (cargo install diesel_cli --no-default-features --features postgres)")
command -v python3 >/dev/null 2>&1 || MISSING+=("python3")
command -v curl >/dev/null 2>&1 || MISSING+=("curl")

# Check Python packages
python3 -c "import pytest" 2>/dev/null || MISSING+=("pytest (uv pip install pytest)")
python3 -c "import requests" 2>/dev/null || MISSING+=("requests (uv pip install requests)")

# Check test.env exists
if [ ! -f "$PROJECT_ROOT/test.env" ]; then
    MISSING+=("test.env file (copy from test.env.example)")
fi

# Check postgres connection if test.env exists
if [ -f "$PROJECT_ROOT/test.env" ]; then
    source "$PROJECT_ROOT/test.env"
    if [ -n "$DATABASE_URL" ]; then
        if ! pg_isready -d "$DATABASE_URL" >/dev/null 2>&1; then
            MISSING+=("postgres not reachable at DATABASE_URL")
        fi
    fi
fi

if [ ${#MISSING[@]} -ne 0 ]; then
    echo "Missing dependencies:"
    for dep in "${MISSING[@]}"; do
        echo "  - $dep"
    done
    exit 1
fi
