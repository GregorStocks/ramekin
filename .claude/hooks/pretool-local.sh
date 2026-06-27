#!/bin/sh
set -eu

tmp="$(mktemp)"
trap 'rm -f "$tmp"' EXIT
cat > "$tmp"

python3 - "$tmp" <<'PY'
import json
import os
import shlex
import sys


def leading_env_assignments(tokens: list[str]) -> tuple[dict[str, str], int]:
    env: dict[str, str] = {}
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if "=" not in token or token.startswith("="):
            break
        name, value = token.split("=", 1)
        if not name.replace("_", "a").isalnum() or not (
            name[0].isalpha() or name[0] == "_"
        ):
            break
        env[name] = value
        index += 1
    return env, index


def collect_git_aliases(args: list[str], env: dict[str, str]) -> dict[str, str]:
    aliases: dict[str, str] = {}
    try:
        count = int(env.get("GIT_CONFIG_COUNT", "0"))
    except ValueError:
        count = 0
    for index in range(count):
        key = env.get(f"GIT_CONFIG_KEY_{index}", "")
        value = env.get(f"GIT_CONFIG_VALUE_{index}", "")
        if key.startswith("alias."):
            aliases[key[len("alias.") :]] = value

    index = 0
    while index < len(args):
        token = args[index]
        if token == "-c" and index + 1 < len(args):
            value = args[index + 1]
            if value.startswith("alias.") and "=" in value:
                name, alias = value[len("alias.") :].split("=", 1)
                aliases[name] = alias
            index += 2
            continue
        index += 1
    return aliases


def skip_git_global_options(args: list[str], start: int = 0) -> int | None:
    index = start
    opts_with_arg = {
        "-C",
        "-c",
        "--exec-path",
        "--git-dir",
        "--work-tree",
        "--namespace",
        "--super-prefix",
        "--config-env",
        "--attr-source",
        "--list-cmds",
    }
    while index < len(args):
        token = args[index]
        if token in opts_with_arg:
            index += 2
            continue
        if any(
            token.startswith(prefix)
            for prefix in (
                "--exec-path=",
                "--git-dir=",
                "--work-tree=",
                "--namespace=",
                "--super-prefix=",
                "--config-env=",
                "--attr-source=",
                "--list-cmds=",
            )
        ):
            index += 1
            continue
        if token.startswith("-"):
            index += 1
            continue
        return index
    return None


def git_subcommand(args: list[str], env: dict[str, str]) -> tuple[str | None, list[str]]:
    aliases = collect_git_aliases(args, env)
    current = list(args)
    seen: set[str] = set()
    for _ in range(16):
        index = skip_git_global_options(current)
        if index is None:
            return None, []
        token = current[index]
        if token not in aliases or token in seen:
            return token, current[index + 1 :]
        seen.add(token)
        try:
            alias_tokens = shlex.split(aliases[token])
        except ValueError:
            return token, current[index + 1 :]
        current = current[:index] + alias_tokens + current[index + 1 :]
    index = skip_git_global_options(current)
    if index is None:
        return None, []
    return current[index], current[index + 1 :]


def shell_c_payload(args: list[str]) -> str | None:
    index = 0
    while index < len(args):
        token = args[index]
        if token == "--":
            return None
        if token in {"-c", "--command"}:
            return args[index + 1] if index + 1 < len(args) else None
        if token.startswith("--command="):
            return token[len("--command=") :]
        if token.startswith("-") and not token.startswith("--") and "c" in token[1:]:
            return args[index + 1] if index + 1 < len(args) else None
        index += 1
    return None


def env_s_payload(args: list[str]) -> str | None:
    index = 0
    while index < len(args):
        token = args[index]
        if token == "--":
            return None
        if token in {"-S", "--split-string"}:
            return args[index + 1] if index + 1 < len(args) else None
        if token.startswith("--split-string="):
            return token[len("--split-string=") :]
        if token.startswith("-") and not token.startswith("--") and "S" in token[1:]:
            cluster = token[1:]
            s_index = cluster.index("S")
            if s_index == len(cluster) - 1:
                return args[index + 1] if index + 1 < len(args) else None
            return cluster[s_index + 1 :]
        if "=" in token and not token.startswith("="):
            index += 1
            continue
        if token.startswith("-"):
            index += 1
            continue
        return None
    return None


