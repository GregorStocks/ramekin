#!/usr/bin/env python3
"""Claude Code PreToolUse hook for Bash commands.

Blocks all cargo commands — use make targets instead.
Blocks rustfmt — use `make lint`.
Blocks GitHub issue commands (gh issue).
Blocks raw PR publishing commands — use agent-submit instead.
Blocks pkill/killall — other agents share this machine.
Blocks direct invocation of the ramekin-cli binary.
Blocks branch-changing git commands without explicit signoff.
Protects generated pipeline output from manual shell edits and from
pushes that leave pipeline results unstaged or uncommitted.
"""

import json
import os
import re
import shlex
import subprocess
import sys

# Matches 'cargo' at shell command position: start of string, after a
# newline, or after a command separator (&&, ||, ;, |, $().  Optional
# leading env-var assignments (FOO=bar) are allowed.
_CARGO_CMD_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*cargo\b"
)

# Matches pkill or killall at shell command position.
_KILL_BY_NAME_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:sudo\s+)?(?:pkill|killall)\b"
)

# Matches 'rustfmt' at shell command position.
_RUSTFMT_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*rustfmt\b"
)

# Matches invocations of the built ramekin-cli binary at shell command
# position (e.g. 'cli/target/release/ramekin-cli', './cli/target/debug/ramekin-cli',
# or an absolute path).  Running the built binary directly bypasses the
# Makefile's dependency checks and DB setup.
_RAMEKIN_CLI_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*"
    r"\S*\bcli/target/(?:release|debug)/ramekin-cli\b"
)

# Matches 'gh issue' at shell command position.
_GH_ISSUE_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*gh\s+issue\b"
)

# Matches 'git push' at shell command position.
_GIT_PUSH_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*git\s+push(?:\s|$)"
)

# Matches 'gh pr edit' at shell command position.
_GH_PR_EDIT_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*gh\s+pr\s+edit\b"
)

# Matches 'git worktree add' creating a worktree under /tmp.
_TMP_WORKTREE_ADD_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*"
    r"git\s+worktree\s+add\b[^\n]*\s/tmp(?:/|\b)"
)

# Matches manual mutation of generated pipeline output via common shell commands.
_PIPELINE_MUTATION_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*"
    r"(?:rm|mv|cp|install|touch|truncate|mkdir|rmdir|sed\s+-i|perl\s+-pi|git\s+checkout|git\s+restore|git\s+clean)\b"
    r"[^\n]*\bdata/(?:pipeline-snapshots|pipeline-runs)(?:/|\b)"
)

_PIPELINE_OUTPUT_PATHS = ("data/pipeline-snapshots", "data/pipeline-runs")
_BRANCH_SWITCH_SIGNOFF_ENV = "RAMEKIN_BRANCH_SWITCH_SIGNOFF"

# Maps cargo subcommands to suggested make targets.
_CARGO_TO_MAKE = {
    "build": "make test",
    "test": "make test",
    "fmt": "make lint",
    "clippy": "make lint",
    "check": "make lint",
    "run": "make test or another make target",
}

# Regex to extract the cargo subcommand from a command string.
_CARGO_SUBCMD_RE = re.compile(
    r"(?:^|\n|&&|\|\||[;&|]|\$\()\s*(?:\w+=\S+\s+)*cargo\s+(\w+)"
)


def _shell_command_segments(command: str) -> list[str]:
    return [
        segment.strip()
        for segment in re.split(r"\n|&&|\|\||[;|]", command)
        if segment.strip()
    ]


def has_uncommitted_pipeline_output() -> bool:
    result = subprocess.run(
        ["git", "status", "--short", "--", *_PIPELINE_OUTPUT_PATHS],
        check=False,
        capture_output=True,
        text=True,
    )
    return bool(result.stdout.strip())


def agent_submit_message() -> str:
    return (
        "Do not publish PR updates with raw 'git push' or 'gh pr edit'. Use "
        "'agent-submit --title \"...\" --body \"...\"' so push, PR metadata "
        "updates, and CI watching happen together."
    )


def _shell_tokens(command: str) -> list[str] | None:
    try:
        return shlex.split(command, posix=True)
    except ValueError:
        return None


def _leading_env_assignments(tokens: list[str]) -> tuple[dict[str, str], int]:
    env: dict[str, str] = {}
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if "=" not in token or token.startswith("="):
            break
        name, value = token.split("=", 1)
        if not name or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
            break
        env[name] = value
        index += 1
    return env, index


def branch_switch_target(command: str) -> str | None:
    for segment in _shell_command_segments(command):
        target = _branch_switch_target_for_segment(segment)
        if target is not None:
            return target
    return None


