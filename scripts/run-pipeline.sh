#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# shellcheck source=/dev/null
source ./scripts/repo-lock.sh
pipeline_lock_name="${PIPELINE_LOCK_NAME:-top-level-run}"
pipeline_lock_owner_name="pipeline run"
if [ -z "${PIPELINE_LOCK_NAME:-}" ]; then
  pipeline_lock_owner_name="top-level run"
fi
acquire_repo_lock "$pipeline_lock_name" "pipeline run" "$pipeline_lock_owner_name"

echo "[pipeline] cargo run starting" | ./scripts/ts

set -a
# shellcheck source=/dev/null
[ -f cli.env ] && . ./cli.env
set +a

time cargo run -q --release --manifest-path cli/Cargo.toml -- pipeline "$@"

echo "[pipeline] cargo run done, running ingredient-tests-generate" | ./scripts/ts
time make ingredient-tests-generate
echo "[pipeline] ingredient-tests-generate done" | ./scripts/ts
