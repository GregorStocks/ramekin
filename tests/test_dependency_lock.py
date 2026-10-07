import os
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
    tmp_path: Path, declared: str, locked: str, path: str | None = None
) -> subprocess.CompletedProcess:
    """Run scripts/check-deps.sh --lockfile against fabricated requirements files.

    The script derives its project root from its own location, so copying it
    into tmp_path is what lets us hand it a stale lockfile.
    """
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    script = scripts / "check-deps.sh"
    shutil.copy(REPO_ROOT / "scripts" / "check-deps.sh", script)
    (tmp_path / "requirements-test.txt").write_text(declared, encoding="utf-8")
    (tmp_path / "requirements-test.lock").write_text(locked, encoding="utf-8")

    env = None if path is None else {**os.environ, "PATH": path}
    return subprocess.run(
        [str(script), "--lockfile"],
        capture_output=True,
        text=True,
        check=False,
        env=env,
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


# uv marks each direct dependency with a `-r requirements-test.txt` via-comment,
# and lists transitive ones under the package that pulled them in.
LOCKED_PYTEST_AND_REQUESTS = """\
certifi==2026.6.17
    # via requests
pytest==8.4.2
    # via -r requirements-test.txt
requests==2.32.5
    # via -r requirements-test.txt
"""


def test_check_deps_accepts_a_lockfile_matching_the_declared_dependencies(tmp_path):
    result = _run_check_deps(
        tmp_path,
        declared="pytest\nRequests\n",
        locked=LOCKED_PYTEST_AND_REQUESTS,
    )

    assert result.returncode == 0, result.stdout


def test_check_deps_rejects_a_lockfile_missing_a_declared_dependency(tmp_path):
    result = _run_check_deps(
        tmp_path,
        declared="pytest\nrequests\npillow\n",
        locked=LOCKED_PYTEST_AND_REQUESTS,
    )

    assert result.returncode == 1
    assert "does not pin pillow" in result.stdout
    assert "make python-test-deps-update" in result.stdout


def test_check_deps_rejects_a_lockfile_still_pinning_a_removed_dependency(tmp_path):
    # `uv pip sync` installs whatever the lock names, so a dropped dependency
    # would keep being installed and CI would pass without it being declared.
    result = _run_check_deps(
        tmp_path,
        declared="pytest\n",
        locked=LOCKED_PYTEST_AND_REQUESTS,
    )

    assert result.returncode == 1
    assert "still pins requests" in result.stdout


def test_check_deps_rejects_a_dependency_only_present_transitively(tmp_path):
    # certifi is in the lock, but only because requests pulled it in -- it is not
    # a requirement the lock was compiled from, so declaring it must not pass.
    result = _run_check_deps(
        tmp_path,
        declared="pytest\nrequests\ncertifi\n",
        locked=LOCKED_PYTEST_AND_REQUESTS,
    )

    assert result.returncode == 1
    assert "does not pin certifi" in result.stdout


@pytest.mark.parametrize("constrained", ["pytest>=10", "requests[socks]", "urllib3<3"])
def test_check_deps_rejects_a_constrained_requirement(tmp_path, constrained):
    # `uv pip sync` reads only the lock, so a specifier or extra here would go
    # unhonoured: `pytest>=10` would silently keep the older pinned pytest.
    result = _run_check_deps(
        tmp_path,
        declared=f"pytest\nrequests\n{constrained}\n",
        locked=LOCKED_PYTEST_AND_REQUESTS,
    )

    assert result.returncode == 1
    assert constrained in result.stdout


def test_lockfile_mode_needs_no_dev_tooling(tmp_path):
    # It gates the venv build, so it has to work before cargo/npm/ast-grep exist.
    result = _run_check_deps(
        tmp_path,
        declared="pytest\nrequests\n",
        locked=LOCKED_PYTEST_AND_REQUESTS,
        path="/usr/bin:/bin",
    )

    assert result.returncode == 0, result.stdout


# Stands in for `npx ... npm ci`: flags any overlapping install, then writes
# the marker npm leaves behind.
FAKE_NPX = """#!/bin/bash
mkdir ../install-running 2>/dev/null || echo overlap >> ../overlaps
echo install >> ../installs
sleep 1
mkdir -p node_modules && touch node_modules/.package-lock.json
rmdir ../install-running
"""


def _ui_deps_sandbox(tmp_path: Path) -> dict[str, str]:
    """Lay out a fake project for scripts/install-ui-deps.sh and return the env
    that puts the fake npx on PATH."""
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in ["install-ui-deps.sh", "repo-lock.sh"]:
        shutil.copy(REPO_ROOT / "scripts" / name, scripts / name)
    ui = tmp_path / "ramekin-ui"
    ui.mkdir()
    (ui / "package.json").write_text("{}", encoding="utf-8")
    (ui / "package-lock.json").write_text("{}", encoding="utf-8")
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    npx = bin_dir / "npx"
    npx.write_text(FAKE_NPX, encoding="utf-8")
    npx.chmod(0o755)
    return {**os.environ, "PATH": f"{bin_dir}:{os.environ['PATH']}"}


def _install_ui_deps(tmp_path: Path, env: dict[str, str]) -> subprocess.Popen:
    return subprocess.Popen(
        [str(tmp_path / "scripts" / "install-ui-deps.sh")],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )


def _installs(tmp_path: Path) -> int:
    log = tmp_path / "installs"
    return len(log.read_text().splitlines()) if log.exists() else 0


def test_concurrent_ui_dep_installs_take_turns(tmp_path):
    # make lint and make test both pull in ui-deps; parallel npm ci runs
    # corrupt node_modules, and the one that waited has nothing left to do.
    env = _ui_deps_sandbox(tmp_path)

    procs = [_install_ui_deps(tmp_path, env) for _ in range(2)]
    outputs = [p.communicate(timeout=30)[0] for p in procs]

    assert [p.returncode for p in procs] == [0, 0], outputs
    assert not (tmp_path / "overlaps").exists()
    assert _installs(tmp_path) == 1
    assert not (tmp_path / "logs" / "locks" / "ui-deps.lock").exists()


def test_uncontended_ui_dep_install_runs_even_when_marker_is_fresh(tmp_path):
    # `make -W ramekin-ui/package-lock.json ui-deps` forces a reinstall without
    # touching mtimes, so only an install that waited may skip.
    env = _ui_deps_sandbox(tmp_path)
    marker = tmp_path / "ramekin-ui" / "node_modules" / ".package-lock.json"
    marker.parent.mkdir()
    marker.touch()

    proc = _install_ui_deps(tmp_path, env)
    output = proc.communicate(timeout=30)[0]

    assert proc.returncode == 0, output
    assert _installs(tmp_path) == 1


def test_ui_dep_install_clears_a_lock_left_by_a_dead_process(tmp_path):
    env = _ui_deps_sandbox(tmp_path)
    finished = subprocess.Popen(["true"])
    finished.wait()
    lock = tmp_path / "logs" / "locks" / "ui-deps.lock"
    lock.mkdir(parents=True)
    (lock / "pid").write_text(f"{finished.pid}\n", encoding="utf-8")

    proc = _install_ui_deps(tmp_path, env)
    output = proc.communicate(timeout=30)[0]

    assert proc.returncode == 0, output
    assert "Removing stale" in output
    assert _installs(tmp_path) == 1
