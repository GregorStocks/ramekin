#!/bin/bash
set -e

BENCHMARK_FILE="${1:-benchmark-results.txt}"

echo "=== Build Performance Benchmark ===" | tee -a "$BENCHMARK_FILE"
echo "Date: $(date)" | tee -a "$BENCHMARK_FILE"
echo "Git commit: $(git rev-parse --short HEAD)" | tee -a "$BENCHMARK_FILE"
echo "" | tee -a "$BENCHMARK_FILE"

# Helper function to time commands
time_command() {
    local label="$1"
    shift
    echo "" | tee -a "$BENCHMARK_FILE"
    echo "[$label]" | tee -a "$BENCHMARK_FILE"
    START=$(date +%s)
    if "$@"; then
        END=$(date +%s)
        DURATION=$((END - START))
        echo "✓ Completed in ${DURATION}s" | tee -a "$BENCHMARK_FILE"
        return 0
    else
        END=$(date +%s)
        DURATION=$((END - START))
        echo "✗ Failed in ${DURATION}s" | tee -a "$BENCHMARK_FILE"
        return 1
    fi
}

# Scenario 1: Cold cache
echo "## Scenario 1: Cold Cache" | tee -a "$BENCHMARK_FILE"
echo "Cleaning everything..." | tee -a "$BENCHMARK_FILE"
make clean 2>&1 | tail -1 | tee -a "$BENCHMARK_FILE"
docker system prune -f --volumes 2>&1 | grep -E "^Total reclaimed" | tee -a "$BENCHMARK_FILE" || true
rm -rf .cache/

time_command "make test (cold cache)" make test

# Scenario 2: No changes
echo "" | tee -a "$BENCHMARK_FILE"
echo "## Scenario 2: No Changes (Incremental)" | tee -a "$BENCHMARK_FILE"
time_command "make test (no changes, 1st run)" make test
time_command "make test (no changes, 2nd run)" make test
time_command "make test (no changes, 3rd run)" make test

# Scenario 3: Trivial change (doesn't affect OpenAPI)
echo "" | tee -a "$BENCHMARK_FILE"
echo "## Scenario 3: Trivial Change" | tee -a "$BENCHMARK_FILE"
echo "Making trivial change to server/src/main.rs..." | tee -a "$BENCHMARK_FILE"
echo "// Benchmark timestamp: $(date +%s)" >> server/src/main.rs
time_command "make test (after trivial change)" make test
git checkout server/src/main.rs

# Scenario 4: OpenAPI change
echo "" | tee -a "$BENCHMARK_FILE"
echo "## Scenario 4: OpenAPI Change" | tee -a "$BENCHMARK_FILE"
echo "Making change to API endpoint..." | tee -a "$BENCHMARK_FILE"
# Add comment to an API file
echo "// Benchmark timestamp: $(date +%s)" >> server/src/api/testing/mod.rs
time_command "make test (after API change)" make test
git checkout server/src/api/testing/mod.rs

echo "" | tee -a "$BENCHMARK_FILE"
echo "=== Benchmark Complete ===" | tee -a "$BENCHMARK_FILE"
echo "Results saved to: $BENCHMARK_FILE" | tee -a "$BENCHMARK_FILE"
