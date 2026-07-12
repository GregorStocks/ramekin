import os
import signal
import socket
import stat
import subprocess
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "run-tests.sh"
SERVICE_PORT_VARIABLES = (
    "PORT",
    "FIXTURE_PORT",
    "MOCK_OPENROUTER_PORT",
    "UI_PORT",
    "UI_PORT_HTTP",
    "PROCESS_COMPOSE_PORT",
)


def _write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def _script_env() -> dict[str, str]:
    env = os.environ.copy()
    for variable in SERVICE_PORT_VARIABLES:
        env.pop(variable, None)
    return env


def _wait_for_path(path: Path) -> None:
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if path.exists():
            return
        time.sleep(0.05)
    raise AssertionError(f"Timed out waiting for {path}")


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
  printf '%s\\n' 0 > "$TEST_STATUS_DIR/ui-unit-tests.exit"
  exit 0
fi

exit 1
""",
    )

    env = _script_env()
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

        env = _script_env()
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

        env = _script_env()
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
    printf '%s\\n' 0 > "$TEST_STATUS_DIR/ui-unit-tests.exit"
  ) &
  exit 0
fi

exit 1
""",
    )

    env = _script_env()
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


def test_run_tests_stops_process_compose_on_termination(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    calls_path = tmp_path / "process-compose-calls"
    started_path = tmp_path / "process-compose-started"
    stopped_path = tmp_path / "process-compose-stopped"
    env_file = tmp_path / "test.env"
    env_file.write_text("PROCESS_COMPOSE_PORT=4317\n", encoding="utf-8")

    _write_executable(
        bin_dir / "process-compose",
        f"""#!/bin/bash
set -e

printf '%s\\n' "$*" >> "{calls_path}"

if [ "$1" = "up" ]; then
  touch "{started_path}"
  while [ ! -f "{stopped_path}" ]; do
    sleep 1
  done
  exit 0
fi

if [ "$1" = "down" ]; then
  touch "{stopped_path}"
  exit 0
fi

exit 1
""",
    )

    env = _script_env()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["TEST_LOG_FILE"] = str(tmp_path / "isolated-test.log")
    env["TEST_STATUS_DIR"] = str(tmp_path / "isolated-status")
    env["TEST_LOCK_NAME"] = "tests-script-cleanup-unit"
    env["PROCESS_COMPOSE_PORT"] = "4317"

    proc = subprocess.Popen(
        ["bash", str(SCRIPT_PATH)],
        cwd=REPO_ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        _wait_for_path(started_path)
        proc.send_signal(signal.SIGTERM)
        stdout, stderr = proc.communicate(timeout=5)
    finally:
        if proc.poll() is None:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.communicate(timeout=5)

    assert proc.returncode == 143, (stdout, stderr)
    assert calls_path.read_text(encoding="utf-8").splitlines() == [
        f"up -e {env_file} -f test-compose.yaml -t=false --port 4317",
        "down --port 4317",
    ]


def test_run_tests_refuses_to_start_when_a_service_port_is_occupied(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    process_compose_called = tmp_path / "process-compose-called"
    env_file = tmp_path / "test.env"
    listener = socket.create_server(("127.0.0.1", 0))
    occupied_port = listener.getsockname()[1]
    env_file.write_text(
        f"PORT={occupied_port}\nPROCESS_COMPOSE_PORT=4317\n", encoding="utf-8"
    )

    _write_executable(
        bin_dir / "process-compose",
        f"""#!/bin/bash
set -e
touch "{process_compose_called}"
exit 0
""",
    )

    env = _script_env()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["TEST_LOCK_NAME"] = "tests-script-stale-port-unit"

    try:
        result = subprocess.run(
            ["bash", str(SCRIPT_PATH)],
            cwd=REPO_ROOT,
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
    finally:
        listener.close()

    assert result.returncode == 1
    assert f"API server port {occupied_port} is already in use" in result.stderr
    assert not process_compose_called.exists()


def test_run_tests_cleans_up_when_process_compose_is_terminated(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    orchestration_pid_path = tmp_path / "orchestration-pid"
    stopped_path = tmp_path / "process-compose-stopped"
    env_file = tmp_path / "test.env"
    env_file.write_text("PROCESS_COMPOSE_PORT=4317\n", encoding="utf-8")

    _write_executable(
        bin_dir / "process-compose",
        f"""#!/bin/bash
set -e

if [ "$1" = "up" ]; then
  printf '%s\n' "$$" > "{orchestration_pid_path}"
  trap 'exit 143' TERM
  while true; do
    sleep 0.1
  done
fi

if [ "$1" = "down" ]; then
  touch "{stopped_path}"
  exit 0
fi

exit 1
""",
    )

    env = _script_env()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["TEST_LOG_FILE"] = str(tmp_path / "isolated-test.log")
    env["TEST_STATUS_DIR"] = str(tmp_path / "isolated-status")
    env["TEST_LOCK_NAME"] = "tests-script-orchestrator-termination-unit"

    proc = subprocess.Popen(
        ["bash", str(SCRIPT_PATH)],
        cwd=REPO_ROOT,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        start_new_session=True,
    )
    try:
        _wait_for_path(orchestration_pid_path)
        os.kill(int(orchestration_pid_path.read_text(encoding="utf-8")), signal.SIGTERM)
        stdout, stderr = proc.communicate(timeout=5)
    finally:
        if proc.poll() is None:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.communicate(timeout=5)

    assert proc.returncode == 143, (stdout, stderr)
    assert stopped_path.exists()
