#!/bin/bash
set -e

# Generate all API clients from OpenAPI spec
# Requires the dev server to be running at localhost:3000

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SERVER_URL="${API_URL:-http://localhost:3000}"

# Create temp directory for spec file (cleaned up on exit)
TMPDIR=$(mktemp -d)
trap "rm -rf $TMPDIR" EXIT
SPEC_FILE="$TMPDIR/openapi.json"

echo "Generating API clients from $SERVER_URL..."

# Check if server is running
if ! curl -s "$SERVER_URL/api-docs/openapi.json" > /dev/null 2>&1; then
    echo "Error: Server is not running at $SERVER_URL"
    echo "Please start the server first: make dev"
    exit 1
fi

# Download the OpenAPI spec
curl -s "$SERVER_URL/api-docs/openapi.json" > "$SPEC_FILE"
echo "Downloaded OpenAPI spec"

# Function to run openapi-generator via Docker
generate() {
    local generator=$1
    local output=$2
    local extra_props=$3

    echo "Generating $generator client -> $output"

    docker run --rm \
        -v "$PROJECT_ROOT:/project" \
        -v "$TMPDIR:/spec:ro" \
        -w /project \
        openapitools/openapi-generator-cli:v7.10.0 generate \
        -i /spec/openapi.json \
        -g "$generator" \
        -o "/project/$output" \
        --additional-properties="$extra_props"
}

# Generate Rust client (for CLI)
generate "rust" "crates/ramekin-client" "packageName=ramekin_client,supportAsync=true"

# Generate TypeScript client (for UI)
generate "typescript-fetch" "ramekin-ui/src/generated" "supportsES6=true,typescriptThreePlus=true"

# Generate Python client (for tests)
generate "python" "tests/generated" "packageName=ramekin_client,generateSourceCodeOnly=true"

echo ""
echo "All clients generated successfully:"
echo "  - Rust:       crates/ramekin-client/"
echo "  - TypeScript: ramekin-ui/src/generated/"
echo "  - Python:     tests/generated/"
