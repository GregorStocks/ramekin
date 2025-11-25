#!/bin/bash
set -e

# Generate all API clients from saved OpenAPI spec
# Called by `make generate-clients` after generating api/openapi.json

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SPEC_FILE="$PROJECT_ROOT/api/openapi.json"

generate_client() {
    local generator=$1
    local output=$2
    local extra_props=$3

    echo "Generating $generator client -> $output"

    rm -rf "$PROJECT_ROOT/$output"

    docker run --rm \
        -v "$PROJECT_ROOT:/project" \
        -w /project \
        openapitools/openapi-generator-cli:v7.10.0 generate \
        -i "/project/api/openapi.json" \
        -g "$generator" \
        -o "/project/$output" \
        --additional-properties="$extra_props" \
        2>&1 | grep -v "^\[main\] INFO"
}

main() {
    if [ ! -f "$SPEC_FILE" ]; then
        echo "Error: OpenAPI spec not found at $SPEC_FILE" >&2
        echo "Please run: make fetch-openapi" >&2
        exit 1
    fi

    echo "Generating API clients from $SPEC_FILE..."

    # Generate Rust client (for CLI)
    generate_client "rust" "crates/generated/ramekin-client" "packageName=ramekin_client,supportAsync=true"

    # Add lint exceptions for generated Rust code
    cat >> "$PROJECT_ROOT/crates/generated/ramekin-client/Cargo.toml" << 'EOF'

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
    (cd "$PROJECT_ROOT/ramekin-ui" && npx tsc -p tsconfig.generated-client.json)

    # Generate Python client (for tests)
    generate_client "python" "tests/generated" "packageName=ramekin_client,generateSourceCodeOnly=true"

    echo ""
    echo "All clients generated successfully:"
    echo "  - Rust:       crates/generated/ramekin-client/"
    echo "  - TypeScript: ramekin-ui/generated-client/"
    echo "  - Python:     tests/generated/"
}

LOG_DIR="$PROJECT_ROOT/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/generate-clients.log"
main > "$LOG_FILE" 2>&1
echo "Generated clients, log at $LOG_FILE"