def _branch_switch_target_for_segment(command: str) -> str | None:
    tokens = _shell_tokens(command)
    if not tokens:
        return None

    index = _git_executable_index(tokens)
    if index is None:
        return None

    subcommand_index = _git_branch_subcommand_index(tokens, index + 1)
    if subcommand_index is None:
        return None

    subcommand = tokens[subcommand_index]
    args = tokens[subcommand_index + 1 :]
    if subcommand == "switch":
        return _branch_switch_target_for_switch(args)
    if subcommand == "checkout":
        return _branch_switch_target_for_checkout(args)
    return None


# Wrapper options that consume an extra argv token (so the hook can walk past
# the option's argument when resolving the real executable).
_WRAPPER_OPTS_WITH_ARG: dict[str, frozenset[str]] = {
    "env": frozenset({"-u", "--unset", "-C", "--chdir", "-S", "--split-string"}),
    "command": frozenset(),
    "builtin": frozenset(),
    "exec": frozenset({"-a"}),
}

_ENV_ASSIGN_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.*", re.DOTALL)

# GNU `env` short options. Options that take an argument may be at the tail
# of a clustered short-option token (e.g. `-iu NAME` == `-i -u NAME`, while
# `-iuNAME` packs the argument inline).
_ENV_SHORT_OPTS_WITH_ARG = frozenset({"u", "C", "S"})
_ENV_SHORT_OPTS_NO_ARG = frozenset({"i", "v", "0"})


def _env_short_cluster_consumes_next(cluster: str) -> bool:
    """True if a '-...' short-option cluster expects its arg in the next token."""
    position = 0
    while position < len(cluster):
        char = cluster[position]
        if char in _ENV_SHORT_OPTS_WITH_ARG:
            # An arg-taking option anywhere but the last position packs its
            # argument inline (e.g. `-uNAME`); at the last position the
            # argument lives in the next argv token.
            return position == len(cluster) - 1
        if char not in _ENV_SHORT_OPTS_NO_ARG:
            return False  # unknown short flag; don't guess it takes an arg
        position += 1
    return False


def _skip_wrapper_args(tokens: list[str], index: int, wrapper: str) -> int:
    """Advance past a wrapper's own options / env-assignments to the next command."""
    opts_with_arg = _WRAPPER_OPTS_WITH_ARG.get(wrapper, frozenset())
    parsing_options = True
    while index < len(tokens):
        token = tokens[index]
        if parsing_options:
            if token == "--":
                parsing_options = False
                index += 1
                continue
            if token == "-" and wrapper == "env":
                parsing_options = False
                index += 1
                continue
            if token in opts_with_arg:
                index += 2
                continue
            if (
                wrapper == "env"
                and token.startswith("-")
                and not token.startswith("--")
                and len(token) > 1
                and _env_short_cluster_consumes_next(token[1:])
            ):
                index += 2
                continue
            if token.startswith("-") and len(token) > 1:
                index += 1
                continue
        if wrapper == "env" and _ENV_ASSIGN_RE.fullmatch(token):
            index += 1
            continue
        return index
    return index


def _executable_index(tokens: list[str], name: str) -> int | None:
    """Locate the argv index of `name` once wrappers/env-prefixes are peeled off."""
    _env, index = _leading_env_assignments(tokens)
    while index < len(tokens):
        token = tokens[index]
        basename = os.path.basename(token)
        if basename in _WRAPPER_OPTS_WITH_ARG:
            index = _skip_wrapper_args(tokens, index + 1, basename)
            continue
        if basename == name:
            return index
        return None
    return None


def _git_executable_index(tokens: list[str]) -> int | None:
    return _executable_index(tokens, "git")


def _git_branch_subcommand_index(tokens: list[str], start: int) -> int | None:
    index = start
    while index < len(tokens):
        token = tokens[index]
        if token in ("switch", "checkout"):
            return index
        if token in ("-C", "-c", "--exec-path", "--git-dir", "--work-tree", "--namespace"):
            index += 2
            continue
        if token.startswith("--exec-path=") or token.startswith("--git-dir="):
            index += 1
            continue
        if token.startswith("--work-tree=") or token.startswith("--namespace="):
            index += 1
            continue
        if token.startswith("-"):
            index += 1
            continue
        return None
    return None


def _branch_switch_target_for_switch(args: list[str]) -> str | None:
    index = 0
    while index < len(args):
        token = args[index]
        if token == "--":
            return None
        if token in ("-c", "-C", "--create", "--force-create"):
            if index + 1 < len(args):
                return args[index + 1]
            return "__UNKNOWN__"
        if token.startswith("-"):
            index += 1
            continue
        return token
    return None


