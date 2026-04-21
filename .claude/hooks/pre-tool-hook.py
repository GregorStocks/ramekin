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

Implementation notes:
    Every rule operates on tokens, not raw regexes on the command string.
    Each shell segment is tokenized with shlex, leading env-var assignments
    are separated out, and transparent wrappers (time, sudo[+flags], nice,
    nohup, env, timeout[+flags] DURATION) are peeled off to reveal the real
    executable.  Matches are done against the executable's basename, so
    `/usr/bin/cargo` trips the cargo rule exactly like `cargo` does.
"""

import json
import os
import re
import shlex
import subprocess
import sys

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


# Wrapper options that consume an extra argv token. Applied when peeling
# wrappers off to reveal the real executable.
_WRAPPER_OPTS_WITH_ARG: dict[str, frozenset[str]] = {
    "env": frozenset({"-u", "--unset", "-C", "--chdir", "-S", "--split-string"}),
    "command": frozenset(),
    "builtin": frozenset(),
    "exec": frozenset({"-a"}),
    "time": frozenset(),
    "nohup": frozenset(),
    "nice": frozenset({"-n", "--adjustment"}),
    "sudo": frozenset(
        {
            "-u", "--user",
            "-g", "--group",
            "-U", "--other-user",
            "-C", "--close-from",
            "-D", "--chdir",
            "-h", "--host",
            "-p", "--prompt",
            "-r", "--role",
            "-t", "--type",
            "-T", "--command-timeout",
        }
    ),
    "timeout": frozenset({"-s", "--signal", "-k", "--kill-after"}),
}

# Wrappers whose last positional before the real command is an argument to
# the wrapper itself (e.g. `timeout 30 CMD`). After flag parsing ends the
# walker consumes one more token before handing off to the outer loop.
_WRAPPERS_WITH_POSITIONAL_ARG = frozenset({"timeout"})

_ENV_ASSIGN_RE = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.*", re.DOTALL)

# GNU `env` short options. Options that take an argument may be at the tail
# of a clustered short-option token (e.g. `-iu NAME` == `-i -u NAME`, while
# `-iuNAME` packs the argument inline).
_ENV_SHORT_OPTS_WITH_ARG = frozenset({"u", "C", "S"})
_ENV_SHORT_OPTS_NO_ARG = frozenset({"i", "v", "0"})


def _env_short_cluster_consumes_next(cluster: str) -> bool:
    position = 0
    while position < len(cluster):
        char = cluster[position]
        if char in _ENV_SHORT_OPTS_WITH_ARG:
            return position == len(cluster) - 1
        if char not in _ENV_SHORT_OPTS_NO_ARG:
            return False
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
        if wrapper in _WRAPPERS_WITH_POSITIONAL_ARG:
            # Consume the wrapper's own positional arg (e.g. timeout DURATION)
            # and let the outer loop resume with the real command.
            return index + 1
        return index
    return index


def _walk_segment(
    segment: str,
) -> tuple[dict[str, str], str, str, list[str]] | None:
    """Return (env, exe_raw, exe_basename, args) for a segment, or None.

    Peels leading env-var assignments and transparent wrappers
    (time/sudo/nohup/nice/timeout/env/command/builtin/exec) before exposing
    the real executable. Match against `exe_basename` so `/usr/bin/cargo`
    resolves to "cargo".
    """
    tokens = _shell_tokens(segment)
    if not tokens:
        return None
    env, index = _leading_env_assignments(tokens)
    while index < len(tokens):
        basename = os.path.basename(tokens[index])
        if basename in _WRAPPER_OPTS_WITH_ARG:
            index = _skip_wrapper_args(tokens, index + 1, basename)
            continue
        break
    if index >= len(tokens):
        return None
    exe_raw = tokens[index]
    exe_basename = os.path.basename(exe_raw)
    args = tokens[index + 1 :]
    return env, exe_raw, exe_basename, args


# Git global options that consume the next token as their argument.
_GIT_GLOBAL_OPTS_WITH_ARG = frozenset(
    {"-C", "-c", "--exec-path", "--git-dir", "--work-tree", "--namespace"}
)
_GIT_GLOBAL_LONG_OPTS_WITH_VALUE = (
    "--exec-path=",
    "--git-dir=",
    "--work-tree=",
    "--namespace=",
)


def _git_subcommand(args: list[str]) -> tuple[str | None, list[str]]:
    """Return (subcommand, remaining_args) for a git invocation."""
    index = 0
    while index < len(args):
        token = args[index]
        if token in _GIT_GLOBAL_OPTS_WITH_ARG:
            index += 2
            continue
        if any(token.startswith(p) for p in _GIT_GLOBAL_LONG_OPTS_WITH_VALUE):
            index += 1
            continue
        if token.startswith("-"):
            index += 1
            continue
        return token, args[index + 1 :]
    return None, []


# git switch / checkout: options that consume the NEXT argv token as their
# argument but do not themselves name the branch target. We must skip over
# them so the parser doesn't mistake the option's value for the branch name.
_SWITCH_OPTS_CONSUMING_NEXT = frozenset({"--conflict", "--start-point"})
_CHECKOUT_OPTS_CONSUMING_NEXT = frozenset(
    {"--conflict", "--pathspec-from-file", "--start-point"}
)
# git switch / checkout: options that signal a branch change we can't
# reliably resolve to a single target. Force the __UNKNOWN__ path so
# signoff can't be matched against anything but the real branch name.
_AMBIGUOUS_SWITCH_OPTS = frozenset({"--track", "-t"})
_AMBIGUOUS_CHECKOUT_OPTS = frozenset({"--track", "-t"})


def _branch_switch_target_for_switch(args: list[str]) -> str | None:
    index = 0
    while index < len(args):
        token = args[index]
        if token == "--":
            return None
        # Bare `-` is git's "previous branch" shorthand, not a flag.
        if token == "-":
            return "__UNKNOWN__"
        if token in _AMBIGUOUS_SWITCH_OPTS:
            return "__UNKNOWN__"
        if token in ("-c", "-C", "--create", "--force-create", "--orphan"):
            if index + 1 < len(args):
                return args[index + 1]
            return "__UNKNOWN__"
        if token in _SWITCH_OPTS_CONSUMING_NEXT:
            index += 2
            continue
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
        if token == "-":
            return "__UNKNOWN__"
        if token in _AMBIGUOUS_CHECKOUT_OPTS:
            return "__UNKNOWN__"
        if token in ("-b", "-B", "--orphan"):
            if index + 1 < len(args):
                return args[index + 1]
            return "__UNKNOWN__"
        if token in _CHECKOUT_OPTS_CONSUMING_NEXT:
            index += 2
            continue
        if token.startswith("-"):
            index += 1
            continue
        if "--" in args[index + 1 :]:
            return None
        # Plain `git checkout <thing>` is ambiguous, but treating it as a
        # branch-changing flow is safer than silently permitting branch hops.
        return token
    return None


# Commands that mutate files on disk. For each, any arg that resolves under
# a protected pipeline path should block the call.
_FS_MUTATION_CMDS = frozenset(
    {"rm", "mv", "cp", "install", "touch", "truncate", "mkdir", "rmdir"}
)
# `sed -i ...` / `perl -pi ...` write the target file in place; for these we
# only want to fire when an in-place flag is present.
_INPLACE_EDIT_FLAGS: dict[str, frozenset[str]] = {
    "sed": frozenset({"-i", "--in-place"}),
    "perl": frozenset(),  # handled by _perl_in_place below
}
# git subcommands that can overwrite working-tree contents.
_GIT_WORKTREE_MUTATION_SUBCMDS = frozenset({"checkout", "restore", "clean"})


def _path_is_protected(path: str) -> bool:
    """True if `path` names a file inside one of the protected pipeline dirs.

    Catches both repo-relative (`data/pipeline-snapshots/x.json`,
    `./data/pipeline-snapshots/`) and absolute forms
    (`/workspace/ramekin/data/pipeline-snapshots/x.json`) so the guard can't
    be slipped by rewriting the path.
    """
    if not path:
        return False
    normalized = os.path.normpath(path)
    for protected in _PIPELINE_OUTPUT_PATHS:
        # Relative path pointing into the protected dir from the repo root.
        if normalized == protected or normalized.startswith(protected + os.sep):
            return True
        # Absolute (or deeply-nested) form — look for the protected suffix
        # at a path-component boundary.
        marker = os.sep + protected
        if marker in normalized:
            tail = normalized.split(marker, 1)[1]
            if tail == "" or tail.startswith(os.sep):
                return True
    return False


def _any_arg_targets_pipeline(args: list[str]) -> bool:
    return any(_path_is_protected(a) for a in args if not a.startswith("-"))


def _sed_is_inplace(args: list[str]) -> bool:
    for arg in args:
        if arg in _INPLACE_EDIT_FLAGS["sed"]:
            return True
        # GNU sed allows `-i.bak` (backup suffix glued on).
        if arg.startswith("-i") and not arg.startswith("--"):
            return True
        if arg.startswith("--in-place"):
            return True
    return False


def _perl_is_inplace(args: list[str]) -> bool:
    # `perl -pi -e '...'` / `perl -pie '...'` / `perl -i.bak -pe '...'` all
    # rewrite in place. Detect any short-option cluster containing `i`.
    for arg in args:
        if not arg.startswith("-") or arg.startswith("--"):
            continue
        if "i" in arg[1:]:
            return True
    return False


def pipeline_mutation_message(
    exe_basename: str, exe_raw: str, args: list[str]
) -> str | None:
    """Return the pipeline-mutation error if this call writes a protected path."""
    if exe_basename in _FS_MUTATION_CMDS and _any_arg_targets_pipeline(args):
        return _pipeline_mutation_error()
    if exe_basename == "sed" and _sed_is_inplace(args) and _any_arg_targets_pipeline(args):
        return _pipeline_mutation_error()
    if exe_basename == "perl" and _perl_is_inplace(args) and _any_arg_targets_pipeline(args):
        return _pipeline_mutation_error()
    if exe_basename == "git":
        subcmd, rest = _git_subcommand(args)
        if subcmd in _GIT_WORKTREE_MUTATION_SUBCMDS and _any_arg_targets_pipeline(rest):
            return _pipeline_mutation_error()
    return None


def _pipeline_mutation_error() -> str:
    return (
        "Do not manually edit, restore, delete, or otherwise mutate "
        "data/pipeline-snapshots or data/pipeline-runs from the shell. "
        "The only acceptable way to change those directories is to run "
        "'make pipeline'."
    )


def _git_worktree_add_under_tmp(args: list[str]) -> bool:
    """True if the git sub-invocation is `worktree add ... /tmp[/...]`."""
    subcmd, rest = _git_subcommand(args)
    if subcmd != "worktree" or not rest or rest[0] != "add":
        return False
    for token in rest[1:]:
        if token == "/tmp" or token.startswith("/tmp/"):
            return True
    return False


def _looks_like_built_ramekin_cli(exe_raw: str, exe_basename: str) -> bool:
    if exe_basename != "ramekin-cli":
        return False
    # Block explicitly when invoked from a target/ build dir. Tolerate any
    # system-installed ramekin-cli (on $PATH) so users can still script
    # around the released binary if one exists.
    return bool(re.search(r"(?:^|/)cli/target/(?:release|debug)/ramekin-cli$", exe_raw))


def _first_positional(args: list[str]) -> str | None:
    for token in args:
        if not token.startswith("-"):
            return token
    return None


# gh flags that consume the next token as their argument. Not exhaustive,
# but covers the cross-cutting ones people reach for (`-R owner/repo`,
# `--repo ...`, `--hostname ghe.example.com`) plus a few per-subcommand
# flags that routinely appear between `gh` and the subcommand on the CLI.
_GH_OPTS_CONSUMING_NEXT = frozenset(
    {"-R", "--repo", "--hostname", "-F", "--field", "-f", "--raw-field"}
)


def _gh_subcommand_chain(args: list[str]) -> list[str]:
    """Return the positional subcommand chain for a gh invocation.

    Skips both flags and their argument values, so commands like
    `gh -R owner/repo issue list` resolve to `["issue", "list"]`.
    """
    chain: list[str] = []
    index = 0
    while index < len(args):
        token = args[index]
        if token in _GH_OPTS_CONSUMING_NEXT:
            index += 2
            continue
        if token.startswith("--") and "=" in token:
            # --repo=owner/repo bundles the value into one token.
            index += 1
            continue
        if token.startswith("-"):
            index += 1
            continue
        chain.append(token)
        index += 1
    return chain


def _segment_rejection(
    env: dict[str, str],
    exe_raw: str,
    exe_basename: str,
    args: list[str],
    dirty_pipeline_output: bool,
) -> str | None:
    # Process-killing by name — other agents share this machine.
    if exe_basename in ("pkill", "killall"):
        return (
            "Do not use pkill/killall — other agents share this machine. "
            "Only kill processes by specific PID if you are certain the PID "
            "belongs to your own worktree."
        )

    if exe_basename == "rustfmt":
        return "Do not run rustfmt directly. Use 'make lint' instead."

    if _looks_like_built_ramekin_cli(exe_raw, exe_basename):
        return (
            "Do not invoke the ramekin-cli binary directly (e.g. "
            "cli/target/release/ramekin-cli). Use the appropriate make target "
            "instead (e.g. 'make test'). The Makefile checks prerequisites "
            "and sets up the DB."
        )

    if exe_basename == "gh":
        chain = _gh_subcommand_chain(args)
        if chain and chain[0] == "issue":
            return (
                "Do not use GitHub issues. See doc/issues.md for how this "
                "project tracks issues (JSON5 files in issues/)."
            )
        if len(chain) >= 2 and chain[0] == "pr" and chain[1] == "edit":
            return agent_submit_message()

    if exe_basename == "cargo":
        subcmd = _first_positional(args)
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

    # Protect pipeline output from shell-level mutation.
    message = pipeline_mutation_message(exe_basename, exe_raw, args)
    if message is not None:
        return message

    if exe_basename == "git":
        subcmd, rest = _git_subcommand(args)

        if _git_worktree_add_under_tmp(args):
            return (
                "Do not create git worktrees under /tmp. /tmp is a tmpfs on this "
                "machine and large worktrees plus builds will exhaust RAM-backed "
                "space. Use a disk-backed path (e.g. ~/code/worktrees) instead."
            )

        if subcmd == "push":
            if dirty_pipeline_output:
                return (
                    "Refusing to push while pipeline output is still modified under "
                    "data/pipeline-snapshots/ or data/pipeline-runs/. Keep the full "
                    "result of 'make pipeline' together: stage it, commit it, and "
                    "only then push."
                )
            return agent_submit_message()

        if subcmd in ("switch", "checkout"):
            if subcmd == "switch":
                target = _branch_switch_target_for_switch(rest)
            else:
                target = _branch_switch_target_for_checkout(rest)
            if target is not None:
                approved = env.get(_BRANCH_SWITCH_SIGNOFF_ENV)
                if target == "__UNKNOWN__":
                    return (
                        "Refusing branch-changing git command. This checkout/switch form "
                        "needs explicit user signoff, and the hook could not determine a "
                        f"single target branch to verify via {_BRANCH_SWITCH_SIGNOFF_ENV}."
                    )
                if approved != target:
                    return (
                        "Refusing branch-changing git command without explicit user signoff. "
                        "After the user approves switching to that specific branch, re-run the "
                        f"command with {_BRANCH_SWITCH_SIGNOFF_ENV}={target} immediately before "
                        "the git invocation."
                    )

    return None


def _segment_uses_git_push(segment: str) -> bool:
    walked = _walk_segment(segment)
    if walked is None:
        return False
    _env, _raw, exe_basename, args = walked
    if exe_basename != "git":
        return False
    subcmd, _ = _git_subcommand(args)
    return subcmd == "push"


def command_uses_git_push(command: str) -> bool:
    return any(_segment_uses_git_push(s) for s in _shell_command_segments(command))


def rejection_message(command: str, dirty_pipeline_output: bool) -> str | None:
    for segment in _shell_command_segments(command):
        walked = _walk_segment(segment)
        if walked is None:
            continue
        env, exe_raw, exe_basename, args = walked
        message = _segment_rejection(
            env, exe_raw, exe_basename, args, dirty_pipeline_output
        )
        if message is not None:
            return message
    return None


def main() -> None:
    try:
        data = json.load(sys.stdin)
    except (json.JSONDecodeError, EOFError):
        return  # allow on parse failure

    command = data.get("tool_input", {}).get("command", "")
    dirty_pipeline_output = False
    if command_uses_git_push(command):
        dirty_pipeline_output = has_uncommitted_pipeline_output()

    message = rejection_message(command, dirty_pipeline_output)
    if message:
        print(message, file=sys.stderr)
        sys.exit(2)


if __name__ == "__main__":
    main()
