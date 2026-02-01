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
    local final_path="$PROJECT_ROOT/$output"
    local temp_path="$PROJECT_ROOT/$output.generating"

    echo "Generating $generator client -> $output"

    # Clean up any leftover temp dir from previous failed run
    rm -rf "${temp_path:?}"

    # Generate to temp directory
    npx --yes @openapitools/openapi-generator-cli@2.27.0 generate \
        -i "$SPEC_FILE" \
        -g "$generator" \
        -o "$temp_path" \
        --additional-properties="$extra_props" \
        2>&1 | grep -v "^\[main\] INFO" || true

    # Atomic swap: remove old, move new into place
    rm -rf "${final_path:?}"
    mv "$temp_path" "$final_path"
}

main() {
    if [ ! -f "$SPEC_FILE" ]; then
        echo "Error: OpenAPI spec not found at $SPEC_FILE" >&2
        echo "Please run: make generate-clients" >&2
        exit 1
    fi

    echo "Generating API clients from $SPEC_FILE..."

    # Generate Rust client (for CLI)
    generate_client "rust" "cli/generated/ramekin-client" "packageName=ramekin_client,supportAsync=true,useAsyncFileStream=true"

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

    # Generate Swift client (for iOS app)
    generate_client "swift5" "ramekin-ios/generated-client" "projectName=RamekinClient,responseAs=AsyncAwait"

    echo ""
    echo "All clients generated successfully:"
    echo "  - Rust:       cli/generated/ramekin-client/"
    echo "  - TypeScript: ramekin-ui/generated-client/"
    echo "  - Python:     tests/generated/"
    echo "  - Swift:      ramekin-ios/generated-client/"
}

LOG_DIR="$PROJECT_ROOT/logs"
mkdir -p "$LOG_DIR"
LOG_FILE="$LOG_DIR/generate-clients.log"
main > "$LOG_FILE" 2>&1
echo "Generated clients, log at $LOG_FILE"
