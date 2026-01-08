#!/bin/bash
set -e

cd "$(dirname "$0")/.."
process-compose up -e test.env -f test-compose.yaml -t=false
