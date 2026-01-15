#!/usr/bin/env python3
"""
Run all linters in parallel.

This script runs:
- Rust formatters and linters (server and cli)
- TypeScript formatter and type checker
- Python formatter and linter
- YAML linter
- Shell script linter
"""

import re
import subprocess
import sys
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
        ],
        cwd=server_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
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
        ],
        cwd=cli_dir,
        capture_output=True,
        text=True,
        check=False,
    )

    # Print any output
    if clippy_result.stdout:
        print(clippy_result.stdout, end="")
    if clippy_result.stderr:
        print(clippy_result.stderr, end="", file=sys.stderr)

    success = fmt_result.returncode == 0 and clippy_result.returncode == 0
    return ("Rust (cli)", success)


def lint_typescript(project_root: Path) -> tuple[str, bool]:
    """Lint TypeScript code."""
    ui_dir = project_root / "ramekin-ui"

    # Run prettier
    prettier_result = subprocess.run(
        ["npx", "prettier", "--write", "--log-level", "warn", "ramekin-ui/src/"],
        cwd=project_root,
        capture_output=True,
        text=True,
        check=False,
    )

    # Ensure node_modules exists
    if not (ui_dir / "node_modules").exists():
        subprocess.run(
            ["npm", "install", "--silent"],
            cwd=ui_dir,
            capture_output=True,
            check=False,
        )

    # Run tsc
    tsc_result = subprocess.run(
        ["npx", "tsc", "-p", "tsconfig.app.json", "--noEmit"],
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
            "{extends: relaxed, rules: {line-length: {max: 120}}}",
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

    Flags uses of raw SQL patterns which bypass Diesel's type-safe DSL:
    - sql_query() - runs arbitrary SQL strings
    - sql::<Type>() - creates raw SQL fragments
    - .sql() - appends raw SQL to queries

    Approved exceptions must be listed in scripts/sql_allowlist.txt.
    """
    # Load allowlist
    allowlist_path = project_root / "scripts" / "sql_allowlist.txt"
    allowlist: set[str] = set()
    if allowlist_path.exists():
        for line in allowlist_path.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                allowlist.add(line)

    # Find all Rust files in server/ and cli/
    rust_files = list((project_root / "server").rglob("*.rs"))
    rust_files.extend((project_root / "cli").rglob("*.rs"))

    # Patterns that indicate raw SQL (potential injection risk)
    # These all allow arbitrary SQL strings that could be vulnerable if
    # user input is interpolated without using .bind()
    dangerous_patterns = [
        (re.compile(r"\bsql_query\s*\("), "sql_query()"),
        (re.compile(r"\bsql::<"), "sql::<Type>()"),
        (re.compile(r"\.sql\s*\("), ".sql()"),
    ]

    violations: list[tuple[Path, int, str, str]] = []

    for rust_file in rust_files:
        rel_path = rust_file.relative_to(project_root)
        content = rust_file.read_text()

        for line_num, line in enumerate(content.splitlines(), start=1):
            for pattern, pattern_name in dangerous_patterns:
                if pattern.search(line):
                    location = f"{rel_path}:{line_num}"
                    if location not in allowlist:
                        violations.append((rel_path, line_num, line.strip(), pattern_name))
                    break  # Only report each line once

    if violations:
        print("Raw SQL detected (potential SQL injection risk):", file=sys.stderr)
        print("", file=sys.stderr)
        for rel_path, line_num, line, pattern_name in violations:
            print(f"  {rel_path}:{line_num} [{pattern_name}]", file=sys.stderr)
            print(f"    {line}", file=sys.stderr)
        print("", file=sys.stderr)
        print("Use Diesel's type-safe DSL instead of raw SQL.", file=sys.stderr)
        print(
            "If raw SQL is unavoidable, add the location to scripts/sql_allowlist.txt",
            file=sys.stderr,
        )
        print("after security review (ensure all user input uses .bind()).", file=sys.stderr)
        return ("Raw SQL check", False)

    return ("Raw SQL check", True)


def main() -> None:
    """Main execution."""
    project_root = get_project_root()

    # Define linters to run
    linters = [
        ("Rust (server)", lambda: lint_rust_server(project_root)),
        ("Rust (cli)", lambda: lint_rust_cli(project_root)),
        ("TypeScript", lambda: lint_typescript(project_root)),
        ("Python", lambda: lint_python(project_root)),
        ("YAML", lambda: lint_yaml(project_root)),
        ("Shell", lambda: lint_shell(project_root)),
        ("Raw SQL check", lambda: check_raw_sql(project_root)),
    ]

    # Run all linters in parallel
    results = {}
    with ThreadPoolExecutor(max_workers=7) as executor:
        futures = {executor.submit(func): name for name, func in linters}

        for future in as_completed(futures):
            name, success = future.result()
            results[name] = success

    # Check if all succeeded
    all_success = all(results.values())

    if all_success:
        print("Linted")
    else:
        print("\nLinting failed for:", file=sys.stderr)
        for name, success in results.items():
            if not success:
                print(f"  - {name}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
