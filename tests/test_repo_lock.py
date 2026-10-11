import os
import signal
import subprocess
import time
from pathlib import Path

from repo_lock_helpers import is_repo_lock_held


REPO_ROOT = Path(__file__).resolve().parents[1]
REPO_LOCK = REPO_ROOT / "scripts" / "repo-lock.sh"


def _bash(script: str, lock_root: Path, **kwargs) -> subprocess.Popen:
    return subprocess.Popen(
        ["bash", "-c", f'source "{REPO_LOCK}"\n{script}'],
        env={**os.environ, "REPO_LOCK_DIR": str(lock_root)},
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        **kwargs,
    )


def _try_lock(lock_root: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["bash", "-c", f'source "{REPO_LOCK}"\nacquire_repo_lock demo "demo run"'],
        env={**os.environ, "REPO_LOCK_DIR": str(lock_root)},
        capture_output=True,
        text=True,
        timeout=30,
    )


def _start_holder(lock_root: Path, script: str) -> tuple[subprocess.Popen, str]:
    """Start a shell that takes the demo lock, runs `script`, then blocks on a
    builtin read (so it has no children of its own). Returns the shell and
    the first line `script` printed."""
    holder = _bash(
        f'acquire_repo_lock demo "demo run"\n{script}\nread -r _',
        lock_root,
    )
    line = holder.stdout.readline().strip()
    assert line, holder.stderr.read()
    return holder, line


def test_lock_held_by_a_sigkilled_shell_is_released(tmp_path):
    # No EXIT trap runs on SIGKILL; the kernel still drops the flock.
    holder, _ = _start_holder(tmp_path, "echo locked")
    assert is_repo_lock_held(tmp_path / "demo.flock")

    holder.kill()
    holder.wait(timeout=5)

    assert not is_repo_lock_held(tmp_path / "demo.flock")
    assert _try_lock(tmp_path).returncode == 0


def test_orphaned_children_keep_the_lock_until_they_exit(tmp_path):
    # `kill -9 <shell>` leaves its workers running; a later run must not start
    # over in the directories they are still writing to.
    holder, child_pid = _start_holder(tmp_path, "sleep 60 &\necho $!")
    try:
        holder.kill()
        holder.wait(timeout=5)

        refused = _try_lock(tmp_path)
        assert refused.returncode == 1, refused.stderr
        assert "another demo run is already running" in refused.stderr
    finally:
        os.kill(int(child_pid), signal.SIGKILL)

    # The orphan isn't our child, so poll rather than wait on it.
    deadline = time.monotonic() + 5
    while is_repo_lock_held(tmp_path / "demo.flock"):
        assert time.monotonic() < deadline, "orphan's lock never released"
        time.sleep(0.05)
    assert _try_lock(tmp_path).returncode == 0


def test_waiter_takes_the_lock_once_the_holder_exits(tmp_path):
    holder, _ = _start_holder(tmp_path, "echo locked")
    waiter = _bash(
        'REPO_LOCK_WAIT=1 acquire_repo_lock demo "demo run"\necho acquired',
        tmp_path,
    )
    try:
        assert "Waiting up to" in waiter.stderr.readline()
    finally:
        holder.communicate("\n", timeout=5)

    output = waiter.communicate(timeout=30)[0]
    assert waiter.returncode == 0
    assert output == "acquired\n"


def test_a_shell_cannot_take_a_second_lock(tmp_path):
    # Both locks share one descriptor, so the second would release the first.
    proc = _bash(
        'acquire_repo_lock demo "demo run"\nacquire_repo_lock other "other run"',
        tmp_path,
    )
    _, stderr = proc.communicate(timeout=30)

    assert proc.returncode == 1
    assert "this shell already holds" in stderr
