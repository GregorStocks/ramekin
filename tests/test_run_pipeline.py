import os
import stat
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "run-pipeline.sh"


def _write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def test_run_pipeline_refuses_when_lock_is_held(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    marker_path = tmp_path / "cargo-called"
    lock_dir = tmp_path / "locks" / "pipeline-script-unit.lock"
    lock_dir.mkdir(parents=True)

    lock_holder = subprocess.Popen(["sleep", "60"])
    try:
        (lock_dir / "pid").write_text(f"{lock_holder.pid}\n", encoding="utf-8")

        _write_executable(
            bin_dir / "cargo",
            f"""#!/bin/bash
set -e
touch "{marker_path}"
exit 0
""",
        )
        _write_executable(
            bin_dir / "make",
            """#!/bin/bash
set -e
exit 0
""",
        )

        env = os.environ.copy()
        env["PATH"] = f"{bin_dir}:{env['PATH']}"
        env["REPO_LOCK_DIR"] = str(tmp_path / "locks")
        env["PIPELINE_LOCK_NAME"] = "pipeline-script-unit"

        result = subprocess.run(
            ["bash", str(SCRIPT_PATH)],
            cwd=REPO_ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
    finally:
        lock_holder.terminate()
        lock_holder.wait(timeout=5)

    assert result.returncode == 1
    assert "Refusing to start pipeline run" in result.stderr
    assert "another pipeline run is already running" in result.stderr
    assert not marker_path.exists()


def test_run_pipeline_default_lock_excludes_other_top_level_runs(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    marker_path = tmp_path / "cargo-called"
    lock_dir = tmp_path / "locks" / "top-level-run.lock"
    lock_dir.mkdir(parents=True)

    lock_holder = subprocess.Popen(["sleep", "60"])
    try:
        (lock_dir / "pid").write_text(f"{lock_holder.pid}\n", encoding="utf-8")

        _write_executable(
            bin_dir / "cargo",
            f"""#!/bin/bash
set -e
touch "{marker_path}"
exit 0
""",
        )
        _write_executable(
            bin_dir / "make",
            """#!/bin/bash
set -e
exit 0
""",
        )

        env = os.environ.copy()
        env["PATH"] = f"{bin_dir}:{env['PATH']}"
        env["REPO_LOCK_DIR"] = str(tmp_path / "locks")

        result = subprocess.run(
            ["bash", str(SCRIPT_PATH)],
            cwd=REPO_ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
    finally:
        lock_holder.terminate()
        lock_holder.wait(timeout=5)

    assert result.returncode == 1
    assert "Refusing to start pipeline run" in result.stderr
    assert "another top-level run is already running" in result.stderr
    assert not marker_path.exists()


def test_run_pipeline_removes_stale_lock_and_runs(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    cargo_marker = tmp_path / "cargo-called"
    make_marker = tmp_path / "make-called"
    lock_dir = tmp_path / "locks" / "pipeline-script-unit.lock"
    lock_dir.mkdir(parents=True)
    (lock_dir / "pid").write_text("999999\n", encoding="utf-8")

    _write_executable(
        bin_dir / "cargo",
        f"""#!/bin/bash
set -e
touch "{cargo_marker}"
exit 0
""",
    )
    _write_executable(
        bin_dir / "make",
        f"""#!/bin/bash
set -e
touch "{make_marker}"
exit 0
""",
    )

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["REPO_LOCK_DIR"] = str(tmp_path / "locks")
    env["PIPELINE_LOCK_NAME"] = "pipeline-script-unit"

    result = subprocess.run(
        ["bash", str(SCRIPT_PATH), "--concurrency", "2"],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0
    assert "Removing stale pipeline run lock" in result.stderr
    assert cargo_marker.exists()
    assert make_marker.exists()
    assert not lock_dir.exists()


def test_run_pipeline_refuses_pidless_lock_within_startup_grace(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    marker_path = tmp_path / "cargo-called"
    lock_dir = tmp_path / "locks" / "pipeline-script-unit.lock"
    lock_dir.mkdir(parents=True)

    _write_executable(
        bin_dir / "cargo",
        f"""#!/bin/bash
set -e
touch "{marker_path}"
exit 0
""",
    )
    _write_executable(
        bin_dir / "make",
        """#!/bin/bash
set -e
exit 0
""",
    )

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["REPO_LOCK_DIR"] = str(tmp_path / "locks")
    env["PIPELINE_LOCK_NAME"] = "pipeline-script-unit"
    env["REPO_LOCK_STARTUP_GRACE_SECONDS"] = "60"

    result = subprocess.run(
        ["bash", str(SCRIPT_PATH)],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 1
    assert "still acquiring the lock" in result.stderr
    assert not marker_path.exists()
    assert lock_dir.exists()


def test_run_pipeline_refuses_empty_pid_lock_within_startup_grace(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    marker_path = tmp_path / "cargo-called"
    lock_dir = tmp_path / "locks" / "pipeline-script-unit.lock"
    lock_dir.mkdir(parents=True)
    (lock_dir / "pid").write_text("", encoding="utf-8")

    _write_executable(
        bin_dir / "cargo",
        f"""#!/bin/bash
set -e
touch "{marker_path}"
exit 0
""",
    )
    _write_executable(
        bin_dir / "make",
        """#!/bin/bash
set -e
exit 0
""",
    )

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["REPO_LOCK_DIR"] = str(tmp_path / "locks")
    env["PIPELINE_LOCK_NAME"] = "pipeline-script-unit"
    env["REPO_LOCK_STARTUP_GRACE_SECONDS"] = "60"

    result = subprocess.run(
        ["bash", str(SCRIPT_PATH)],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 1
    assert "still acquiring the lock" in result.stderr
    assert not marker_path.exists()
    assert lock_dir.exists()
