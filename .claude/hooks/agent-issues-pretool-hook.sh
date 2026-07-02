#!/bin/sh
# BEGIN agent-issues generated
set -eu

agent_issues_tool="git+https://github.com/GregorStocks/agent-issues.git"

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
cat > "$tmp"

project_dir="${CLAUDE_PROJECT_DIR:-$(pwd)}"
cd "$project_dir"

local_hook="$project_dir/.claude/hooks/pretool-local.sh"
if [ -x "$local_hook" ]; then
    set +e
    "$local_hook" < "$tmp"
    local_status="$?"
    set -e
    if [ "$local_status" -ne 0 ]; then
        echo "Local pre-tool hook failed with status $local_status; blocking command." >&2
        exit 2
    fi
fi

hook_bin="$(command -v agent-pretool-hook || true)"
if [ -z "$hook_bin" ]; then
    uv_bin_dir="$(uv tool dir --bin 2>/dev/null || true)"
    hook_bin="$uv_bin_dir/agent-pretool-hook"
fi
if [ ! -x "$hook_bin" ] && command -v uv >/dev/null 2>&1; then
    if ! uv tool install -U "$agent_issues_tool" >/dev/null; then
        echo "Failed to install agent-issues with uv; blocking command." >&2
    fi
    uv_bin_dir="$(uv tool dir --bin 2>/dev/null || true)"
    hook_bin="$uv_bin_dir/agent-pretool-hook"
fi
if [ ! -x "$hook_bin" ]; then
    echo "agent-pretool-hook is not installed or not executable; blocking command." >&2
    exit 2
fi

set +e
"$hook_bin" --config "$project_dir/.agent-issues/pretool-hook.json5" < "$tmp"
hook_status="$?"
set -e
if [ "$hook_status" -eq 0 ] || [ "$hook_status" -eq 2 ]; then
    exit "$hook_status"
fi

echo "agent-pretool-hook failed with status $hook_status; blocking command." >&2
exit 2
# END agent-issues generated
