#!/usr/bin/env python3
"""
Run all linters in parallel.

This script runs:
- Rust formatters and linters (server, cli, ingredient-density)
- TypeScript formatter and type checker
- CSS linter (Stylelint)
- Python formatter and linter
- YAML linter
- Shell script linter
- Swift linter (SwiftLint)
"""

import json
import os
import re
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path


def get_project_root() -> Path:
    """Get the project root directory."""
    return Path(__file__).parent.parent


def run_command(
    name: str, command: list[str], cwd: Path | None = None
) -> tuple[str, bool]:
    """Run a command and return (name, success)."""
    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            check=False,
        )

        # Print output if there is any (errors or warnings)
        if result.stdout:
            print(result.stdout, end="")
        if result.stderr:
            print(result.stderr, end="", file=sys.stderr)

        success = result.returncode == 0
        return (name, success)
    except Exception as e:
        print(f"Error running {name}: {e}", file=sys.stderr)
        return (name, False)


def lint_rust_server(project_root: Path) -> tuple[str, bool]:
    """Lint Rust server code."""
    server_dir = project_root / "server"

    # Run fmt
    fmt_result = subprocess.run(
        ["cargo", "fmt", "--all"],
        cwd=server_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Run clippy (using --release to match the server build profile in CI)
    clippy_result = subprocess.run(
        [
            "cargo",
            "clippy",
            "--release",
            "--all-targets",
            "--all-features",
            "-q",
            "--",
            "-D",
            "warnings",
            "-D",
            # Prevent UTF-8 panics from byte-based string slicing
            "clippy::string_slice",
        ],
        cwd=server_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
    if fmt_result.stdout:
        print(fmt_result.stdout, end="")
    if fmt_result.stderr:
        print(fmt_result.stderr, end="", file=sys.stderr)
    if clippy_result.stdout:
        print(clippy_result.stdout, end="")
    if clippy_result.stderr:
        print(clippy_result.stderr, end="", file=sys.stderr)

    success = fmt_result.returncode == 0 and clippy_result.returncode == 0
    return ("Rust (server)", success)


def lint_rust_cli(project_root: Path) -> tuple[str, bool]:
    """Lint Rust CLI code."""
    cli_dir = project_root / "cli"

    # Run fmt
    fmt_result = subprocess.run(
        ["cargo", "fmt", "--all"],
        cwd=cli_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Run clippy
    clippy_result = subprocess.run(
        [
            "cargo",
            "clippy",
            "--all-targets",
            "--all-features",
            "-q",
            "--",
            "-D",
            "warnings",
            "-D",
            # Prevent UTF-8 panics from byte-based string slicing
            "clippy::string_slice",
        ],
        cwd=cli_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
    if fmt_result.stdout:
        print(fmt_result.stdout, end="")
    if fmt_result.stderr:
        print(fmt_result.stderr, end="", file=sys.stderr)
    if clippy_result.stdout:
        print(clippy_result.stdout, end="")
    if clippy_result.stderr:
        print(clippy_result.stderr, end="", file=sys.stderr)

    success = fmt_result.returncode == 0 and clippy_result.returncode == 0
    return ("Rust (cli)", success)


def lint_rust_core(project_root: Path) -> tuple[str, bool]:
    """Lint Rust ramekin-core crate."""
    crate_dir = project_root / "ramekin-core"

    # Run fmt
    fmt_result = subprocess.run(
        ["cargo", "fmt", "--all"],
        cwd=crate_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Run clippy
    clippy_result = subprocess.run(
        [
            "cargo",
            "clippy",
            "--all-targets",
            "--all-features",
            "-q",
            "--",
            "-D",
            "warnings",
            "-D",
            # Prevent UTF-8 panics from byte-based string slicing
            "clippy::string_slice",
        ],
        cwd=crate_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
    if fmt_result.stdout:
        print(fmt_result.stdout, end="")
    if fmt_result.stderr:
        print(fmt_result.stderr, end="", file=sys.stderr)
    if clippy_result.stdout:
        print(clippy_result.stdout, end="")
    if clippy_result.stderr:
        print(clippy_result.stderr, end="", file=sys.stderr)

    success = fmt_result.returncode == 0 and clippy_result.returncode == 0
    return ("Rust (core)", success)


def lint_rust_ingredient_density(project_root: Path) -> tuple[str, bool]:
    """Lint Rust ingredient-density crate."""
    crate_dir = project_root / "ingredient-density"

    # Run fmt
    fmt_result = subprocess.run(
        ["cargo", "fmt", "--all"],
        cwd=crate_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Run clippy
    clippy_result = subprocess.run(
        [
            "cargo",
            "clippy",
            "--all-targets",
            "--all-features",
            "-q",
            "--",
            "-D",
            "warnings",
            "-D",
            # Prevent UTF-8 panics from byte-based string slicing
            "clippy::string_slice",
        ],
        cwd=crate_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
    if fmt_result.stdout:
        print(fmt_result.stdout, end="")
    if fmt_result.stderr:
        print(fmt_result.stderr, end="", file=sys.stderr)
    if clippy_result.stdout:
        print(clippy_result.stdout, end="")
    if clippy_result.stderr:
        print(clippy_result.stderr, end="", file=sys.stderr)

    success = fmt_result.returncode == 0 and clippy_result.returncode == 0
    return ("Rust (ingredient-density)", success)


def ensure_ui_node_modules(ui_dir: Path) -> bool:
    """Install UI dependencies using npx when node_modules is missing."""
    required_paths = [
        ui_dir / "node_modules" / "prettier",
        ui_dir / "node_modules" / "typescript",
        ui_dir / "node_modules" / "jspdf",
        ui_dir / "node_modules" / "fflate",
    ]

    if all(path.exists() for path in required_paths):
        return True

    install_result = subprocess.run(
        ["npx", "--yes", "-p", "npm@latest", "npm", "ci", "--silent"],
        cwd=ui_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    if install_result.stdout:
        print(install_result.stdout, end="")
    if install_result.stderr:
        print(install_result.stderr, end="", file=sys.stderr)

    return install_result.returncode == 0


def lint_typescript(project_root: Path) -> tuple[str, bool]:
    """Lint TypeScript code."""
    ui_dir = project_root / "ramekin-ui"

    if not ensure_ui_node_modules(ui_dir):
        return ("TypeScript", False)

    # Run prettier
    prettier_result = subprocess.run(
        [
            "npx",
            "--yes",
            "-p",
            "npm@latest",
            "npm",
            "exec",
            "--",
            "prettier",
            "--write",
            "--log-level",
            "warn",
            "src/",
        ],
        cwd=ui_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Run tsc
    tsc_result = subprocess.run(
        [
            "npx",
            "--yes",
            "-p",
            "npm@latest",
            "npm",
            "exec",
            "--",
            "tsc",
            "-p",
            "tsconfig.app.json",
            "--noEmit",
        ],
        cwd=ui_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
    if prettier_result.stdout:
        print(prettier_result.stdout, end="")
    if prettier_result.stderr:
        print(prettier_result.stderr, end="", file=sys.stderr)
    if tsc_result.stdout:
        print(tsc_result.stdout, end="")
    if tsc_result.stderr:
        print(tsc_result.stderr, end="", file=sys.stderr)

    success = prettier_result.returncode == 0 and tsc_result.returncode == 0
    return ("TypeScript", success)


def lint_css(project_root: Path) -> tuple[str, bool]:
    """Lint CSS files with Stylelint."""
    ui_dir = project_root / "ramekin-ui"

    if not ensure_ui_node_modules(ui_dir):
        return ("CSS", False)

    result = subprocess.run(
        [
            "npx",
            "--yes",
            "-p",
            "npm@latest",
            "npm",
            "exec",
            "--",
            "stylelint",
            "--fix",
            "src/**/*.css",
        ],
        cwd=ui_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    return ("CSS", result.returncode == 0)


def lint_swift(project_root: Path) -> tuple[str, bool]:
    """Lint Swift code with SwiftLint."""
    ios_dir = project_root / "ramekin-ios"

    # Check if swiftlint is installed
    which_result = subprocess.run(
        ["which", "swiftlint"],
        capture_output=True,
        check=False,
    )
    if which_result.returncode != 0:
        print("swiftlint not installed (brew install swiftlint)", file=sys.stderr)
        return ("Swift", False)

    # On macOS, ensure DEVELOPER_DIR points to Xcode.app if available
    # (swiftlint needs SourceKit which isn't in CommandLineTools)
    env = os.environ.copy()
    xcode_path = Path("/Applications/Xcode.app/Contents/Developer")
    if xcode_path.exists():
        env["DEVELOPER_DIR"] = str(xcode_path)

    result = subprocess.run(
        ["swiftlint", "--quiet", "--strict"],
        cwd=ios_dir,
        capture_output=True,
        text=True,
        check=False,
        env=env,
    )

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    return ("Swift", result.returncode == 0)


def lint_python(project_root: Path) -> tuple[str, bool]:
    """Lint Python code."""
    # Run ruff format
    format_result = subprocess.run(
        [
            "uvx",
            "ruff",
            "format",
            "--quiet",
            "--exclude",
            "tests/generated",
            "tests/",
            "scripts/",
        ],
        cwd=project_root,
        capture_output=True,
        text=True,
        check=False,
    )

    # Run ruff check
    check_result = subprocess.run(
        [
            "uvx",
            "ruff",
            "check",
            "--fix",
            "--quiet",
            "--exclude",
            "tests/generated",
            "tests/",
            "scripts/",
        ],
        cwd=project_root,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
    if format_result.stdout:
        print(format_result.stdout, end="")
    if format_result.stderr:
        print(format_result.stderr, end="", file=sys.stderr)
    if check_result.stdout:
        print(check_result.stdout, end="")
    if check_result.stderr:
        print(check_result.stderr, end="", file=sys.stderr)

    success = format_result.returncode == 0 and check_result.returncode == 0
    return ("Python", success)


def lint_yaml(project_root: Path) -> tuple[str, bool]:
    """Lint YAML files."""
    yaml_files = [
        "process-compose.yaml",
        *project_root.glob(".github/**/*.yml"),
    ]

    result = subprocess.run(
        [
            "uvx",
            "yamllint",
            "--strict",
            "-d",
            "{extends: default, rules: {line-length: {max: 120}}}",
            *yaml_files,
        ],
        cwd=project_root,
        capture_output=True,
        text=True,
        check=False,
    )

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    return ("YAML", result.returncode == 0)


def lint_shell(project_root: Path) -> tuple[str, bool]:
    """Lint shell scripts with shellcheck."""
    # Check if shellcheck is installed
    which_result = subprocess.run(
        ["which", "shellcheck"],
        capture_output=True,
        check=False,
    )
    if which_result.returncode != 0:
        print("shellcheck not installed (apt install shellcheck)", file=sys.stderr)
        return ("Shell", False)

    scripts_dir = project_root / "scripts"
    shell_scripts = list(scripts_dir.glob("*.sh")) + [scripts_dir / "pre-push"]

    result = subprocess.run(
        ["shellcheck", *[str(s) for s in shell_scripts]],
        cwd=project_root,
        capture_output=True,
        text=True,
        check=False,
    )

    if result.stdout:
        print(result.stdout, end="")
    if result.stderr:
        print(result.stderr, end="", file=sys.stderr)

    return ("Shell", result.returncode == 0)


def check_raw_sql(project_root: Path) -> tuple[str, bool]:
    """Check for raw SQL usage that could be vulnerable to SQL injection.

    Uses ast-grep for proper AST-based detection of raw SQL patterns:
    - sql_query() - runs arbitrary SQL strings
    - sql::<Type>() - creates raw SQL fragments
    - .sql() - appends raw SQL to queries

    Approved exceptions must be listed in scripts/sql_allowlist.txt.
    """
    # Check if ast-grep is installed
    which_result = subprocess.run(
        ["which", "ast-grep"],
        capture_output=True,
        check=False,
    )
    if which_result.returncode != 0:
        print("ast-grep not installed (cargo install ast-grep)", file=sys.stderr)
        return ("Raw SQL check", False)

    # The designated module for all raw SQL - see server/src/raw_sql.rs for safety docs
    allowed_files = {"server/src/raw_sql.rs"}

    # Run ast-grep with the rule file
    rule_file = project_root / "scripts" / "raw-sql-rules.yml"
    result = subprocess.run(
        [
            "ast-grep",
            "scan",
            "--rule",
            str(rule_file),
            "--json",
            "server/",
            "cli/",
            "ingredient-density/",
        ],
        cwd=project_root,
        capture_output=True,
        text=True,
        check=False,
    )

    # Parse JSON output to get matches
    violations: dict[str, tuple[str, int, str]] = {}
    if result.stdout.strip():
        try:
            matches = json.loads(result.stdout)
            for match in matches:
                file_path = match.get("file", "")
                # Make path relative to project root
                if file_path.startswith(str(project_root)):
                    file_path = file_path[len(str(project_root)) + 1 :]
                line_num = match.get("range", {}).get("start", {}).get("line", 0)
                # ast-grep uses 0-indexed lines, convert to 1-indexed
                line_num += 1
                text = match.get("text", "").split("\n")[0].strip()

                location = f"{file_path}:{line_num}"
                if file_path not in allowed_files and location not in violations:
                    violations[location] = (file_path, line_num, text)
        except json.JSONDecodeError:
            print(f"Failed to parse ast-grep output: {result.stdout}", file=sys.stderr)
            return ("Raw SQL check", False)

    if violations:
        print("Raw SQL detected (potential SQL injection risk):", file=sys.stderr)
        print("", file=sys.stderr)
        for file_path, line_num, text in violations.values():
            print(f"  {file_path}:{line_num}", file=sys.stderr)
            print(f"    {text}", file=sys.stderr)
        print("", file=sys.stderr)
        print("Use Diesel's type-safe DSL instead of raw SQL.", file=sys.stderr)
        print(
            "If raw SQL is unavoidable, add the location to scripts/sql_allowlist.txt",
            file=sys.stderr,
        )
        print(
            "after security review (ensure all user input uses .bind()).",
            file=sys.stderr,
        )
        return ("Raw SQL check", False)

    return ("Raw SQL check", True)


def lint_issues(project_root: Path) -> tuple[str, bool]:
    """Validate issue JSON5 files in issues/ directory."""
    issue_lint = shutil.which("issue-lint")
    if issue_lint is not None:
        return run_command("Issues", [issue_lint, str(project_root)])

    return _lint_issues_fallback(project_root)


ISSUE_REQUIRED_FIELDS = {
    "title",
    "description",
    "status",
    "priority",
    "type",
    "labels",
    "created_at",
    "updated_at",
}
ISSUE_OPTIONAL_FIELDS = {"blocked"}
ISSUE_KNOWN_FIELDS = ISSUE_REQUIRED_FIELDS | ISSUE_OPTIONAL_FIELDS
ISSUE_FILENAME_RE = re.compile(r"^(p[1-4]|blocked)-[a-z0-9][a-z0-9-]*$")


def _expected_issue_prefix(issue: dict) -> str:
    return "blocked" if issue.get("blocked") else f"p{issue['priority']}"


def _lint_issues_fallback(project_root: Path) -> tuple[str, bool]:
    """Fallback issue validation when shared tooling is unavailable."""
    issues_dir = project_root / "issues"
    if not issues_dir.exists():
        return ("Issues", True)

    errors = [
        f"{legacy_issue_file.name}: legacy issue file extension; rename to .json5"
        for legacy_issue_file in sorted(issues_dir.glob("*.json"))
    ]

    for issue_file in sorted(issues_dir.glob("*.json5")):
        try:
            text = issue_file.read_text()
            # Strip JSON5 features so stdlib json can parse:
            # line continuations (backslash-newline) and trailing commas
            text = text.replace("\\\n", "")
            text = re.sub(r",(\s*[}\]])", r"\1", text)
            issue = json.loads(text)
        except (json.JSONDecodeError, ValueError) as e:
            errors.append(f"{issue_file.name}: invalid JSON5 - {e}")
            continue

        if "id" in issue:
            errors.append(f"{issue_file.name}: has 'id' field (filename serves as id)")

        missing = ISSUE_REQUIRED_FIELDS - set(issue.keys())
        if missing:
            errors.append(
                f"{issue_file.name}: missing fields: {', '.join(sorted(missing))}"
            )
            continue

        unknown = set(issue.keys()) - ISSUE_KNOWN_FIELDS
        if unknown:
            errors.append(
                f"{issue_file.name}: unknown fields: {', '.join(sorted(unknown))}"
            )

        if not ISSUE_FILENAME_RE.fullmatch(issue_file.stem):
            errors.append(
                f"{issue_file.name}: filename must start with "
                "p1-/p2-/p3-/p4-/blocked- and use kebab-case"
            )
        else:
            expected_prefix = _expected_issue_prefix(issue)
            actual_prefix = issue_file.stem.split("-", 1)[0]
            if actual_prefix != expected_prefix:
                errors.append(
                    f"{issue_file.name}: filename prefix must be "
                    f"'{expected_prefix}-' for this issue"
                )

        if issue["status"] != "open":
            errors.append(
                f"{issue_file.name}: status is '{issue['status']}' "
                "(delete resolved issues)"
            )

        if not isinstance(issue["priority"], int) or not 1 <= issue["priority"] <= 4:
            errors.append(
                f"{issue_file.name}: priority must be int 1-4, got {issue['priority']}"
            )

        if not isinstance(issue["labels"], list):
            errors.append(f"{issue_file.name}: labels must be an array")

        if "blocked" in issue and not isinstance(issue["blocked"], (bool, str)):
            errors.append(f"{issue_file.name}: blocked must be a boolean or string")

    if errors:
        print("Issue validation errors:", file=sys.stderr)
        for error in errors:
            print(f"  {error}", file=sys.stderr)
        return ("Issues", False)

    return ("Issues", True)


def run_with_timing(name: str, func: callable) -> tuple[str, bool, float]:
    """Run a linter function and return (name, success, elapsed_seconds)."""

    start = time.monotonic()
    _, success = func()
    elapsed = time.monotonic() - start
    return (name, success, elapsed)


def main() -> None:
    """Main execution."""

    overall_start = time.monotonic()
    project_root = get_project_root()

    # Install UI node_modules before parallel linting to avoid race condition
    # (both lint_typescript and lint_css call ensure_ui_node_modules concurrently)
    ui_dir = project_root / "ramekin-ui"
    ensure_ui_node_modules(ui_dir)

    # Define linters to run
    linters = [
        ("Rust (server)", lambda: lint_rust_server(project_root)),
        ("Rust (cli)", lambda: lint_rust_cli(project_root)),
        ("Rust (core)", lambda: lint_rust_core(project_root)),
        (
            "Rust (ingredient-density)",
            lambda: lint_rust_ingredient_density(project_root),
        ),
        ("TypeScript", lambda: lint_typescript(project_root)),
        ("CSS", lambda: lint_css(project_root)),
        ("Swift", lambda: lint_swift(project_root)),
        ("Python", lambda: lint_python(project_root)),
        ("YAML", lambda: lint_yaml(project_root)),
        ("Shell", lambda: lint_shell(project_root)),
        ("Raw SQL check", lambda: check_raw_sql(project_root)),
        ("Issues", lambda: lint_issues(project_root)),
    ]

    # Run all linters in parallel with timing
    results: dict[str, bool] = {}
    timings: dict[str, float] = {}
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = {
            executor.submit(run_with_timing, name, func): name for name, func in linters
        }

        for future in as_completed(futures):
            name, success, elapsed = future.result()
            results[name] = success
            timings[name] = elapsed

    overall_elapsed = time.monotonic() - overall_start

    # Print timing summary (sorted by duration, slowest first)
    print("\nLinter timings:")
    for name, elapsed in sorted(timings.items(), key=lambda x: -x[1]):
        status = "✓" if results[name] else "✗"
        print(f"  {status} {name:20s} {elapsed:6.1f}s")
    print(f"  {'─' * 30}")
    print(f"  Total elapsed:       {overall_elapsed:6.1f}s (parallel)")

    # Check if all succeeded
    all_success = all(results.values())

    if all_success:
        print("\nLinted")
    else:
        print("\nLinting failed for:", file=sys.stderr)
        for name, success in results.items():
            if not success:
                print(f"  - {name}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
