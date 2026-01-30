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

process-compose up -e "$ENV_FILE" -f test-ui-compose.yaml -t=false --port "$PROCESS_COMPOSE_PORT"
EXIT_CODE=$?

ELAPSED=$(($(date +%s) - START_TIME))
echo "[$(date +%H:%M:%S)] UI test orchestration completed in ${ELAPSED}s"

exit $EXIT_CODE
