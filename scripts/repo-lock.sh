#!/bin/bash

repo_lock_timestamp() {
    date +%H:%M:%S
}

release_repo_lock() {
    if [ -n "${REPO_LOCK_DIR_ACTIVE:-}" ] && [ -d "$REPO_LOCK_DIR_ACTIVE" ]; then
        rm -rf "$REPO_LOCK_DIR_ACTIVE"
    fi
}

acquire_repo_lock() {
    local lock_name="$1"
    local human_name="$2"
    local lock_root="${REPO_LOCK_DIR:-logs/locks}"
    local lock_dir="${lock_root}/${lock_name}.lock"
    local lock_pid=

    mkdir -p "$lock_root"

    while ! mkdir "$lock_dir" 2>/dev/null; do
        if [ -f "$lock_dir/pid" ]; then
            lock_pid=$(tr -d '[:space:]' < "$lock_dir/pid")
        else
            lock_pid=""
        fi

        if [ -n "$lock_pid" ] && kill -0 "$lock_pid" 2>/dev/null; then
            echo "[$(repo_lock_timestamp)] Refusing to start ${human_name}: another ${human_name} is already running (pid ${lock_pid})." >&2
            echo "[$(repo_lock_timestamp)] Active lock: ${lock_dir}" >&2
            return 1
        fi

        echo "[$(repo_lock_timestamp)] Removing stale ${human_name} lock: ${lock_dir}" >&2
        rm -rf "$lock_dir"
    done

    printf '%s\n' "$$" > "$lock_dir/pid"
    printf '%s\n' "${PWD}" > "$lock_dir/pwd"
    REPO_LOCK_DIR_ACTIVE="$lock_dir"
    trap release_repo_lock EXIT INT TERM
}
