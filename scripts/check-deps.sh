#!/bin/bash
# Check for required development and test dependencies.
#
# Usage: check-deps.sh [--lint]
#   --lint  Only check the tools needed by `make lint`.
#
# Reports every missing dependency with install instructions for the
# current platform (macOS, Fedora, Arch, or generic Linux). Never
# installs anything itself.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

LINT_ONLY=false
if [ "$1" = "--lint" ]; then
    LINT_ONLY=true
fi

# Keep in sync with .github/actions/setup/action.yml
SWIFTLINT_VERSION=0.63.2

case "$(uname -s)" in
    Darwin)
        PLATFORM=macos
        ;;
    *)
        ID="" ID_LIKE=""
        if [ -r /etc/os-release ]; then
            # shellcheck source=/dev/null
            . /etc/os-release
        fi
        case "$ID $ID_LIKE" in
            *fedora* | *rhel*) PLATFORM=fedora ;;
            *arch*) PLATFORM=arch ;;
            *) PLATFORM=linux ;;
        esac
        ;;
esac

MISSING=()

# hint <macos> <fedora> <arch> <generic-linux>
# Echoes the install instruction for the current platform.
hint() {
    case "$PLATFORM" in
        macos) echo "$1" ;;
        fedora) echo "$2" ;;
        arch) echo "$3" ;;
        *) echo "$4" ;;
    esac
}

# require <command> <install-instructions>
require() {
    command -v "$1" >/dev/null 2>&1 || MISSING+=("$1 ($2)")
}

SWIFTLINT_LINUX="curl -fsSL https://github.com/realm/SwiftLint/releases/download/${SWIFTLINT_VERSION}/swiftlint_linux_amd64.zip -o /tmp/swiftlint.zip && unzip -o /tmp/swiftlint.zip -d /tmp/swiftlint && install -D -m 0755 /tmp/swiftlint/swiftlint ~/.local/bin/swiftlint"

# --- Tools needed by both `make lint` and the dev/test environment ---

require cargo "install Rust: https://rustup.rs"
require npm "$(hint \
    "brew install node" \
    "sudo dnf install nodejs npm" \
    "sudo pacman -S nodejs npm" \
    "install Node.js 22+: https://nodejs.org")"
require uv "$(hint \
    "brew install uv" \
    "sudo dnf install uv" \
    "sudo pacman -S uv" \
    "curl -LsSf https://astral.sh/uv/install.sh | sh")"
require python3 "$(hint \
    "brew install python" \
    "sudo dnf install python3" \
    "sudo pacman -S python" \
    "install Python 3 via your package manager")"
require ast-grep "cargo install ast-grep"

# --- Tools needed only by `make lint` ---

if $LINT_ONLY; then
    require shellcheck "$(hint \
        "brew install shellcheck" \
        "sudo dnf install ShellCheck" \
        "sudo pacman -S shellcheck" \
        "sudo apt-get install shellcheck")"
    require swiftlint "$(hint \
        "brew install swiftlint" \
        "$SWIFTLINT_LINUX" \
        "$SWIFTLINT_LINUX" \
        "$SWIFTLINT_LINUX")"
fi

# --- Everything else (dev/test environment) ---

if ! $LINT_ONLY; then
    require diesel "cargo install diesel_cli --no-default-features --features postgres"

    # cargo-watch for dev (not needed in CI)
    if [ -z "$CI" ]; then
        if ! cargo watch --version >/dev/null 2>&1; then
            MISSING+=("cargo-watch (cargo install cargo-watch)")
        fi
    fi

    require mkcert "$(hint \
        "brew install mkcert && mkcert -install" \
        "sudo dnf install mkcert && mkcert -install" \
        "sudo pacman -S mkcert && mkcert -install" \
        "see https://github.com/FiloSottile/mkcert, then mkcert -install")"

    # process-compose for process management (not needed in CI)
    if [ -z "$CI" ]; then
        require process-compose "$(hint \
            "brew install process-compose" \
            "download from https://github.com/F1bonacc1/process-compose/releases into ~/.local/bin" \
            "install process-compose-bin from the AUR" \
            "download from https://github.com/F1bonacc1/process-compose/releases into ~/.local/bin")"
    fi

    # Python packages (installed into .venv by `make venv`)
    python3 -c "import pytest" 2>/dev/null || MISSING+=("pytest (run: make venv)")
    python3 -c "import requests" 2>/dev/null || MISSING+=("requests (run: make venv)")

    # Env files (not needed in CI, created explicitly there)
    if [ -z "$CI" ]; then
        if [ ! -f "$PROJECT_ROOT/dev.env" ]; then
            MISSING+=("dev.env file (run: make worktree-setup)")
        fi
        if [ ! -f "$PROJECT_ROOT/test.env" ]; then
            MISSING+=("test.env file (run: make worktree-setup)")
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
fi

if [ ${#MISSING[@]} -ne 0 ]; then
    echo "Missing dependencies:"
    for dep in "${MISSING[@]}"; do
        echo "  - $dep"
    done
    exit 1
fi
