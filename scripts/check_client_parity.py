#!/usr/bin/env python3
"""Detect generated-client endpoint usage drift between the web and iOS clients.

Default contract: every OpenAPI operation is expected to be used by BOTH
clients. `api/client-parity-exceptions.json5` only lists the deliberate
exceptions to that rule.

The script reads `api/openapi.json` for the canonical operationId list,
greps each client tree (minus its generated-client dir) for the camelCase
method name, and compares observed `(web, ios)` flags to the exceptions
file. Failure modes:

- An op is `web-only`, `ios-only`, or unused, but isn't listed as an
  exception → either the other client needs to adopt it, or you should
  record the exception with a reason.
- An op is listed as an exception but is actually used by both clients
  now → drop the exception.
- An exception's recorded `platforms` doesn't match what's observed.

Re-run with `--update` to rewrite the exceptions file from observed
state, preserving existing reasons where the platform set didn't change.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


# Exception platforms — "both" is implicit (default) and not a valid exception.
EXCEPTION_PLATFORMS = {"web-only", "ios-only", "neither"}

WEB_ROOTS = ["ramekin-ui/src"]
IOS_ROOTS = [
    "ramekin-ios/Ramekin",
    "ramekin-ios/RamekinShareExtension",
    "ramekin-ios/RamekinTests",
    "ramekin-ios/RamekinUITests",
    "ramekin-ios/Shared",
]

EXCEPTIONS_FILE = "api/client-parity-exceptions.json5"


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def snake_to_camel(name: str) -> str:
    parts = name.split("_")
    return parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:])


def load_operations(root: Path) -> list[tuple[str, str]]:
    """Return sorted (operationId, primary_tag) tuples from openapi.json."""
    spec = json.loads((root / "api" / "openapi.json").read_text())
    ops: list[tuple[str, str]] = []
    for methods in spec["paths"].values():
        for verb, op in methods.items():
            if verb not in {"get", "post", "put", "delete", "patch"}:
                continue
            op_id = op.get("operationId")
            tags = op.get("tags") or []
            if not op_id or not tags:
                raise SystemExit(f"OpenAPI op missing operationId/tag: {op}")
            ops.append((op_id, tags[0]))
    return sorted(ops)


def tag_to_pascal(tag: str) -> str:
    return "".join(p[:1].upper() + p[1:] for p in tag.split("_"))


def _grep(args: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(args, capture_output=True, text=True, check=False)


def files_referencing(
    roots: list[Path], tokens: tuple[str, ...], suffixes: tuple[str, ...]
) -> list[str]:
    """Return paths of files (within roots) that contain ANY of the tokens."""
    existing = [str(r) for r in roots if r.exists()]
    if not existing or not tokens:
        return []
    includes = [f"--include=*{s}" for s in suffixes]
    # `-F` makes each pattern a literal string; with `-e` we OR them together.
    token_args: list[str] = []
    for t in tokens:
        token_args += ["-e", t]
    result = _grep(["grep", "-rlF", *includes, *token_args, *existing])
    seen: set[str] = set()
    out: list[str] = []
    for line in result.stdout.splitlines():
        if line and line not in seen:
            seen.add(line)
            out.append(line)
    return out


def any_file_has_call(files: list[str], method_name: str) -> bool:
    """True if any of `files` contains a `.<method_name>(` call site."""
    if not files:
        return False
    pattern = rf"\.{re.escape(method_name)}\("
    result = _grep(["grep", "-lE", pattern, *files])
    return result.returncode == 0


IOS_WRAPPER_RECEIVER = "RamekinAPI.shared"


def observed_platforms(root: Path, tag: str, method_name: str) -> str:
    """Detect whether each client uses the API operation.

    Matches are scoped to files that actually name an API surface — the
    generated class (e.g. `ShoppingListApi` in TS, `ShoppingListAPI` in
    Swift), or the iOS hand-rolled wrapper `RamekinAPI.shared` that
    forwards a subset of calls to the same endpoints. Scoping eliminates
    false positives from same-named helpers on local types (e.g.
    `store.clearChecked()` on a CoreData wrapper) — those files don't
    name any of the API surfaces.
    """
    pascal = tag_to_pascal(tag)
    web_files = files_referencing(
        [root / r for r in WEB_ROOTS], (f"{pascal}Api",), (".ts", ".tsx")
    )
    ios_files = files_referencing(
        [root / r for r in IOS_ROOTS],
        (f"{pascal}API", IOS_WRAPPER_RECEIVER),
        (".swift",),
    )
    web = any_file_has_call(web_files, method_name)
    ios = any_file_has_call(ios_files, method_name)
    if web and ios:
        return "both"
    if web:
        return "web-only"
    if ios:
        return "ios-only"
    return "neither"


# Tiny JSON5 reader: strips // and /* */ comments and trailing commas before
# handing off to json. Good enough for our hand-edited exceptions file.
_LINE_COMMENT = re.compile(r"//[^\n]*")
_BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.DOTALL)
_TRAILING_COMMA = re.compile(r",(\s*[}\]])")
_UNQUOTED_KEY = re.compile(r"([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:")


def load_exceptions(path: Path) -> dict:
    text = path.read_text()
    text = _BLOCK_COMMENT.sub("", text)
    text = _LINE_COMMENT.sub("", text)
    text = _UNQUOTED_KEY.sub(r'\1"\2":', text)
    text = _TRAILING_COMMA.sub(r"\1", text)
    return json.loads(text)


def parse_entry(entry) -> tuple[str, str | None]:
    if isinstance(entry, dict):
        return entry.get("platforms"), entry.get("reason")
    return "<invalid>", None


def check(root: Path) -> tuple[list[str], dict[str, str]]:
    """Return (errors, observed_map). observed_map is op_id -> platforms."""
    errors: list[str] = []
    ops = load_operations(root)
    observed = {
        op_id: observed_platforms(root, tag, snake_to_camel(op_id))
        for op_id, tag in ops
    }

    exceptions_path = root / EXCEPTIONS_FILE
    if not exceptions_path.exists():
        errors.append(f"Missing {EXCEPTIONS_FILE} — create it (use --update to seed).")
        return errors, observed

    declared = load_exceptions(exceptions_path)
    observed_keys = set(observed)

    for op_id in sorted(declared):
        if op_id not in observed_keys:
            errors.append(
                f"{op_id}: listed in {EXCEPTIONS_FILE} but not in "
                "api/openapi.json (delete the entry)."
            )
            continue

        declared_platforms, reason = parse_entry(declared[op_id])
        if declared_platforms not in EXCEPTION_PLATFORMS:
            errors.append(
                f"{op_id}: invalid platforms value '{declared_platforms}' "
                f"(must be one of {sorted(EXCEPTION_PLATFORMS)})."
            )
            continue
        if not reason:
            errors.append(
                f"{op_id}: every exception must include a reason — "
                "use { platforms: ..., reason: ... }."
            )

        actual = observed[op_id]
        if actual == "both":
            errors.append(
                f"{op_id}: {EXCEPTIONS_FILE} lists this as "
                f"'{declared_platforms}', but both clients now use it. "
                "Delete the exception entry."
            )
        elif declared_platforms != actual:
            errors.append(
                f"{op_id}: parity drift — exception says "
                f"'{declared_platforms}' but source shows '{actual}'. "
                "Either align the clients or update the exception entry."
            )

    for op_id in sorted(observed_keys - set(declared)):
        actual = observed[op_id]
        if actual != "both":
            errors.append(
                f"{op_id}: only used by '{actual}' but missing from "
                f"{EXCEPTIONS_FILE}. Either adopt it in the other client, "
                "or add an exception with a reason."
            )

    return errors, observed


def write_seed(root: Path, observed: dict[str, str]) -> None:
    """Rewrite the exceptions file from observed state, preserving reasons.

    Reasons whose recorded platforms still match the observed state are
    kept. Reasons for ops that are now `both` are dropped (the entry is
    removed entirely). Newly-non-both ops are written with a TODO reason.
    """
    path = root / EXCEPTIONS_FILE
    existing: dict[str, dict] = {}
    if path.exists():
        for op_id, entry in load_exceptions(path).items():
            platforms, reason = parse_entry(entry)
            existing[op_id] = {"platforms": platforms, "reason": reason}

    lines = [
        "// Generated-client endpoint parity exceptions.",
        "// See scripts/check_client_parity.py.",
        "//",
        "// The default contract is: every OpenAPI operation is used by",
        "// BOTH clients (ramekin-ui and ramekin-ios). This file only",
        "// lists deliberate exceptions to that rule.",
        "//",
        "// Each entry's `platforms` is one of:",
        '//   "web-only"  — referenced only in the web client',
        '//   "ios-only"  — referenced only in the iOS client',
        '//   "neither"   — not referenced by either client (e.g. infra/',
        "//                  health endpoint, or fetched via URL rather",
        "//                  than the generated client)",
        "//",
        "// Every entry must include a non-empty `reason`.",
        "//",
        "// To regenerate from the source tree after a deliberate adoption",
        "// or removal, run `./scripts/check_client_parity.py --update`",
        "// and commit the diff with a note explaining the change.",
        "{",
    ]

    for op_id in sorted(observed):
        platforms = observed[op_id]
        if platforms == "both":
            continue
        prior = existing.get(op_id)
        reason = (
            prior["reason"]
            if prior and prior.get("platforms") == platforms and prior.get("reason")
            else "TODO: explain why"
        )
        lines.append(
            f'  {op_id}: {{ platforms: "{platforms}", reason: {json.dumps(reason)} }},'
        )
    lines += ["}", ""]
    path.write_text("\n".join(lines))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--update",
        action="store_true",
        help=(
            f"Rewrite {EXCEPTIONS_FILE} from observed state. Existing "
            "reasons are preserved where the platforms haven't changed."
        ),
    )
    args = parser.parse_args()

    root = project_root()
    errors, observed = check(root)

    if args.update:
        write_seed(root, observed)
        non_both = sum(1 for v in observed.values() if v != "both")
        print(f"Wrote {EXCEPTIONS_FILE} with {non_both} exception(s).")
        return 0

    if errors:
        print("Client parity drift detected:\n", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        print(
            f"\nTo reseed {EXCEPTIONS_FILE} from observed state (after a "
            "deliberate change):\n"
            "  ./scripts/check_client_parity.py --update\n"
            "Then commit the diff with a note explaining the change.",
            file=sys.stderr,
        )
        return 1

    exceptions = sum(1 for v in observed.values() if v != "both")
    print(
        f"Client parity OK ({len(observed)} operations, "
        f"{exceptions} listed exception(s))."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
