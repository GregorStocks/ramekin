import json
import os
import subprocess
import sys
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parents[1]
SHELL_WRAPPER = PROJECT_ROOT / ".claude" / "hooks" / "agent-issues-pretool-hook.sh"
PYTHON_BRIDGE = PROJECT_ROOT / ".claude" / "hooks" / "pre-tool-hook.py"


def run_hook(command: str) -> subprocess.CompletedProcess[str]:
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": command},
    }
    return subprocess.run(
        [str(SHELL_WRAPPER)],
        input=json.dumps(payload),
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def assert_blocks(command: str, expected: str) -> None:
    result = run_hook(command)
    assert result.returncode == 2
    assert expected in result.stderr


def assert_allows(command: str) -> None:
    result = run_hook(command)
    assert result.returncode == 0
    assert result.stderr == ""


def test_blocks_direct_cargo_and_rustfmt_invocations() -> None:
    assert_blocks("cargo test", "Use `make test` instead.")
    assert_blocks("cargo fmt", "Use `make lint` instead.")
    assert_blocks("rustfmt server/src/main.rs", "Use `make lint` instead.")


def test_blocks_direct_docker_and_built_cli_invocations() -> None:
    assert_blocks("docker ps", "Do not run docker directly.")
    assert_blocks(
        "./cli/target/debug/ramekin-cli --help",
        "Do not invoke the built ramekin-cli binary directly.",
    )


def test_blocks_process_name_kills() -> None:
    assert_blocks("pkill -f cargo", "pkill/killall")
    assert_blocks("killall cargo", "pkill/killall")


def test_blocks_github_issues_and_raw_pr_publication() -> None:
    assert_blocks("gh issue create --title Bug", "Do not use GitHub Issues.")
    assert_blocks("gh pr edit 123 --title Updated", "agent-submit")
    assert_blocks("git push origin HEAD", "agent-submit")


def test_branch_switch_uses_ramekin_signoff_env() -> None:
    assert_blocks(
        "git switch feature-branch",
        "RAMEKIN_BRANCH_SWITCH_SIGNOFF=feature-branch",
    )
    assert_allows(
        "RAMEKIN_BRANCH_SWITCH_SIGNOFF=feature-branch git switch feature-branch"
    )


def test_blocks_manual_pipeline_output_mutation() -> None:
    assert_blocks(
        "rm data/pipeline-snapshots/example.json",
        "Do not manually mutate generated output paths.",
    )
    assert_blocks(
        "printf '{}' > data/pipeline-runs/run.json",
        "Do not redirect shell output into generated output paths.",
    )


def test_blocks_tmp_worktree_creation() -> None:
    assert_blocks(
        "git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "bash -lc 'git worktree add /tmp/ramekin-scratch feature-branch'",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "(git worktree add /tmp/ramekin-scratch feature-branch)",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "{ git worktree add /tmp/ramekin-scratch feature-branch; }",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        'echo "$(git worktree add /tmp/ramekin-scratch feature-branch)"',
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "true&&git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "cd /;git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "git -c alias.w='worktree add' w /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        'env -S "git worktree add /tmp/ramekin-scratch feature-branch"',
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "env -u FOO git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "command git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
    )
    assert_blocks(
        "timeout 30 git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
    )


def test_allows_documenting_tmp_worktree_commands() -> None:
    assert_allows("echo git worktree add /tmp/ramekin-scratch feature-branch")


def test_python_bridge_runs_shared_hook_from_repo_root() -> None:
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": "rm data/pipeline-runs/run.json"},
    }
    result = subprocess.run(
        [sys.executable, str(PYTHON_BRIDGE)],
        input=json.dumps(payload),
        cwd=PROJECT_ROOT / "cli",
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "Do not manually mutate generated output paths." in result.stderr


def test_python_bridge_runs_local_hook() -> None:
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": "git worktree add /tmp/ramekin-scratch feature"},
    }
    result = subprocess.run(
        [sys.executable, str(PYTHON_BRIDGE)],
        input=json.dumps(payload),
        cwd=PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "Do not create git worktrees under /tmp." in result.stderr


def fake_hook_env(tmp_path: Path) -> dict[str, str]:
    fake_hook = tmp_path / "agent-pretool-hook"
    fake_hook.write_text("#!/bin/sh\necho shared hook crashed >&2\nexit 1\n")
    fake_hook.chmod(0o755)
    return {
        **os.environ,
        "PATH": f"{tmp_path}{os.pathsep}{os.environ['PATH']}",
    }


def test_shell_wrapper_normalizes_shared_hook_failures_to_blocking_exit(
    tmp_path: Path,
) -> None:
    result = subprocess.run(
        [str(SHELL_WRAPPER)],
        input=json.dumps({"tool_name": "Bash", "tool_input": {"command": "true"}}),
        cwd=PROJECT_ROOT,
        env=fake_hook_env(tmp_path),
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "agent-pretool-hook failed with status 1" in result.stderr


def test_python_bridge_normalizes_shared_hook_failures_to_blocking_exit(
    tmp_path: Path,
) -> None:
    result = subprocess.run(
        [sys.executable, str(PYTHON_BRIDGE)],
        input=json.dumps({"tool_name": "Bash", "tool_input": {"command": "true"}}),
        cwd=PROJECT_ROOT,
        env=fake_hook_env(tmp_path),
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "agent-pretool-hook failed with status 1" in result.stderr
