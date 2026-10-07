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

# Only skip after waiting: a caller that got the lock immediately asked for an
# install (possibly forced via make -W), so honor it.
if [ -n "$REPO_LOCK_WAITED" ] &&
    [ "$MARKER" -nt ramekin-ui/package.json ] &&
    [ "$MARKER" -nt ramekin-ui/package-lock.json ]; then
    exit 0
fi

# Subshell: the lock-release trap uses a path relative to the project root.
(cd ramekin-ui && npx --yes -p npm@latest npm ci --silent)
