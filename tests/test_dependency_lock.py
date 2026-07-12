import shutil
import subprocess
from pathlib import Path

import pytest


REPO_ROOT = Path(__file__).resolve().parents[1]

# Targets developers and CI run routinely. None of them may upgrade dependencies.
ROUTINE_TARGETS = ["test", "test-ui", "lint", "venv", "check-deps"]


def _dry_run_recipes(target: str, newer_file: str) -> str:
    """Return the recipes make would run for `target`, pretending `newer_file`
    was just modified.

    `--what-if` simulates an mtime without touching anything on disk. Marking a
    file this way makes it newer than every other file, so pass only the one
    file whose staleness is under test.
    """
    result = subprocess.run(
        [
            "make",
            "--dry-run",
            "--no-print-directory",
            f"--what-if={newer_file}",
            target,
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        check=True,
    )
    return result.stdout


def _run_check_deps(
    tmp_path: Path, declared: str, locked: str
) -> subprocess.CompletedProcess:
    """Run scripts/check-deps.sh against fabricated requirements files.

    The script derives its project root from its own location, so copying it
    into tmp_path is what lets us hand it a stale lockfile.
    """
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    script = scripts / "check-deps.sh"
    shutil.copy(REPO_ROOT / "scripts" / "check-deps.sh", script)
    (tmp_path / "requirements-test.txt").write_text(declared, encoding="utf-8")
    (tmp_path / "requirements-test.lock").write_text(locked, encoding="utf-8")

    return subprocess.run(
        [str(script), "--lockfile"], capture_output=True, text=True, check=False
    )


@pytest.mark.parametrize("target", ROUTINE_TARGETS)
def test_routine_targets_never_regenerate_the_lockfile(target):
    # A fresh `git checkout` does not preserve mtimes, so requirements-test.txt
    # routinely lands on disk newer than the lockfile it was compiled into.
    recipes = _dry_run_recipes(target, "requirements-test.txt")

    assert "uv pip compile" not in recipes, (
        f"`make {target}` would regenerate requirements-test.lock, so an ordinary "
        "run can silently upgrade pinned dependencies. Only "
        "`make python-test-deps-update` may compile the lock."
    )


def test_no_rule_regenerates_the_lockfile():
    # Stronger than the routine targets above: asking make to build the lock
    # itself proves no rule anywhere can rebuild it from requirements-test.txt,
    # including targets added after this test was written.
    recipes = _dry_run_recipes("requirements-test.lock", "requirements-test.txt")

    assert "uv pip compile" not in recipes


def test_test_target_installs_from_the_committed_lockfile():
    recipes = _dry_run_recipes("test", "requirements-test.lock")

    assert "uv pip sync requirements-test.lock" in recipes


def test_dependency_upgrades_require_the_explicit_update_target():
    recipes = _dry_run_recipes("python-test-deps-update", "requirements-test.txt")

    assert "uv pip compile --universal --upgrade requirements-test.txt" in recipes
    assert "--output-file requirements-test.lock" in recipes


@pytest.mark.parametrize("target", ROUTINE_TARGETS)
def test_routine_targets_check_the_lockfile_covers_declared_dependencies(target):
    recipes = _dry_run_recipes(target, "requirements-test.txt")

    assert "check-deps.sh --lockfile" in recipes


def test_the_lockfile_check_does_not_block_the_target_that_fixes_a_stale_lock():
    # The staleness check tells developers to run python-test-deps-update, so
    # that target must not depend on the check it exists to satisfy — otherwise
    # a stale lock blocks its own remedy.
    recipes = _dry_run_recipes("python-test-deps-update", "requirements-test.txt")

    assert "check-deps.sh --lockfile" not in recipes


def test_check_deps_accepts_a_lockfile_pinning_every_declared_dependency(tmp_path):
    result = _run_check_deps(
        tmp_path,
        declared="pytest\nPython_Dateutil\n",
        locked="pytest==8.4.2\npython-dateutil==2.9.0\n",
    )

    assert result.returncode == 0, result.stdout


def test_check_deps_rejects_a_lockfile_missing_a_declared_dependency(tmp_path):
    result = _run_check_deps(
        tmp_path,
        declared="pytest\npillow\n",
        locked="pytest==8.4.2\n",
    )

    assert result.returncode == 1
    assert "pillow" in result.stdout
    assert "make python-test-deps-update" in result.stdout
