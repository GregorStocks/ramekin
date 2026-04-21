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
    env_file.write_text("PROCESS_COMPOSE_PORT=4317\n", encoding="utf-8")

    _write_executable(
        bin_dir / "process-compose",
        f"""#!/bin/bash
set -e

if [ "$1" = "up" ]; then
  mkdir -p logs
  mkdir -p logs/test-status
  printf '%s\\n' "{log_line}" > logs/test.log
  printf '%s\\n' 0 > logs/test-status/rust-tests-server.exit
  printf '%s\\n' 0 > logs/test-status/rust-tests-cli.exit
  printf '%s\\n' 1 > logs/test-status/rust-tests-core.exit
  printf '%s\\n' 0 > logs/test-status/api-tests.exit
  exit 0
fi

exit 1
""",
    )

    env = os.environ.copy()
    env["PATH"] = f"{bin_dir}:{env['PATH']}"
    env["TEST_ENV_FILE"] = str(env_file)

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
    assert "Last 200 lines of logs/test.log:" in result.stdout
    assert log_line in result.stdout
