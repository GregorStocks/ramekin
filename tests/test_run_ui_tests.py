import os
import stat
import subprocess
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "run-ui-tests.sh"


def _write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


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
    assert "UI test orchestration failed. Last 200 lines of logs/test-ui.log:" in result.stdout
    assert log_line in result.stdout
