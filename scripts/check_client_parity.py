#!/usr/bin/env python3
"""Detect generated-client endpoint usage drift between the web and iOS clients.

Reads api/openapi.json for the canonical list of operationIds, greps each
client tree (minus its generated-client dir) for the camelCase method name,
and diffs the observed set of (web, ios) flags against api/client-parity.json5.

Failures: an operation is used on a side the matrix doesn't allow, an entry
in the matrix doesn't match what's in the source tree, or an entry is missing
from the matrix entirely. Re-run with --update to rewrite the matrix from
observed state (preserving existing reasons where the platforms haven't
changed).
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path


VALID_PLATFORMS = {"both", "web-only", "ios-only", "neither"}
NEEDS_REASON = {"web-only", "ios-only", "neither"}

WEB_ROOTS = ["ramekin-ui/src"]
IOS_ROOTS = [
    "ramekin-ios/Ramekin",
    "ramekin-ios/RamekinShareExtension",
    "ramekin-ios/RamekinTests",
    "ramekin-ios/RamekinUITests",
    "ramekin-ios/Shared",
]


def project_root() -> Path:
    return Path(__file__).resolve().parent.parent


def snake_to_camel(name: str) -> str:
    parts = name.split("_")
    return parts[0] + "".join(p[:1].upper() + p[1:] for p in parts[1:])


def load_operation_ids(root: Path) -> list[str]:
    spec = json.loads((root / "api" / "openapi.json").read_text())
    ops: list[str] = []
    for methods in spec["paths"].values():
        for verb, op in methods.items():
            if verb not in {"get", "post", "put", "delete", "patch"}:
                continue
            op_id = op.get("operationId")
            if not op_id:
                raise SystemExit(f"OpenAPI op missing operationId: {op}")
            ops.append(op_id)
    return sorted(ops)


def grep_any(roots: list[Path], pattern: str) -> bool:
    existing = [str(r) for r in roots if r.exists()]
    if not existing:
        return False
    result = subprocess.run(
        [
            "grep",
            "-rE",
            "--include=*.ts",
            "--include=*.tsx",
            "--include=*.swift",
            pattern,
            *existing,
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    return result.returncode == 0


def observed_platforms(root: Path, method_name: str) -> str:
    pattern = rf"\.{re.escape(method_name)}\("
    web = grep_any([root / r for r in WEB_ROOTS], pattern)
    ios = grep_any([root / r for r in IOS_ROOTS], pattern)
    if web and ios:
        return "both"
    if web:
        return "web-only"
    if ios:
        return "ios-only"
    return "neither"


# Tiny JSON5 reader: strips // and /* */ comments and trailing commas before
# handing off to json. Good enough for our hand-edited matrix file.
_LINE_COMMENT = re.compile(r"//[^\n]*")
_BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.DOTALL)
_TRAILING_COMMA = re.compile(r",(\s*[}\]])")
_UNQUOTED_KEY = re.compile(r"([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)\s*:")


def load_parity(path: Path) -> dict:
    text = path.read_text()
    text = _BLOCK_COMMENT.sub("", text)
    text = _LINE_COMMENT.sub("", text)
    text = _UNQUOTED_KEY.sub(r'\1"\2":', text)
    text = _TRAILING_COMMA.sub(r"\1", text)
    return json.loads(text)


def parse_entry(entry) -> tuple[str, str | None]:
    if isinstance(entry, str):
        return entry, None
    if isinstance(entry, dict):
        platforms = entry.get("platforms")
        reason = entry.get("reason")
        return platforms, reason
    return "<invalid>", None


def check(root: Path) -> tuple[list[str], dict[str, str]]:
    """Return (errors, observed_map). observed_map is op_id -> platforms."""
    errors: list[str] = []
    op_ids = load_operation_ids(root)
    observed = {op: observed_platforms(root, snake_to_camel(op)) for op in op_ids}

    parity_path = root / "api" / "client-parity.json5"
    if not parity_path.exists():
        errors.append(
            f"Missing {parity_path.relative_to(root)} — create it with one "
            "entry per OpenAPI operationId."
        )
        return errors, observed

    parity = load_parity(parity_path)
    declared = parity.get("operations") or {}

    declared_keys = set(declared)
    observed_keys = set(observed)

    for missing in sorted(observed_keys - declared_keys):
        errors.append(
            f"{missing}: present in api/openapi.json but missing from "
            f"api/client-parity.json5 (observed: {observed[missing]})"
        )

    for stale in sorted(declared_keys - observed_keys):
        errors.append(
            f"{stale}: listed in api/client-parity.json5 but not in "
            "api/openapi.json (delete it)"
        )

    for op_id in sorted(observed_keys & declared_keys):
        declared_platforms, reason = parse_entry(declared[op_id])
        if declared_platforms not in VALID_PLATFORMS:
            errors.append(
                f"{op_id}: invalid platforms value '{declared_platforms}' "
                f"(must be one of {sorted(VALID_PLATFORMS)})"
            )
            continue
        if declared_platforms in NEEDS_REASON and not reason:
            errors.append(
                f"{op_id}: '{declared_platforms}' requires a reason — "
                "use {{ platforms: ..., reason: ... }} form"
            )
        if declared_platforms != observed[op_id]:
            errors.append(
                f"{op_id}: parity drift — matrix says '{declared_platforms}' "
                f"but source says '{observed[op_id]}'. Either update the "
                "other client to match, or change the matrix entry (with "
                "a reason explaining why)."
            )

    return errors, observed


def write_seed(root: Path, observed: dict[str, str]) -> None:
    """Write a fresh matrix from observed state, preserving existing reasons.

    When --update is invoked, we want to keep human-written reasons for
    entries whose platforms didn't change, but drop reasons that no longer
    apply (because the platform set shifted).
    """
    path = root / "api" / "client-parity.json5"
    existing: dict[str, dict] = {}
    if path.exists():
        prev = load_parity(path).get("operations") or {}
        for op_id, entry in prev.items():
            platforms, reason = parse_entry(entry)
            existing[op_id] = {"platforms": platforms, "reason": reason}

    lines = [
        "// Generated-client endpoint parity matrix.",
        "// One entry per OpenAPI operationId. See scripts/check_client_parity.py.",
        "//",
        "// Each entry is one of:",
        '//   "both"      — used by both web (ramekin-ui) and ios (ramekin-ios)',
        '//   "web-only"  — referenced only in the web client',
        '//   "ios-only"  — referenced only in the iOS client',
        '//   "neither"   — not referenced by either client (e.g. infra/health',
        "//                  endpoint, or fetched via URL rather than the",
        "//                  generated client)",
        "//",
        '// Non-"both" entries must include a reason via the object form:',
        '//   { platforms: "ios-only", reason: "..." }',
        "{",
        "  operations: {",
    ]

    for op_id in sorted(observed):
        platforms = observed[op_id]
        prior = existing.get(op_id)
        reason = (
            prior["reason"]
            if prior and prior.get("platforms") == platforms and prior.get("reason")
            else None
        )
        if platforms == "both":
            lines.append(f'    {op_id}: "both",')
        else:
            reason_text = reason if reason else "TODO: explain why"
            lines.append(
                f'    {op_id}: {{ platforms: "{platforms}", '
                f"reason: {json.dumps(reason_text)} }},"
            )
    lines += ["  },", "}", ""]
    path.write_text("\n".join(lines))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--update",
        action="store_true",
        help="Rewrite api/client-parity.json5 from observed state. "
        "Existing reasons are preserved when the platforms haven't changed.",
    )
    args = parser.parse_args()

    root = project_root()
    errors, observed = check(root)

    if args.update:
        write_seed(root, observed)
        print(f"Wrote api/client-parity.json5 with {len(observed)} entries.")
        return 0

    if errors:
        print("Client parity drift detected:\n", file=sys.stderr)
        for err in errors:
            print(f"  - {err}", file=sys.stderr)
        print(
            "\nTo update the matrix from observed state (e.g. after a "
            "deliberate adoption):\n"
            "  ./scripts/check_client_parity.py --update\n"
            "Then commit the diff with a note explaining the change.",
            file=sys.stderr,
        )
        return 1

    print(f"Client parity OK ({len(observed)} operations checked).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
