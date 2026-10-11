#!/bin/bash

# Repo locks are flock(2) locks on a file the holder's shell keeps open on
# descriptor 200. The kernel releases them when every copy of that descriptor is
# closed, so a crashed or SIGKILLed holder never leaves a stale lock behind, and
# children inherit the descriptor: a run killed while its workers (npm ci,
# openapi-generator, ...) keep running stays locked until they finish too.
# Lock files are never deleted; unlinking one would let a new waiter lock a
# fresh file while orphaned children still hold the old one.

repo_lock_timestamp() {
    date +%H:%M:%S
}

# Lock descriptor 200 exclusively, waiting up to $1 seconds. Exits 3 if the lock
# is still held. python3 because flock(1) isn't on stock macOS; the lock belongs
# to the shell's open file, so it outlives this helper process.
repo_lock_flock() {
    python3 -c '
import fcntl, sys, time

deadline = time.monotonic() + float(sys.argv[1])
while True:
    try:
        fcntl.flock(200, fcntl.LOCK_EX | fcntl.LOCK_NB)
        sys.exit(0)
    except BlockingIOError:
        if time.monotonic() >= deadline:
            sys.exit(3)
        time.sleep(0.1)
' "$1"
}

release_repo_lock() {
    if [ -n "${REPO_LOCK_FILE_ACTIVE:-}" ]; then
        exec 200>&-
        REPO_LOCK_FILE_ACTIVE=
    fi
}

# With REPO_LOCK_WAIT=1, wait up to REPO_LOCK_WAIT_SECONDS for a live lock
# holder instead of refusing to start, so a hung holder can't block forever.
acquire_repo_lock() {
    local lock_name="$1"
    local human_name="$2"
    local lock_owner_name="${3:-$human_name}"
    local lock_root="${REPO_LOCK_DIR:-logs/locks}"
    local lock_file="${lock_root}/${lock_name}.flock"
    local wait_seconds="${REPO_LOCK_WAIT_SECONDS:-600}"
    local status=0
    local holder_pid

    # One descriptor per shell; a second lock would silently drop the first.
    if [ -n "${REPO_LOCK_FILE_ACTIVE:-}" ]; then
        echo "[$(repo_lock_timestamp)] Can't lock ${lock_file}: this shell already holds ${REPO_LOCK_FILE_ACTIVE}." >&2
        return 1
    fi

    mkdir -p "$lock_root"
    # Append mode: opening must not truncate the holder's pid out of the file.
    exec 200>>"$lock_file"

    repo_lock_flock 0 || status=$?
    if [ "$status" -eq 3 ] && [ -n "${REPO_LOCK_WAIT:-}" ]; then
        echo "[$(repo_lock_timestamp)] Waiting up to ${wait_seconds}s for another ${lock_owner_name} to finish before starting ${human_name}." >&2
        echo "[$(repo_lock_timestamp)] Active lock: ${lock_file}" >&2
        status=0
        repo_lock_flock "$wait_seconds" || status=$?
    fi
    if [ "$status" -ne 0 ]; then
        exec 200>&-
        if [ "$status" -eq 3 ]; then
            # Empty if the holder hasn't recorded itself yet.
            holder_pid=$(head -n 1 "$lock_file" 2>/dev/null || true)
            echo "[$(repo_lock_timestamp)] Refusing to start ${human_name}: another ${lock_owner_name} is already running${holder_pid:+ (pid ${holder_pid})}." >&2
            echo "[$(repo_lock_timestamp)] Active lock: ${lock_file}" >&2
        else
            echo "[$(repo_lock_timestamp)] Failed to lock ${lock_file}." >&2
        fi
        return 1
    fi

    # For humans and the refusal message above; the lock itself is the flock.
    printf '%s\n%s\n' "$$" "${PWD}" > "$lock_file"
    REPO_LOCK_FILE_ACTIVE="$lock_file"
    trap release_repo_lock EXIT
    # Exit on signals (releasing via the EXIT trap) rather than swallowing them:
    # a trap that only released the lock would let the run carry on unlocked.
    trap 'exit 130' INT
    trap 'exit 143' TERM
}
