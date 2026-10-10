import os
import shutil
import signal
import subprocess
import time
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]

# Stands in for both openapi-generator (creates the -o dir) and tsc (no-op).
FAKE_NPX = """#!/bin/bash
if [ -n "${GENERATE_STARTED:-}" ]; then
    touch "$GENERATE_STARTED"
    sleep 1
fi
while [ $# -gt 0 ]; do
    if [ "$1" = "-o" ]; then
        mkdir -p "$2"
        echo generate >> "$GENERATE_LOG"
    fi
    shift
done
"""


def _generate_clients_sandbox(tmp_path: Path) -> dict[str, str]:
    """Lay out a fake project for scripts/generate-clients.sh and return the
    env that puts the fake npx on PATH."""
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    for name in ["generate-clients.sh", "repo-lock.sh"]:
        shutil.copy(REPO_ROOT / "scripts" / name, scripts / name)
    (tmp_path / "api").mkdir()
    (tmp_path / "api" / "openapi.json").write_text("{}", encoding="utf-8")
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    npx = bin_dir / "npx"
    npx.write_text(FAKE_NPX, encoding="utf-8")
    npx.chmod(0o755)
    return {
        **os.environ,
        "PATH": f"{bin_dir}:{os.environ['PATH']}",
        "GENERATE_LOG": str(tmp_path / "generated"),
    }


def _generate_clients(tmp_path: Path, env: dict[str, str], **extra_env: str):
    return subprocess.run(
        [str(tmp_path / "scripts" / "generate-clients.sh")],
        env={**env, **extra_env},
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=30,
    )


def _lock(tmp_path: Path) -> Path:
    return tmp_path / "logs" / "locks" / "generate-clients.lock"


def _generated(tmp_path: Path) -> int:
    log = tmp_path / "generated"
    return len(log.read_text().splitlines()) if log.exists() else 0


def test_generate_clients_releases_its_lock(tmp_path):
    env = _generate_clients_sandbox(tmp_path)

    result = _generate_clients(tmp_path, env)

    assert result.returncode == 0, result.stdout
    assert _generated(tmp_path) == 4
    assert not _lock(tmp_path).exists()


def test_generate_clients_clears_a_lock_left_by_a_killed_run(tmp_path):
    # A SIGKILLed run never reaches its EXIT trap; later runs used to wait on
    # its lock forever.
    env = _generate_clients_sandbox(tmp_path)
    finished = subprocess.Popen(["true"])
    finished.wait()
    lock = _lock(tmp_path)
    lock.mkdir(parents=True)
    (lock / "pid").write_text(f"{finished.pid}\n", encoding="utf-8")

    result = _generate_clients(tmp_path, env)

    assert result.returncode == 0, result.stdout
    assert "Removing stale" in result.stdout
    assert _generated(tmp_path) == 4
    assert not lock.exists()


def test_generate_clients_gives_up_on_a_lock_held_too_long(tmp_path):
    # A killed run's PID can be reused by an unrelated process, which would
    # otherwise keep the lock looking live forever.
    env = _generate_clients_sandbox(tmp_path)
    lock = _lock(tmp_path)
    lock.mkdir(parents=True)
    (lock / "pid").write_text(f"{os.getpid()}\n", encoding="utf-8")

    result = _generate_clients(tmp_path, env, REPO_LOCK_WAIT_SECONDS="1")

    assert result.returncode == 1, result.stdout
    assert "Refusing to start API client generation" in result.stdout
    assert str(lock) in result.stdout
    assert _generated(tmp_path) == 0
    assert lock.exists()


def test_generate_clients_stops_when_terminated(tmp_path):
    # SIGTERM to the script alone (not its process group) must end the run, not
    # just drop the lock and let it keep regenerating clients unlocked.
    env = _generate_clients_sandbox(tmp_path)
    started = tmp_path / "started"
    proc = subprocess.Popen(
        [str(tmp_path / "scripts" / "generate-clients.sh")],
        env={**env, "GENERATE_STARTED": str(started)},
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    deadline = time.monotonic() + 10
    while not started.exists():
        assert time.monotonic() < deadline, "generator never started"
        time.sleep(0.05)

    proc.send_signal(signal.SIGTERM)
    output = proc.communicate(timeout=30)[0]

    assert proc.returncode == 143, output
    assert _generated(tmp_path) == 1
    assert not _lock(tmp_path).exists()
