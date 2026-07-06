import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest


PROJECT_ROOT = Path(__file__).resolve().parents[1]
SHELL_WRAPPER = PROJECT_ROOT / ".claude" / "hooks" / "agent-issues-pretool-hook.sh"
PYTHON_BRIDGE = PROJECT_ROOT / ".claude" / "hooks" / "pre-tool-hook.py"
LOCAL_HOOK = PROJECT_ROOT / ".claude" / "hooks" / "pretool-local.sh"
PRETOOL_CONFIG = PROJECT_ROOT / ".agent-issues" / "pretool-hook.json5"


@pytest.fixture
def hook_project(tmp_path: Path) -> Path:
    project = tmp_path / "project"
    (project / ".agent-issues").mkdir(parents=True)
    (project / ".claude" / "hooks").mkdir(parents=True)
    (project / "cli").mkdir()

    shutil.copy2(PRETOOL_CONFIG, project / ".agent-issues" / "pretool-hook.json5")
    shutil.copy2(
        SHELL_WRAPPER,
        project / ".claude" / "hooks" / "agent-issues-pretool-hook.sh",
    )
    shutil.copy2(LOCAL_HOOK, project / ".claude" / "hooks" / "pretool-local.sh")
    shutil.copy2(PYTHON_BRIDGE, project / ".claude" / "hooks" / "pre-tool-hook.py")

    subprocess.run(["git", "init", "-q"], cwd=project, check=True)
    return project


def hook_env(project_root: Path, **overrides: str) -> dict[str, str]:
    return {
        **os.environ,
        **overrides,
        "CLAUDE_PROJECT_DIR": str(project_root),
    }


def run_hook(command: str, project_root: Path) -> subprocess.CompletedProcess[str]:
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": command},
    }
    shell_wrapper = project_root / ".claude" / "hooks" / "agent-issues-pretool-hook.sh"
    return subprocess.run(
        [str(shell_wrapper)],
        input=json.dumps(payload),
        cwd=project_root,
        env=hook_env(project_root),
        capture_output=True,
        text=True,
        check=False,
    )


def assert_blocks(command: str, expected: str, project_root: Path) -> None:
    result = run_hook(command, project_root)
    assert result.returncode == 2
    assert expected in result.stderr


def assert_allows(command: str, project_root: Path) -> None:
    result = run_hook(command, project_root)
    assert result.returncode == 0
    assert result.stderr == ""


def test_blocks_direct_cargo_and_rustfmt_invocations(hook_project: Path) -> None:
    assert_blocks("cargo test", "Use `make test` instead.", hook_project)
    assert_blocks("cargo fmt", "Use `make lint` instead.", hook_project)
    assert_blocks(
        "rustfmt server/src/main.rs", "Use `make lint` instead.", hook_project
    )


def test_blocks_direct_docker_and_built_cli_invocations(hook_project: Path) -> None:
    assert_blocks("docker ps", "Do not run docker directly.", hook_project)
    assert_blocks(
        "./cli/target/debug/ramekin-cli --help",
        "Do not invoke the built ramekin-cli binary directly.",
        hook_project,
    )


def test_blocks_process_name_kills(hook_project: Path) -> None:
    assert_blocks("pkill -f cargo", "pkill/killall", hook_project)
    assert_blocks("killall cargo", "pkill/killall", hook_project)


def test_blocks_github_issues_and_raw_pr_publication(hook_project: Path) -> None:
    assert_blocks(
        "gh issue create --title Bug", "Do not use GitHub Issues.", hook_project
    )
    assert_blocks("gh pr edit 123 --title Updated", "agent-submit", hook_project)
    assert_blocks("git push origin HEAD", "agent-submit", hook_project)


def test_branch_switch_uses_ramekin_signoff_env(hook_project: Path) -> None:
    assert_blocks(
        "git switch feature-branch",
        "RAMEKIN_BRANCH_SWITCH_SIGNOFF=feature-branch",
        hook_project,
    )
    assert_allows(
        "RAMEKIN_BRANCH_SWITCH_SIGNOFF=feature-branch git switch feature-branch",
        hook_project,
    )


