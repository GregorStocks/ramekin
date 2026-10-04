import os
import shutil
import signal
import socket
import stat
import subprocess
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SERVICE_PORT_VARIABLES = (
    "PORT",
    "FIXTURE_PORT",
    "MOCK_OPENROUTER_PORT",
    "UI_PORT",
    "UI_PORT_HTTP",
    "PROCESS_COMPOSE_PORT",
)


def _script_copy(tmp_path: Path) -> Path:
    """Copy the runner into a scratch repo so its logs/ is private to the test:
    the script cds to its repo root, and parallel tests share the real one."""
    scripts = tmp_path / "repo" / "scripts"
    scripts.mkdir(parents=True)
    for name in ("run-ui-tests.sh", "test-orchestration.sh"):
        shutil.copy2(REPO_ROOT / "scripts" / name, scripts / name)
    return scripts / "run-ui-tests.sh"


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


def test_run_ui_tests_surfaces_orchestration_logs_on_failure(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    marker_path = tmp_path / "playwright-called"
    log_line = "synthetic process-compose failure"
    env_file = tmp_path / "test.env"
    env_file.write_text("PROCESS_COMPOSE_PORT=4318\nUI_PORT=4173\n", encoding="utf-8")

    _write_executable(
        bin_dir / "playwright",
        f"""#!/bin/bash
set -e
touch "{marker_path}"
exit 0
""",
    )
    _write_executable(
        bin_dir / "process-compose",
        f"""#!/bin/bash
set -e
mkdir -p logs
printf '%s\\n' "{log_line}" > logs/test-ui.log
exit 7
""",
    )

    env = _script_env()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["PROCESS_COMPOSE_PORT"] = "4318"
    env.pop("CI", None)

    result = subprocess.run(
        ["bash", str(_script_copy(tmp_path))],
        cwd=tmp_path / "repo",
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )

    assert marker_path.exists()
    assert result.returncode == 7
    assert (
        "UI test orchestration failed. Last 200 lines of logs/test-ui.log:"
        in result.stdout
    )
    assert log_line in result.stdout


def test_run_ui_tests_stops_process_compose_on_termination(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    calls_path = tmp_path / "process-compose-calls"
    started_path = tmp_path / "process-compose-started"
    stopped_path = tmp_path / "process-compose-stopped"
    env_file = tmp_path / "test.env"
    env_file.write_text("PROCESS_COMPOSE_PORT=4318\nUI_PORT=4173\n", encoding="utf-8")

    _write_executable(
        bin_dir / "playwright",
        """#!/bin/bash
set -e
exit 0
""",
    )
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
    env["PROCESS_COMPOSE_PORT"] = "4318"
    env.pop("CI", None)

    proc = subprocess.Popen(
        ["bash", str(_script_copy(tmp_path))],
        cwd=tmp_path / "repo",
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
        f"up -e {env_file} -f test-ui-compose.yaml -t=false --port 4318",
        "down --port 4318",
    ]


def test_run_ui_tests_refuses_to_start_when_a_service_port_is_occupied(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    playwright_called = tmp_path / "playwright-called"
    process_compose_called = tmp_path / "process-compose-called"
    env_file = tmp_path / "test.env"
    listener = socket.create_server(("127.0.0.1", 0))
    occupied_port = listener.getsockname()[1]
    env_file.write_text(
        f"PORT={occupied_port}\nPROCESS_COMPOSE_PORT=4318\nUI_PORT=4173\n",
        encoding="utf-8",
    )

    _write_executable(
        bin_dir / "playwright",
        f"""#!/bin/bash
set -e
touch "{playwright_called}"
exit 0
""",
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
    env.pop("CI", None)

    try:
        result = subprocess.run(
            ["bash", str(_script_copy(tmp_path))],
            cwd=tmp_path / "repo",
            env=env,
            capture_output=True,
            text=True,
            check=False,
        )
    finally:
        listener.close()

    assert result.returncode == 1
    assert f"API server port {occupied_port} is already in use" in result.stderr
    assert not playwright_called.exists()
    assert not process_compose_called.exists()


def test_run_ui_tests_cleans_up_when_process_compose_is_terminated(tmp_path):
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()

    orchestration_pid_path = tmp_path / "orchestration-pid"
    stopped_path = tmp_path / "process-compose-stopped"
    env_file = tmp_path / "test.env"
    env_file.write_text("PROCESS_COMPOSE_PORT=4318\nUI_PORT=4173\n", encoding="utf-8")

    _write_executable(
        bin_dir / "playwright",
        """#!/bin/bash
set -e
exit 0
""",
    )
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
    env.pop("CI", None)

    proc = subprocess.Popen(
        ["bash", str(_script_copy(tmp_path))],
        cwd=tmp_path / "repo",
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
