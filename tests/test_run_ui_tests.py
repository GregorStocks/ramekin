import os
import signal
import stat
import subprocess
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "run-ui-tests.sh"


def _write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


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

    log_path = REPO_ROOT / "logs" / "test-ui.log"
    original_log = log_path.read_bytes() if log_path.exists() else None

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["PROCESS_COMPOSE_PORT"] = "4318"
    env.pop("CI", None)

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
        if original_log is None:
            log_path.unlink(missing_ok=True)
        else:
            log_path.write_bytes(original_log)

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

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)
    env["PROCESS_COMPOSE_PORT"] = "4318"
    env.pop("CI", None)

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
        f"up -e {env_file} -f test-ui-compose.yaml -t=false --port 4318",
        "down --port 4318",
    ]
