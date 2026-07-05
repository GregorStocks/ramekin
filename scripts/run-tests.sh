#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# shellcheck source=/dev/null
source ./scripts/repo-lock.sh
test_lock_name="${TEST_LOCK_NAME:-top-level-run}"
test_lock_owner_name="test run"
if [ -z "${TEST_LOCK_NAME:-}" ]; then
  test_lock_owner_name="top-level run"
fi
acquire_repo_lock "$test_lock_name" "test run" "$test_lock_owner_name"

ENV_FILE="${TEST_ENV_FILE:-test.env}"
ORIG_PROCESS_COMPOSE_PORT="${PROCESS_COMPOSE_PORT:-}"
TEST_LOG_FILE="${TEST_LOG_FILE:-logs/test.log}"
STATUS_DIR="${TEST_STATUS_DIR:-logs/test-status}"
STATUS_WAIT_SECONDS="${TEST_STATUS_WAIT_SECONDS:-120}"
TARGET_PROCESSES=(
  rust-tests-server
  rust-tests-cli
  rust-tests-core
  api-tests
  ui-unit-tests
)

# Source env file to get PROCESS_COMPOSE_PORT
set -a
# shellcheck source=/dev/null
source "$ENV_FILE"
set +a

if [ -n "$ORIG_PROCESS_COMPOSE_PORT" ]; then
  export PROCESS_COMPOSE_PORT="$ORIG_PROCESS_COMPOSE_PORT"
fi
export TEST_STATUS_DIR="$STATUS_DIR"

PROCESS_COMPOSE_STARTED=0
PROCESS_COMPOSE_PID=""

trap 'EXIT_CODE=$?; set +e; if [ "$PROCESS_COMPOSE_STARTED" -eq 1 ]; then process-compose down --port "$PROCESS_COMPOSE_PORT" >/dev/null 2>&1 || true; if [ -n "$PROCESS_COMPOSE_PID" ]; then wait "$PROCESS_COMPOSE_PID" >/dev/null 2>&1 || true; fi; fi; release_repo_lock; exit "$EXIT_CODE"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

print_failure_logs() {
  if [ -f "$TEST_LOG_FILE" ]; then
    echo "[$(date +%H:%M:%S)] Test orchestration failed. Last 200 lines of ${TEST_LOG_FILE}:"
    tail -n 200 "$TEST_LOG_FILE"
  else
    echo "[$(date +%H:%M:%S)] Test orchestration failed. ${TEST_LOG_FILE} not found."
    ls -la "$(dirname "$TEST_LOG_FILE")" || true
  fi
}

collect_failed_processes() {
  local process_name
  local status_file
  local process_exit_code

  FAILED_PROCESSES=()
  for process_name in "${TARGET_PROCESSES[@]}"; do
    status_file="${STATUS_DIR}/${process_name}.exit"
    if [ ! -f "$status_file" ]; then
      FAILED_PROCESSES+=("${process_name} (status=missing)")
      continue
    fi

    process_exit_code=$(tr -d '[:space:]' < "$status_file")
    if [ "$process_exit_code" != "0" ]; then
      FAILED_PROCESSES+=("${process_name} (exit_code=${process_exit_code})")
    fi
  done
}

wait_for_status_files() {
  local deadline
  local now
  local process_name
  local status_file
  local missing

  deadline=$((SECONDS + STATUS_WAIT_SECONDS))

  while [ "$SECONDS" -lt "$deadline" ]; do
    missing=0
    for process_name in "${TARGET_PROCESSES[@]}"; do
      status_file="${STATUS_DIR}/${process_name}.exit"
      if [ ! -f "$status_file" ]; then
        missing=1
        break
      fi
    done

    if [ "$missing" -eq 0 ]; then
      return 0
    fi

    sleep 1
    now=$(date +%H:%M:%S)
    echo "[$now] Waiting for test status files to flush..."
  done
}

echo "[$(date +%H:%M:%S)] Starting test orchestration via process-compose"
START_TIME=$(date +%s)

mkdir -p "$(dirname "$TEST_LOG_FILE")"
rm -rf "$STATUS_DIR"
mkdir -p "$STATUS_DIR"

# Prefer prebuilt server binary so readiness probes do not race a cold release build.
if [ -x "./server/target/release/ramekin-server" ]; then
  export SERVER_CMD="./target/release/ramekin-server"
fi

set +e
PROCESS_COMPOSE_STARTED=1
process-compose up -e "$ENV_FILE" -f test-compose.yaml -t=false --port "$PROCESS_COMPOSE_PORT" &
PROCESS_COMPOSE_PID=$!
wait "$PROCESS_COMPOSE_PID"
EXIT_CODE=$?
PROCESS_COMPOSE_STARTED=0
PROCESS_COMPOSE_PID=""
set -e

if [ $EXIT_CODE -ne 0 ]; then
  print_failure_logs
fi

if [ $EXIT_CODE -eq 0 ]; then
  wait_for_status_files
  collect_failed_processes
  if [ ${#FAILED_PROCESSES[@]} -ne 0 ]; then
    EXIT_CODE=1
    echo "[$(date +%H:%M:%S)] One or more test processes failed:"
    printf '  - %s\n' "${FAILED_PROCESSES[@]}"
    print_failure_logs
  fi
fi

ELAPSED=$(($(date +%s) - START_TIME))
echo "[$(date +%H:%M:%S)] Test orchestration completed in ${ELAPSED}s"

exit $EXIT_CODE
