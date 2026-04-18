import importlib.util
import sys
from pathlib import Path
from types import SimpleNamespace


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "worktree-setup.py"
SPEC = importlib.util.spec_from_file_location("worktree_setup", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
WORKTREE_SETUP = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = WORKTREE_SETUP
SPEC.loader.exec_module(WORKTREE_SETUP)


def test_worktree_setup_creates_both_databases_when_postgres_is_ready(monkeypatch):
    commands: list[tuple[list[str], dict[str, str] | None]] = []

    def fake_run(command, **kwargs):
        commands.append((command, kwargs.get("env")))
        return SimpleNamespace(returncode=0, stderr="")

    monkeypatch.setattr(WORKTREE_SETUP.subprocess, "run", fake_run)

    config = WORKTREE_SETUP.GeneratedConfig(
        workspace_name="demo",
        dev_database_name="ramekin_demo",
        test_database_name="ramekin_demo_test",
        dev_port=1,
        dev_ui_port=2,
        dev_ui_port_http=3,
        dev_process_compose_port=4,
        test_port=5,
        test_fixture_port=6,
        test_ui_port=7,
        test_mock_openrouter_port=8,
        test_process_compose_port=9,
    )

    WORKTREE_SETUP.create_workspace_databases_if_available(config, "localhost", 54321)

    assert [command for command, _ in commands] == [
        ["pg_isready", "-h", "localhost", "-p", "54321", "-U", "ramekin"],
        [
            "createdb",
            "-h",
            "localhost",
            "-p",
            "54321",
            "-U",
            "ramekin",
            "--no-password",
            "ramekin_demo",
        ],
        [
            "createdb",
            "-h",
            "localhost",
            "-p",
            "54321",
            "-U",
            "ramekin",
            "--no-password",
            "ramekin_demo_test",
        ],
    ]
    assert commands[1][1] is not None
    assert commands[1][1]["PGPASSWORD"] == "ramekin"
    assert commands[2][1] is not None
    assert commands[2][1]["PGPASSWORD"] == "ramekin"


def test_worktree_setup_skips_database_creation_when_postgres_is_unreachable(
    monkeypatch, capsys
):
    commands: list[list[str]] = []

    def fake_run(command, **kwargs):
        commands.append(command)
        return SimpleNamespace(returncode=1, stderr="")

    monkeypatch.setattr(WORKTREE_SETUP.subprocess, "run", fake_run)

    config = WORKTREE_SETUP.GeneratedConfig(
        workspace_name="demo",
        dev_database_name="ramekin_demo",
        test_database_name="ramekin_demo_test",
        dev_port=1,
        dev_ui_port=2,
        dev_ui_port_http=3,
        dev_process_compose_port=4,
        test_port=5,
        test_fixture_port=6,
        test_ui_port=7,
        test_mock_openrouter_port=8,
        test_process_compose_port=9,
    )

    WORKTREE_SETUP.create_workspace_databases_if_available(config, "localhost", 54321)

    assert commands == [
        ["pg_isready", "-h", "localhost", "-p", "54321", "-U", "ramekin"]
    ]
    assert "skipped workspace database creation" in capsys.readouterr().out


def test_apply_overrides_rewrites_ramekin_urls_to_assigned_ui_port():
    dev_template = REPO_ROOT.joinpath("dev.env.example").read_text(encoding="utf-8")
    test_template = REPO_ROOT.joinpath("test.env.example").read_text(encoding="utf-8")

    dev_rendered = WORKTREE_SETUP.apply_overrides(
        dev_template,
        {
            "UI_PORT": "57684",
            "RAMEKIN_SELF_SIGNED_URL": "https://localhost:57684",
            "RAMEKIN_EXTERNAL_URL": "https://localhost:57684",
        },
    )
    assert "RAMEKIN_SELF_SIGNED_URL=https://localhost:57684" in dev_rendered
    assert "RAMEKIN_EXTERNAL_URL=https://localhost:57684" in dev_rendered
    assert "RAMEKIN_SELF_SIGNED_URL=https://localhost:5173" not in dev_rendered
    assert "RAMEKIN_EXTERNAL_URL=https://localhost:5173" not in dev_rendered

    test_rendered = WORKTREE_SETUP.apply_overrides(
        test_template,
        {
            "UI_PORT": "57690",
            "RAMEKIN_SELF_SIGNED_URL": "https://localhost:57690",
            "RAMEKIN_EXTERNAL_URL": "http://localhost:57690",
        },
    )
    assert "RAMEKIN_SELF_SIGNED_URL=https://localhost:57690" in test_rendered
    assert "RAMEKIN_EXTERNAL_URL=http://localhost:57690" in test_rendered
    assert "RAMEKIN_SELF_SIGNED_URL=https://localhost:5174" not in test_rendered
    assert "RAMEKIN_EXTERNAL_URL=http://localhost:5174" not in test_rendered
