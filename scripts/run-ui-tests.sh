#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# Use TEST_ENV_FILE if set, otherwise default to test.env
ENV_FILE="${TEST_ENV_FILE:-test.env}"

# Source env file to get PROCESS_COMPOSE_PORT
set -a
# shellcheck source=/dev/null
source "$ENV_FILE"
set +a

echo "[$(date +%H:%M:%S)] Starting UI test orchestration via process-compose"
START_TIME=$(date +%s)

echo "[$(date +%H:%M:%S)] Ensuring Playwright browsers are installed"
if [ "${CI:-}" = "true" ] && [ "$(uname -s)" = "Linux" ]; then
  playwright install chromium --with-deps
else
  playwright install chromium
fi

process-compose up -e "$ENV_FILE" -f test-ui-compose.yaml -t=false --port "$PROCESS_COMPOSE_PORT"
EXIT_CODE=$?
if [ $EXIT_CODE -ne 0 ] && [ -f logs/test-ui.log ]; then
  echo "[$(date +%H:%M:%S)] UI test orchestration failed. Last 200 lines of logs/test-ui.log:"
  tail -n 200 logs/test-ui.log
fi

ELAPSED=$(($(date +%s) - START_TIME))
echo "[$(date +%H:%M:%S)] UI test orchestration completed in ${ELAPSED}s"

exit $EXIT_CODE
