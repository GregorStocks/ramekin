#!/bin/bash

repo_lock_timestamp() {
    date +%H:%M:%S
}

repo_lock_mtime_epoch() {
    local path="$1"

    if stat -f %m "$path" >/dev/null 2>&1; then
        stat -f %m "$path"
        return 0
    fi

    stat -c %Y "$path"
}

release_repo_lock() {
    if [ -n "${REPO_LOCK_DIR_ACTIVE:-}" ] && [ -d "$REPO_LOCK_DIR_ACTIVE" ]; then
        rm -rf "$REPO_LOCK_DIR_ACTIVE"
    fi
}

# With REPO_LOCK_WAIT=1, wait for a live lock holder instead of refusing to
# start. Waiting is bounded by REPO_LOCK_WAIT_SECONDS because PID liveness can't
# tell a crashed holder from an unrelated process that reused its PID.
# Uses acquire_repo_lock's wait_started local (bash locals are dynamically scoped).
repo_lock_wait() {
    local human_name="$1"
    local lock_owner_name="$2"
    local lock_dir="$3"
    local wait_seconds="${REPO_LOCK_WAIT_SECONDS:-600}"

    [ -n "${REPO_LOCK_WAIT:-}" ] || return 1
    if [ -z "$wait_started" ]; then
        wait_started=$(date +%s)
        echo "[$(repo_lock_timestamp)] Waiting up to ${wait_seconds}s for another ${lock_owner_name} to finish before starting ${human_name}." >&2
        echo "[$(repo_lock_timestamp)] Active lock: ${lock_dir}" >&2
    elif [ $(($(date +%s) - wait_started)) -ge "$wait_seconds" ]; then
        return 1
    fi
    sleep 0.2
}

acquire_repo_lock() {
    local lock_name="$1"
    local human_name="$2"
    local lock_owner_name="${3:-$human_name}"
    local lock_root="${REPO_LOCK_DIR:-logs/locks}"
    local lock_dir="${lock_root}/${lock_name}.lock"
    local lock_grace_seconds="${REPO_LOCK_STARTUP_GRACE_SECONDS:-30}"
    local lock_pid=
    local lock_mtime=
    local now_epoch=
    local wait_started=

    mkdir -p "$lock_root"

    while ! mkdir "$lock_dir" 2>/dev/null; do
        if [ -f "$lock_dir/pid" ]; then
            lock_pid=$(tr -d '[:space:]' < "$lock_dir/pid")
            case "$lock_pid" in
                ''|*[!0-9]*) lock_pid="" ;;
            esac
        else
            lock_pid=""
        fi

        if [ -z "$lock_pid" ]; then
            lock_mtime=$(repo_lock_mtime_epoch "$lock_dir" 2>/dev/null || echo 0)
            now_epoch=$(date +%s)
            if [ "$lock_mtime" -gt 0 ] && [ $((now_epoch - lock_mtime)) -lt "$lock_grace_seconds" ]; then
                if repo_lock_wait "$human_name" "$lock_owner_name" "$lock_dir"; then
                    continue
                fi
                echo "[$(repo_lock_timestamp)] Refusing to start ${human_name}: another ${lock_owner_name} is still acquiring the lock." >&2
                echo "[$(repo_lock_timestamp)] Active lock: ${lock_dir}" >&2
                return 1
            fi
        fi

        if [ -n "$lock_pid" ] && kill -0 "$lock_pid" 2>/dev/null; then
            if repo_lock_wait "$human_name" "$lock_owner_name" "$lock_dir"; then
                continue
            fi
            echo "[$(repo_lock_timestamp)] Refusing to start ${human_name}: another ${lock_owner_name} is already running (pid ${lock_pid})." >&2
            echo "[$(repo_lock_timestamp)] Active lock: ${lock_dir}" >&2
            return 1
        fi

        echo "[$(repo_lock_timestamp)] Removing stale ${lock_owner_name} lock: ${lock_dir}" >&2
        rm -rf "$lock_dir"
    done

    printf '%s\n' "$$" > "$lock_dir/pid"
    printf '%s\n' "${PWD}" > "$lock_dir/pwd"
    REPO_LOCK_DIR_ACTIVE="$lock_dir"
    trap release_repo_lock EXIT INT TERM
}
