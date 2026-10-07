#!/bin/bash
set -euo pipefail

# Install the lockfile-pinned UI toolchain. Concurrent make targets (e.g. lint
# and test in a fresh worktree) all depend on this, and parallel `npm ci` runs
# corrupt the shared node_modules, so installs take turns.

cd "$(dirname "$0")/.."

# shellcheck source=/dev/null
source ./scripts/repo-lock.sh

MARKER=ramekin-ui/node_modules/.package-lock.json

REPO_LOCK_WAIT=1 acquire_repo_lock ui-deps "UI dependency install"

# Make decided to install before taking the lock, so another install may have
# finished since; rerunning npm ci would delete node_modules out from under
# whoever is using it. UI_DEPS_FORCE=1 reinstalls anyway (e.g. to repair a
# corrupted node_modules).
if [ -z "${UI_DEPS_FORCE:-}" ] &&
    [ "$MARKER" -nt ramekin-ui/package.json ] &&
    [ "$MARKER" -nt ramekin-ui/package-lock.json ]; then
    exit 0
fi

# Subshell: the lock-release trap uses a path relative to the project root.
(cd ramekin-ui && npx --yes -p npm@latest npm ci --silent)
