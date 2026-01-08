#!/bin/bash
set -e

cd "$(dirname "$0")/.."
process-compose up -e test.env -f test-ui-compose.yaml -t=false