def _branch_switch_target_for_checkout(args: list[str]) -> str | None:
    index = 0
    while index < len(args):
        token = args[index]
        if token == "--":
            return None
        if token in ("-b", "-B", "--orphan"):
            if index + 1 < len(args):
                return args[index + 1]
            return "__UNKNOWN__"
        if token.startswith("-"):
            index += 1
            continue
        if "--" in args[index + 1 :]:
            return None
        # Plain `git checkout <thing>` is ambiguous, but treating it as a
        # branch-changing flow is safer than silently permitting branch hops.
        return token
    return None


def branch_switch_violation(command: str) -> str | None:
    for segment in _shell_command_segments(command):
        target = _branch_switch_target_for_segment(segment)
        if target is None:
            continue

        tokens = _shell_tokens(segment)
        assert tokens is not None
        env, _index = _leading_env_assignments(tokens)
        approved = env.get(_BRANCH_SWITCH_SIGNOFF_ENV)
        if target != "__UNKNOWN__" and approved == target:
            continue

        if target == "__UNKNOWN__":
            return (
                "Refusing branch-changing git command. This checkout/switch form "
                "needs explicit user signoff, and the hook could not determine a "
                f"single target branch to verify via {_BRANCH_SWITCH_SIGNOFF_ENV}."
            )

        return (
            "Refusing branch-changing git command without explicit user signoff. "
            "After the user approves switching to that specific branch, re-run the "
            f"command with {_BRANCH_SWITCH_SIGNOFF_ENV}={target} immediately before "
            "the git invocation."
        )

    return None


def rejection_message(command: str, dirty_pipeline_output: bool) -> str | None:
    # Block process-killing commands that target by name — multiple agent
    # worktrees share this machine and pkill/killall would hit other agents.
    if _KILL_BY_NAME_RE.search(command):
        return (
            "Do not use pkill/killall — other agents share this machine. "
            "Only kill processes by specific PID if you are certain the PID "
            "belongs to your own worktree."
        )

    if _GH_ISSUE_RE.search(command):
        return (
            "Do not use GitHub issues. See doc/issues.md for how this "
            "project tracks issues (JSON5 files in issues/)."
        )

    if _RUSTFMT_RE.search(command):
        return "Do not run rustfmt directly. Use 'make lint' instead."

    if _TMP_WORKTREE_ADD_RE.search(command):
        return (
            "Do not create git worktrees under /tmp. /tmp is a tmpfs on this "
            "machine and large worktrees plus builds will exhaust RAM-backed "
            "space. Use a disk-backed path (e.g. ~/code/worktrees) instead."
        )

    branch_switch_error = branch_switch_violation(command)
    if branch_switch_error is not None:
        return branch_switch_error

    if _PIPELINE_MUTATION_RE.search(command):
        return (
            "Do not manually edit, restore, delete, or otherwise mutate "
            "data/pipeline-snapshots or data/pipeline-runs from the shell. "
            "The only acceptable way to change those directories is to run "
            "'make pipeline'."
        )

    if _GIT_PUSH_RE.search(command) and dirty_pipeline_output:
        return (
            "Refusing to push while pipeline output is still modified under "
            "data/pipeline-snapshots/ or data/pipeline-runs/. Keep the full "
            "result of 'make pipeline' together: stage it, commit it, and "
            "only then push."
        )

    if _GIT_PUSH_RE.search(command) or _GH_PR_EDIT_RE.search(command):
        return agent_submit_message()

    if _RAMEKIN_CLI_RE.search(command):
        return (
            "Do not invoke the ramekin-cli binary directly (e.g. "
            "cli/target/release/ramekin-cli). Use the appropriate make target "
            "instead (e.g. 'make test'). The Makefile checks prerequisites "
            "and sets up the DB."
        )

    if _CARGO_CMD_RE.search(command):
        match = _CARGO_SUBCMD_RE.search(command)
        subcmd = match.group(1) if match else None
        suggestion = _CARGO_TO_MAKE.get(subcmd) if subcmd else None

        if suggestion:
            return (
                f"Do not run cargo commands directly. "
                f"Use '{suggestion}' instead."
            )
        return (
            "Do not run cargo commands directly. Use the appropriate make "
            "target instead (e.g. 'make test', 'make lint'). The Makefile "
            "checks prerequisites and sets up the DB."
        )

    return None


def main() -> None:
    try:
        data = json.load(sys.stdin)
    except (json.JSONDecodeError, EOFError):
        return  # allow on parse failure

    command = data.get("tool_input", {}).get("command", "")
    dirty_pipeline_output = False
    if _GIT_PUSH_RE.search(command):
        dirty_pipeline_output = has_uncommitted_pipeline_output()

    message = rejection_message(command, dirty_pipeline_output)
    if message:
        print(message, file=sys.stderr)
        sys.exit(2)


if __name__ == "__main__":
    main()
