#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# shellcheck source=/dev/null
source ./scripts/repo-lock.sh
acquire_repo_lock "${PIPELINE_LOCK_NAME:-pipeline}" "pipeline run"

echo "[pipeline] cargo run starting" | ./scripts/ts

set -a
# shellcheck source=/dev/null
[ -f cli.env ] && . ./cli.env
set +a

time cargo run -q --release --manifest-path cli/Cargo.toml -- pipeline "$@"

echo "[pipeline] cargo run done, running ingredient-tests-generate" | ./scripts/ts
time make ingredient-tests-generate
echo "[pipeline] ingredient-tests-generate done" | ./scripts/ts
