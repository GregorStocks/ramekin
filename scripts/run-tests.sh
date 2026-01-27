#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# Source test.env to get PROCESS_COMPOSE_PORT
set -a
# shellcheck source=/dev/null
source test.env
set +a

echo "[$(date +%H:%M:%S)] Starting test orchestration via process-compose"
START_TIME=$(date +%s)

process-compose up -e test.env -f test-compose.yaml -t=false --port "$PROCESS_COMPOSE_PORT"
EXIT_CODE=$?

ELAPSED=$(($(date +%s) - START_TIME))
echo "[$(date +%H:%M:%S)] Test orchestration completed in ${ELAPSED}s"

exit $EXIT_CODE
