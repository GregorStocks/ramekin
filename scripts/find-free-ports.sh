#!/bin/bash
# Finds N free ports and outputs them as environment variable assignments
# Usage: eval $(./scripts/find-free-ports.sh PORT UI_PORT MOCK_OPENROUTER_PORT PROCESS_COMPOSE_PORT)

set -e

# Find all ports at once, keeping sockets open until all are found to avoid duplicates
python3 -c "
import socket
import sys

var_names = sys.argv[1:]
sockets = []
ports = []

# Bind all sockets first (keeps ports reserved)
for _ in var_names:
    s = socket.socket()
    s.bind(('', 0))
    sockets.append(s)
    ports.append(s.getsockname()[1])

# Close sockets (ports are now free but we have unique values)
for s in sockets:
    s.close()

# Print assignments
for name, port in zip(var_names, ports):
    print(f'export {name}={port}')
" "$@"