def first_invocation(tokens: list[str]) -> tuple[dict[str, str], str, list[str]] | None:
    env, index = leading_env_assignments(tokens)
    if index >= len(tokens):
        return None
    return env, tokens[index], tokens[index + 1 :]


def command_from_args(args: list[str]) -> str:
    return " ".join(shlex.quote(arg) for arg in args)


def skip_env_options(args: list[str]) -> list[str]:
    index = 0
    opts_with_arg = {"-u", "--unset", "-C", "--chdir", "-S", "--split-string"}
    while index < len(args):
        token = args[index]
        if token == "--":
            return args[index + 1 :]
        if token in opts_with_arg:
            index += 2
            continue
        if token.startswith("--split-string="):
            index += 1
            continue
        if token.startswith("--"):
            index += 1
            continue
        if token.startswith("-") and len(token) > 1:
            index += 1
            continue
        if "=" in token and not token.startswith("="):
            index += 1
            continue
        return args[index:]
    return []


def skip_simple_wrapper_options(args: list[str], *, consumes_duration: bool = False) -> list[str]:
    index = 0
    while index < len(args):
        token = args[index]
        if token == "--":
            index += 1
            break
        if token in {"-a", "-n", "--adjustment", "-s", "--signal", "-k", "--kill-after"}:
            index += 2
            continue
        if token.startswith("-"):
            index += 1
            continue
        break
    if consumes_duration and index < len(args):
        index += 1
    return args[index:]


def unwrap_group(command: str) -> str | None:
    stripped = command.strip()
    if not stripped:
        return None
    if stripped.startswith("!") and (
        len(stripped) == 1 or stripped[1].isspace() or stripped[1] in "({"
    ):
        return stripped[1:].lstrip()
    if stripped.startswith("(") and stripped.endswith(")"):
        return stripped[1:-1].strip()
    if stripped.startswith("{") and stripped.endswith("}"):
        return stripped[1:-1].strip().rstrip(";").strip()
    return None


def extract_subshells(command: str) -> list[str]:
    bodies: list[str] = []
    in_single = False
    in_double = False
    index = 0
    while index < len(command):
        char = command[index]
        if char == "\\" and index + 1 < len(command) and not in_single:
            index += 2
            continue
        if char == "'" and not in_double:
            in_single = not in_single
            index += 1
            continue
        if char == '"' and not in_single:
            in_double = not in_double
            index += 1
            continue
        if in_single:
            index += 1
            continue

        dollar_paren = char == "$" and index + 1 < len(command) and command[index + 1] == "("
        process_sub = (
            not in_double
            and char in "<>"
            and index + 1 < len(command)
            and command[index + 1] == "("
        )
        if dollar_paren or process_sub:
            depth = 1
            cursor = index + 2
            body_single = False
            body_double = False
            while cursor < len(command) and depth > 0:
                body_char = command[cursor]
                if body_char == "\\" and cursor + 1 < len(command) and not body_single:
                    cursor += 2
                    continue
                if body_char == "'" and not body_double:
                    body_single = not body_single
                    cursor += 1
                    continue
                if body_char == '"' and not body_single:
                    body_double = not body_double
                    cursor += 1
                    continue
                if body_single or body_double:
                    cursor += 1
                    continue
                if body_char == "(":
                    depth += 1
                elif body_char == ")":
                    depth -= 1
                    if depth == 0:
                        break
                cursor += 1
            if depth == 0:
                bodies.append(command[index + 2 : cursor])
                index = cursor + 1
                continue

        if char == "`":
            cursor = index + 1
            while cursor < len(command):
                body_char = command[cursor]
                if body_char == "\\" and cursor + 1 < len(command):
                    cursor += 2
                    continue
                if body_char == "`":
                    break
                cursor += 1
            if cursor < len(command):
                bodies.append(command[index + 1 : cursor])
                index = cursor + 1
                continue
        index += 1
    return bodies


