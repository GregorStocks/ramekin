import os
import stat
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "run-tests.sh"


def _write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def test_run_tests_fails_when_a_test_process_failed_but_process_compose_exits_zero(
    tmp_path,
):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    log_line = "synthetic hidden rust test failure"
    env_file = tmp_path / "test.env"
    log_path = tmp_path / "isolated-test.log"
    status_dir = tmp_path / "isolated-status"
    env_file.write_text("PROCESS_COMPOSE_PORT=4317\n", encoding="utf-8")

    _write_executable(
        bin_dir / "process-compose",
        f"""#!/bin/bash
set -e

if [ "$1" = "up" ]; then
  mkdir -p "$TEST_STATUS_DIR"
  printf '%s\\n' "{log_line}" > "$TEST_LOG_FILE"
  printf '%s\\n' 0 > "$TEST_STATUS_DIR/rust-tests-server.exit"
  printf '%s\\n' 0 > "$TEST_STATUS_DIR/rust-tests-cli.exit"
  printf '%s\\n' 1 > "$TEST_STATUS_DIR/rust-tests-core.exit"
  printf '%s\\n' 0 > "$TEST_STATUS_DIR/api-tests.exit"
  exit 0
fi

exit 1
""",
    )

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["TEST_LOG_FILE"] = str(log_path)
    env["TEST_STATUS_DIR"] = str(status_dir)
    env["TEST_LOCK_NAME"] = "tests-script-failure-unit"

    result = subprocess.run(
        ["bash", str(SCRIPT_PATH)],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 1
    assert "One or more test processes failed:" in result.stdout
    assert "rust-tests-core (exit_code=1)" in result.stdout
    assert f"Last 200 lines of {log_path}:" in result.stdout
    assert log_line in result.stdout


def test_run_tests_refuses_when_lock_is_held(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    marker_path = tmp_path / "process-compose-called"
    env_file = tmp_path / "test.env"
    env_file.write_text("PROCESS_COMPOSE_PORT=4317\n", encoding="utf-8")
    lock_dir = tmp_path / "locks" / "tests-script-lock-held-unit.lock"
    lock_dir.mkdir(parents=True)

    lock_holder = subprocess.Popen(["sleep", "60"])
    try:
        (lock_dir / "pid").write_text(f"{lock_holder.pid}\n", encoding="utf-8")

        _write_executable(
            bin_dir / "process-compose",
            f"""#!/bin/bash
set -e
touch "{marker_path}"
exit 0
""",
        )

        env = os.environ.copy()
        env["PATH"] = f"{bin_dir}:{env['PATH']}"
        env["TEST_ENV_FILE"] = str(env_file)
        env["REPO_LOCK_DIR"] = str(tmp_path / "locks")
        env["TEST_LOCK_NAME"] = "tests-script-lock-held-unit"

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
    assert "Refusing to start test run" in result.stderr
    assert "another test run is already running" in result.stderr
    assert not marker_path.exists()


def test_run_tests_default_lock_excludes_other_top_level_runs(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    marker_path = tmp_path / "process-compose-called"
    env_file = tmp_path / "test.env"
    env_file.write_text("PROCESS_COMPOSE_PORT=4317\n", encoding="utf-8")
    lock_dir = tmp_path / "locks" / "top-level-run.lock"
    lock_dir.mkdir(parents=True)

    lock_holder = subprocess.Popen(["sleep", "60"])
    try:
        (lock_dir / "pid").write_text(f"{lock_holder.pid}\n", encoding="utf-8")

        _write_executable(
            bin_dir / "process-compose",
            f"""#!/bin/bash
set -e
touch "{marker_path}"
exit 0
""",
        )

        env = os.environ.copy()
        env["PATH"] = f"{bin_dir}:{env['PATH']}"
        env["TEST_ENV_FILE"] = str(env_file)
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
    assert "Refusing to start test run" in result.stderr
    assert "another top-level run is already running" in result.stderr
    assert not marker_path.exists()


def test_run_tests_waits_for_status_files_before_failing(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    env_file = tmp_path / "test.env"
    log_path = tmp_path / "isolated-test.log"
    status_dir = tmp_path / "isolated-status"
    env_file.write_text("PROCESS_COMPOSE_PORT=4317\n", encoding="utf-8")

    _write_executable(
        bin_dir / "process-compose",
        """#!/bin/bash
set -e

if [ "$1" = "up" ]; then
  mkdir -p "$TEST_STATUS_DIR"
  printf '%s\\n' "delayed status flush" > "$TEST_LOG_FILE"
  (
    sleep 1
    printf '%s\\n' 0 > "$TEST_STATUS_DIR/rust-tests-server.exit"
    printf '%s\\n' 0 > "$TEST_STATUS_DIR/rust-tests-cli.exit"
    printf '%s\\n' 0 > "$TEST_STATUS_DIR/rust-tests-core.exit"
    printf '%s\\n' 0 > "$TEST_STATUS_DIR/api-tests.exit"
  ) &
  exit 0
fi

exit 1
""",
    )

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["TEST_LOG_FILE"] = str(log_path)
    env["TEST_STATUS_DIR"] = str(status_dir)
    env["TEST_LOCK_NAME"] = "tests-script-status-unit"
    env["TEST_STATUS_WAIT_SECONDS"] = "5"

    result = subprocess.run(
        ["bash", str(SCRIPT_PATH)],
        cwd=REPO_ROOT,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 0
    assert "Waiting for test status files to flush..." in result.stdout
    assert "One or more test processes failed:" not in result.stdout
