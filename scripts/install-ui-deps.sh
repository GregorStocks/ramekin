#!/bin/bash
set -euo pipefail

# The one way to install the UI's node_modules. Pins npm so every install path
# resolves package-lock.json the same way. Skips an install that is already
# current; UI_DEPS_FORCE=1 reinstalls anyway (e.g. to repair a corrupted
# node_modules).

cd "$(dirname "$0")/.."

MARKER=ramekin-ui/node_modules/.package-lock.json

if [ -z "${UI_DEPS_FORCE:-}" ] &&
    [ "$MARKER" -nt ramekin-ui/package.json ] &&
    [ "$MARKER" -nt ramekin-ui/package-lock.json ]; then
    exit 0
fi

cd ramekin-ui
npx --yes -p npm@11.6.2 npm ci --silent
