import importlib.util
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "worktree-setup.py"
SPEC = importlib.util.spec_from_file_location("worktree_setup", SCRIPT_PATH)
assert SPEC is not None and SPEC.loader is not None
WORKTREE_SETUP = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = WORKTREE_SETUP
SPEC.loader.exec_module(WORKTREE_SETUP)


def test_worktree_databases_are_files_inside_the_worktree(tmp_path):
    dev, test = WORKTREE_SETUP.database_paths(tmp_path)
    assert dev == tmp_path / ".db" / "dev.sqlite"
    assert test == tmp_path / ".db" / "test.sqlite"


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
            "UI_PORT_HTTP": "57691",
            "RAMEKIN_SELF_SIGNED_URL": "http://localhost:57690",
            "RAMEKIN_EXTERNAL_URL": "http://localhost:57691",
        },
    )
    assert "RAMEKIN_SELF_SIGNED_URL=http://localhost:57690" in test_rendered
    assert "RAMEKIN_EXTERNAL_URL=http://localhost:57691" in test_rendered
    assert "UI_PORT_HTTP=57691" in test_rendered
    assert "RAMEKIN_SELF_SIGNED_URL=http://localhost:5174" not in test_rendered
    assert "RAMEKIN_EXTERNAL_URL=http://localhost:5175" not in test_rendered


def test_generated_dev_env_does_not_enable_ai_with_a_placeholder(tmp_path):
    output = tmp_path / "dev.env"
    WORKTREE_SETUP.write_env_file(REPO_ROOT / "dev.env.example", output, {})
    active_lines = [
        line for line in output.read_text().splitlines() if not line.startswith("#")
    ]
    assert not any(line.startswith("OPENROUTER_API_KEY=") for line in active_lines)
