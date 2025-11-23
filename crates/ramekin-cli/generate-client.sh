#!/bin/bash
set -e

# Generate Rust client from OpenAPI spec
# The server must be running at localhost:3000

echo "Generating Rust client from OpenAPI spec..."

# Check if server is running
if ! curl -s http://localhost:3000/api-docs/openapi.json > /dev/null; then
    echo "Error: Server is not running at localhost:3000"
    echo "Please start the server first: docker compose up"
    exit 1
fi

# Download the OpenAPI spec
curl -s http://localhost:3000/api-docs/openapi.json > openapi.json

# Generate the Rust client using openapi-generator
# Install with: npm install -g @openapitools/openapi-generator-cli
npx @openapitools/openapi-generator-cli generate \
    -i openapi.json \
    -g rust \
    -o src/generated \
    --additional-properties=packageName=ramekin_client,supportAsync=true

# Clean up
rm openapi.json

echo "Client generated successfully in src/generated/"
echo "You may need to add dependencies to Cargo.toml based on the generated client"
