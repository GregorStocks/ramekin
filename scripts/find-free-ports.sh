#!/bin/bash
# Finds N free ports and outputs them as environment variable assignments
# Usage: eval $(./scripts/find-free-ports.sh PORT UI_PORT MOCK_OPENROUTER_PORT PROCESS_COMPOSE_PORT)

set -e

for var_name in "$@"; do
    # Use Python to find a free port (binds to port 0, gets assigned port, closes)
    port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("", 0)); print(s.getsockname()[1]); s.close()')
    echo "export ${var_name}=${port}"
done