def test_blocks_manual_pipeline_output_mutation(hook_project: Path) -> None:
    assert_blocks(
        "rm data/pipeline-snapshots/example.json",
        "Do not manually mutate generated output paths.",
        hook_project,
    )
    assert_blocks(
        "printf '{}' > data/pipeline-runs/run.json",
        "Do not redirect shell output into generated output paths.",
        hook_project,
    )


def test_blocks_tmp_worktree_creation(hook_project: Path) -> None:
    assert_blocks(
        "git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "bash -lc 'git worktree add /tmp/ramekin-scratch feature-branch'",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "(git worktree add /tmp/ramekin-scratch feature-branch)",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "{ git worktree add /tmp/ramekin-scratch feature-branch; }",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        'echo "$(git worktree add /tmp/ramekin-scratch feature-branch)"',
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "true&&git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "cd /;git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "git -c alias.w='worktree add' w /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        'env -S "git worktree add /tmp/ramekin-scratch feature-branch"',
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "env -u FOO git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "command git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )
    assert_blocks(
        "timeout 30 git worktree add /tmp/ramekin-scratch feature-branch",
        "Do not create git worktrees under /tmp.",
        hook_project,
    )


def test_allows_documenting_tmp_worktree_commands(hook_project: Path) -> None:
    assert_allows(
        "echo git worktree add /tmp/ramekin-scratch feature-branch", hook_project
    )


def test_python_bridge_runs_shared_hook_from_repo_root(hook_project: Path) -> None:
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": "rm data/pipeline-runs/run.json"},
    }
    result = subprocess.run(
        [sys.executable, str(hook_project / ".claude" / "hooks" / "pre-tool-hook.py")],
        input=json.dumps(payload),
        cwd=hook_project / "cli",
        env=hook_env(hook_project),
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "Do not manually mutate generated output paths." in result.stderr


def test_python_bridge_runs_local_hook(hook_project: Path) -> None:
    payload = {
        "tool_name": "Bash",
        "tool_input": {"command": "git worktree add /tmp/ramekin-scratch feature"},
    }
    result = subprocess.run(
        [sys.executable, str(hook_project / ".claude" / "hooks" / "pre-tool-hook.py")],
        input=json.dumps(payload),
        cwd=hook_project,
        env=hook_env(hook_project),
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "Do not create git worktrees under /tmp." in result.stderr


def fake_hook_env(tmp_path: Path, project_root: Path) -> dict[str, str]:
    fake_hook = tmp_path / "agent-pretool-hook"
    fake_hook.write_text("#!/bin/sh\necho shared hook crashed >&2\nexit 1\n")
    fake_hook.chmod(0o755)
    return hook_env(project_root, PATH=f"{tmp_path}{os.pathsep}{os.environ['PATH']}")


def test_shell_wrapper_normalizes_shared_hook_failures_to_blocking_exit(
    tmp_path: Path,
    hook_project: Path,
) -> None:
    result = subprocess.run(
        [str(hook_project / ".claude" / "hooks" / "agent-issues-pretool-hook.sh")],
        input=json.dumps({"tool_name": "Bash", "tool_input": {"command": "true"}}),
        cwd=hook_project,
        env=fake_hook_env(tmp_path, hook_project),
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "agent-pretool-hook failed with status 1" in result.stderr


def test_python_bridge_normalizes_shared_hook_failures_to_blocking_exit(
    tmp_path: Path,
    hook_project: Path,
) -> None:
    result = subprocess.run(
        [sys.executable, str(hook_project / ".claude" / "hooks" / "pre-tool-hook.py")],
        input=json.dumps({"tool_name": "Bash", "tool_input": {"command": "true"}}),
        cwd=hook_project,
        env=fake_hook_env(tmp_path, hook_project),
        capture_output=True,
        text=True,
        check=False,
    )

    assert result.returncode == 2
    assert "agent-pretool-hook failed with status 1" in result.stderr
