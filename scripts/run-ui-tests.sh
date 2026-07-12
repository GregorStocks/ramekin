#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# shellcheck source=/dev/null
source ./scripts/test-orchestration.sh

# Use TEST_ENV_FILE if set, otherwise default to test.env
ENV_FILE="${TEST_ENV_FILE:-test.env}"
ORIG_PROCESS_COMPOSE_PORT="${PROCESS_COMPOSE_PORT:-}"

# Source env file to get PROCESS_COMPOSE_PORT
set -a
# shellcheck source=/dev/null
source "$ENV_FILE"
set +a

if [ -n "$ORIG_PROCESS_COMPOSE_PORT" ]; then
  export PROCESS_COMPOSE_PORT="$ORIG_PROCESS_COMPOSE_PORT"
fi

if [ -z "${UI_PORT_HTTP:-}" ]; then
  export UI_PORT_HTTP=$((UI_PORT + 1))
fi

assert_test_ports_available \
  "API server" "${PORT:-}" \
  "fixture server" "${FIXTURE_PORT:-}" \
  "mock OpenRouter" "${MOCK_OPENROUTER_PORT:-}" \
  "UI server" "${UI_PORT:-}" \
  "UI HTTP proxy" "${UI_PORT_HTTP:-}" \
  "process-compose control" "${PROCESS_COMPOSE_PORT:-}"

PROCESS_COMPOSE_STARTED=0
PROCESS_COMPOSE_PID=""

trap 'EXIT_CODE=$?; set +e; stop_test_orchestration "$PROCESS_COMPOSE_STARTED" "$PROCESS_COMPOSE_PID" "$PROCESS_COMPOSE_PORT"; exit "$EXIT_CODE"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Ensure logs directory exists for process-compose output
mkdir -p logs

# Prefer prebuilt server binary if available to speed startup
if [ -x "./server/target/release/ramekin-server" ]; then
  export SERVER_CMD="./target/release/ramekin-server"
fi

echo "[$(date +%H:%M:%S)] Starting UI test orchestration via process-compose"
START_TIME=$(date +%s)

echo "[$(date +%H:%M:%S)] Ensuring Playwright browsers are installed"
if [ "${CI:-}" = "true" ] && [ "$(uname -s)" = "Linux" ]; then
  playwright install chromium --with-deps
else
  playwright install chromium
fi

set +e
PROCESS_COMPOSE_STARTED=1
process-compose up -e "$ENV_FILE" -f test-ui-compose.yaml -t=false --port "$PROCESS_COMPOSE_PORT" &
PROCESS_COMPOSE_PID=$!
wait "$PROCESS_COMPOSE_PID"
EXIT_CODE=$?
stop_test_orchestration "$PROCESS_COMPOSE_STARTED" "$PROCESS_COMPOSE_PID" "$PROCESS_COMPOSE_PORT"
PROCESS_COMPOSE_STARTED=0
PROCESS_COMPOSE_PID=""
set -e
if [ $EXIT_CODE -ne 0 ] && [ -f logs/test-ui.log ]; then
  echo "[$(date +%H:%M:%S)] UI test orchestration failed. Last 200 lines of logs/test-ui.log:"
  tail -n 200 logs/test-ui.log
elif [ $EXIT_CODE -ne 0 ]; then
  echo "[$(date +%H:%M:%S)] UI test orchestration failed. logs/test-ui.log not found."
  ls -la logs || true
fi

ELAPSED=$(($(date +%s) - START_TIME))
echo "[$(date +%H:%M:%S)] UI test orchestration completed in ${ELAPSED}s"

exit $EXIT_CODE
