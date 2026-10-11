"""Helpers for tests of scripts/repo-lock.sh, whose locks are flock(2) locks."""

import contextlib
import fcntl
import os
from pathlib import Path


@contextlib.contextmanager
def held_repo_lock(lock_file: Path):
    """Hold `lock_file` the way a running script would, recording our PID."""
    lock_file.parent.mkdir(parents=True, exist_ok=True)
    with open(lock_file, "a+", encoding="utf-8") as f:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        f.truncate(0)
        f.write(f"{os.getpid()}\n")
        f.flush()
        yield


def is_repo_lock_held(lock_file: Path) -> bool:
    if not lock_file.exists():
        return False
    with open(lock_file, "a", encoding="utf-8") as f:
        try:
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError:
            return True
    return False
