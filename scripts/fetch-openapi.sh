#!/bin/bash
set -e

# Download OpenAPI spec from running server and save to committed file
# Requires the dev server to be running at localhost:3000

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SERVER_URL="${API_URL:-http://localhost:3000}"
SPEC_FILE="$PROJECT_ROOT/api/openapi.json"

echo "Fetching OpenAPI spec from $SERVER_URL..."

# Check if server is running
if ! curl -s "$SERVER_URL/api-docs/openapi.json" > /dev/null 2>&1; then
    echo "Error: Server is not running at $SERVER_URL"
    echo "Please start the server first: make dev"
    exit 1
fi

# Create api directory if it doesn't exist
mkdir -p "$(dirname "$SPEC_FILE")"

# Download the OpenAPI spec
curl -s "$SERVER_URL/api-docs/openapi.json" | jq '.' > "$SPEC_FILE"
echo "Saved OpenAPI spec to $SPEC_FILE"
