#!/bin/bash
# Check for required development and test dependencies

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

MISSING=()

# Check Rust toolchain
command -v cargo >/dev/null 2>&1 || MISSING+=("cargo (install Rust: https://rustup.rs)")
command -v diesel >/dev/null 2>&1 || MISSING+=("diesel CLI (cargo install diesel_cli --no-default-features --features postgres)")

# Check cargo-watch for dev (not needed in CI)
if [ -z "$CI" ]; then
    if ! cargo watch --version >/dev/null 2>&1; then
        MISSING+=("cargo-watch (cargo install cargo-watch)")
    fi
fi

# Check Node.js
command -v npm >/dev/null 2>&1 || MISSING+=("npm (install Node.js)")

# Check process-compose for process management (not needed in CI)
if [ -z "$CI" ]; then
    command -v process-compose >/dev/null 2>&1 || MISSING+=("process-compose (brew install process-compose or see https://github.com/F1bonacc1/process-compose)")
fi

# Check Python
command -v python3 >/dev/null 2>&1 || MISSING+=("python3")

# Check Python packages
python3 -c "import pytest" 2>/dev/null || MISSING+=("pytest (uv pip install pytest)")
python3 -c "import requests" 2>/dev/null || MISSING+=("requests (uv pip install requests)")

# Check env files (not needed in CI, created explicitly there)
if [ -z "$CI" ]; then
    if [ ! -f "$PROJECT_ROOT/dev.env" ]; then
        MISSING+=("dev.env file (copy from dev.env.example)")
    fi
    if [ ! -f "$PROJECT_ROOT/test.env" ]; then
        MISSING+=("test.env file (copy from test.env.example)")
    fi
fi

# Check postgres connection using test.env
if [ -f "$PROJECT_ROOT/test.env" ]; then
    # shellcheck source=/dev/null
    source "$PROJECT_ROOT/test.env"
    if [ -n "$DATABASE_URL" ]; then
        if ! pg_isready -d "$DATABASE_URL" >/dev/null 2>&1; then
            MISSING+=("postgres not reachable (run: make db-up)")
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
