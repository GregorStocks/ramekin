#!/bin/bash
set -e

cd "$(dirname "$0")/.."

# Source test.env to get PROCESS_COMPOSE_PORT
set -a
# shellcheck source=/dev/null
source test.env
set +a

process-compose up -e test.env -f test-ui-compose.yaml -t=false --port "$PROCESS_COMPOSE_PORT"
