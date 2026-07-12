#!/bin/bash

assert_test_ports_available() {
    local service_name
    local port

    while [ "$#" -gt 0 ]; do
        service_name="$1"
        port="$2"
        shift 2

        if [ -z "$port" ]; then
            continue
        fi

        if (: >/dev/tcp/127.0.0.1/"$port") 2>/dev/null; then
            echo "[$(date +%H:%M:%S)] Refusing to start test orchestration: ${service_name} port ${port} is already in use." >&2
            return 1
        fi
    done
}

stop_test_orchestration() {
    local started="${1:-0}"
    local process_compose_pid="${2:-}"
    local process_compose_port="${3:-}"

    if [ "$started" -ne 1 ]; then
        return
    fi

    process-compose down --port "$process_compose_port" >/dev/null 2>&1 || true
    if [ -n "$process_compose_pid" ]; then
        wait "$process_compose_pid" >/dev/null 2>&1 || true
    fi
}
