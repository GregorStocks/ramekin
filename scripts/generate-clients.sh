#!/bin/bash
set -e

# Generate all API clients from saved OpenAPI spec
# Called by `make generate-clients` after generating api/openapi.json

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SPEC_FILE="$PROJECT_ROOT/api/openapi.json"

# OpenAPI Generator configuration
OPENAPI_GENERATOR_VERSION="7.10.0"
OPENAPI_GENERATOR_JAR="$PROJECT_ROOT/.cache/openapi-generator-cli-$OPENAPI_GENERATOR_VERSION.jar"
OPENAPI_GENERATOR_URL="https://repo1.maven.org/maven2/org/openapitools/openapi-generator-cli/$OPENAPI_GENERATOR_VERSION/openapi-generator-cli-$OPENAPI_GENERATOR_VERSION.jar"

ensure_openapi_generator() {
    if [ ! -f "$OPENAPI_GENERATOR_JAR" ]; then
        echo "Downloading OpenAPI Generator CLI..."
        mkdir -p "$(dirname "$OPENAPI_GENERATOR_JAR")"
        curl -L -o "$OPENAPI_GENERATOR_JAR" "$OPENAPI_GENERATOR_URL"
    fi
}

generate_client() {
    local generator=$1
    local output=$2
    local extra_props=$3

    echo "Generating $generator client -> $output"

    rm -rf "$PROJECT_ROOT/$output"

    java -jar "$OPENAPI_GENERATOR_JAR" generate \
        -i "$SPEC_FILE" \
        -g "$generator" \
        -o "$PROJECT_ROOT/$output" \
        --additional-properties="$extra_props" \
        2>&1 | grep -v "^\[main\] INFO" || true
}

main() {
    if [ ! -f "$SPEC_FILE" ]; then
        echo "Error: OpenAPI spec not found at $SPEC_FILE" >&2
        echo "Please run: make generate-clients" >&2
        exit 1
    fi

    ensure_openapi_generator

    echo "Generating API clients from $SPEC_FILE..."

    # Generate Rust client (for CLI)
    generate_client "rust" "cli/generated/ramekin-client" "packageName=ramekin_client,supportAsync=true"

    # Add lint exceptions for generated Rust code
    cat >> "$PROJECT_ROOT/cli/generated/ramekin-client/Cargo.toml" << 'EOF'

[lints.rust]
unused_variables = "allow"
unused_mut = "allow"
EOF

    # Generate TypeScript client (for UI)
    generate_client "typescript-fetch" "ramekin-ui/generated-client" "supportsES6=true,typescriptThreePlus=true"

    # Create package.json for generated client (pointing to dist)
    cat > "$PROJECT_ROOT/ramekin-ui/generated-client/package.json" << 'EOF'
{
  "name": "ramekin-client",
  "version": "0.0.0",
  "main": "./dist/index.js",
  "types": "./dist/index.d.ts"
}
EOF

    # Compile TypeScript client to JS + .d.ts
    echo "Compiling TypeScript client..."
    (cd "$PROJECT_ROOT/ramekin-ui" && npx --yes -p typescript tsc -p tsconfig.generated-client.json)

    # Generate Python client (for tests)
    generate_client "python" "tests/generated" "packageName=ramekin_client,generateSourceCodeOnly=true"

    echo ""
    echo "All clients generated successfully:"
    echo "  - Rust:       cli/generated/ramekin-client/"
    echo "  - TypeScript: ramekin-ui/generated-client/"
    echo "  - Python:     tests/generated/"
}

LOG_DIR="$PROJECT_ROOT/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/generate-clients.log"
main > "$LOG_FILE" 2>&1
echo "Generated clients, log at $LOG_FILE"