def shell_segments(command: str) -> list[str]:
    command = command.replace("\\\n", " ")
    segments: list[str] = []
    buffer: list[str] = []
    in_single = False
    in_double = False
    paren_depth = 0
    brace_depth = 0
    index = 0

    def flush() -> None:
        text = "".join(buffer).strip()
        if text:
            segments.append(text)
        buffer.clear()

    while index < len(command):
        char = command[index]
        if char == "\\" and index + 1 < len(command) and not in_single:
            buffer.append(char)
            buffer.append(command[index + 1])
            index += 2
            continue
        if char == "'" and not in_double:
            in_single = not in_single
            buffer.append(char)
            index += 1
            continue
        if char == '"' and not in_single:
            in_double = not in_double
            buffer.append(char)
            index += 1
            continue
        if in_single or in_double:
            buffer.append(char)
            index += 1
            continue
        if char == "(":
            paren_depth += 1
            buffer.append(char)
            index += 1
            continue
        if char == ")":
            paren_depth = max(0, paren_depth - 1)
            buffer.append(char)
            index += 1
            continue
        if char == "{":
            brace_depth += 1
            buffer.append(char)
            index += 1
            continue
        if char == "}":
            brace_depth = max(0, brace_depth - 1)
            buffer.append(char)
            index += 1
            continue
        if paren_depth > 0 or brace_depth > 0:
            buffer.append(char)
            index += 1
            continue
        if command.startswith("&&", index) or command.startswith("||", index):
            flush()
            index += 2
            continue
        if char in ";\n|&":
            flush()
            index += 1
            continue
        buffer.append(char)
        index += 1
    flush()
    return segments


def blocks_tmp_worktree(command: str) -> bool:
    if any(blocks_tmp_worktree(body) for body in extract_subshells(command)):
        return True
    inner = unwrap_group(command)
    if inner is not None:
        return blocks_tmp_worktree(inner)
    segments = shell_segments(command)
    if len(segments) > 1:
        return any(blocks_tmp_worktree(segment) for segment in segments)
    try:
        tokens = shlex.split(command)
    except ValueError:
        return False
    invocation = first_invocation(tokens)
    if invocation is None:
        return False
    env, executable, args = invocation
    basename = os.path.basename(executable)
    if basename == "env":
        payload = env_s_payload(args)
        if payload is not None and blocks_tmp_worktree(payload):
            return True
        return blocks_tmp_worktree(command_from_args(skip_env_options(args)))
    if basename in {"command", "builtin", "exec"}:
        return blocks_tmp_worktree(command_from_args(skip_simple_wrapper_options(args)))
    if basename in {"time", "nice", "nohup", "sudo"}:
        return blocks_tmp_worktree(command_from_args(skip_simple_wrapper_options(args)))
    if basename == "timeout":
        return blocks_tmp_worktree(
            command_from_args(skip_simple_wrapper_options(args, consumes_duration=True))
        )
    if basename in {"sh", "bash", "zsh", "dash", "ksh"}:
        payload = shell_c_payload(args)
        return payload is not None and blocks_tmp_worktree(payload)
    if basename == "eval":
        return blocks_tmp_worktree(" ".join(args))
    if basename != "git":
        return False
    subcommand, rest = git_subcommand(args, env)
    if subcommand != "worktree" or not rest or rest[0] != "add":
        return False
    return any(path == "/tmp" or path.startswith("/tmp/") for path in rest[1:])


with open(sys.argv[1], encoding="utf-8") as handle:
    data = json.load(handle)

tool_input = data.get("tool_input", {})
command = tool_input.get("command") if isinstance(tool_input, dict) else None
if not isinstance(command, str):
    raise SystemExit(0)

tool_name = data.get("tool_name")
if isinstance(tool_name, str) and tool_name not in {"Bash", "Shell"}:
    raise SystemExit(0)

if blocks_tmp_worktree(command):
    print(
        "Do not create git worktrees under /tmp. /tmp is RAM-backed on this "
        "machine and large worktrees plus builds can exhaust shared space. "
        "Use a disk-backed path such as ~/code/worktrees instead.",
        file=sys.stderr,
    )
    raise SystemExit(2)
PY
