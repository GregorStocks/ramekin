#!/bin/bash
set -e

echo "Waiting for server to be ready..."

# Wait for server to respond
max_attempts=60
attempt=0
until curl -s http://server:3000/api/garbages > /dev/null 2>&1; do
    attempt=$((attempt + 1))
    if [ $attempt -ge $max_attempts ]; then
        echo "Server did not become ready in time"
        exit 1
    fi
    echo "Waiting for server... (attempt $attempt/$max_attempts)"
    sleep 2
done

echo "Server is ready!"

# Run the tests
echo "Running tests..."
pytest /app/tests -v
