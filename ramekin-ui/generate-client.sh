#!/bin/bash
set -e

# Generate TypeScript client from OpenAPI spec
# The server must be running at localhost:3000

echo "Generating TypeScript client from OpenAPI spec..."

# Check if server is running
if ! curl -s http://localhost:3000/api-docs/openapi.json > /dev/null; then
    echo "Error: Server is not running at localhost:3000"
    echo "Please start the server first: make dev"
    exit 1
fi

# Download the OpenAPI spec
curl -s http://localhost:3000/api-docs/openapi.json > openapi.json

# Generate the TypeScript client using openapi-generator
npx @openapitools/openapi-generator-cli generate \
    -i openapi.json \
    -g typescript-fetch \
    -o src/generated/api \
    --additional-properties=supportsES6=true,typescriptThreePlus=true

# Clean up
rm openapi.json

echo "Client generated successfully in src/generated/api/"
echo "The generated client is checked into git for easy building"
