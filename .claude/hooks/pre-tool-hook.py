#!/usr/bin/env python3
import subprocess
import sys
from pathlib import Path


def main() -> int:
    project_dir = Path(__file__).resolve().parents[2]
    result = subprocess.run(
        [str(project_dir / ".claude/hooks/agent-issues-pretool-hook.sh")],
        input=sys.stdin.buffer.read(),
        cwd=project_dir,
        check=False,
    )
    if result.returncode in (0, 2):
        return result.returncode
    print(
        f"agent-pretool-hook failed with status {result.returncode}; blocking command.",
        file=sys.stderr,
    )
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
