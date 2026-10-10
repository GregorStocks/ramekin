#!/bin/bash
# Kill leftover server processes matching a pattern, but only this worktree's.
# Both dev and serve run the server with cwd ./server, so a cwd match keeps us
# from killing other worktrees' servers (or the calling make recipe's shell).
#
# Usage: kill-worktree-server.sh <pgrep -f pattern>

set -u

if [ $# -ne 1 ]; then
    echo "Usage: $0 <pattern>" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SERVER_DIR="$(cd "$SCRIPT_DIR/../server" && pwd -P)"

process_cwd() {
    if [ -e "/proc/$1/cwd" ]; then
        readlink "/proc/$1/cwd"
    else
        lsof -a -p "$1" -d cwd -Fn 2>/dev/null | sed -n 's/^n//p'
    fi
}

for pid in $(pgrep -f -- "$1"); do
    [ "$pid" = "$$" ] && continue
    if [ "$(process_cwd "$pid" 2>/dev/null)" = "$SERVER_DIR" ]; then
        kill "$pid" 2>/dev/null || true
    fi
done

exit 0
