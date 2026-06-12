"""Tests for scripts/check_client_parity.py.

We drive the script against a synthetic fake-repo in tmp_path. The detector's
contract is: every OpenAPI op is expected to be used by BOTH clients; the
exceptions file only lists deliberate non-`both` cases.
"""

import importlib.util
import json
import shutil
import subprocess
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts" / "check_client_parity.py"


def _load_module():
    spec = importlib.util.spec_from_file_location("ccp", SCRIPT_PATH)
    mod = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    spec.loader.exec_module(mod)
    return mod


def _make_openapi(ops: dict[str, str]) -> dict:
    """Build a minimal OpenAPI spec with the given {operationId: tag} map."""
    spec: dict = {
        "openapi": "3.0.0",
        "info": {"title": "t", "version": "0"},
        "paths": {},
    }
    for i, (op_id, tag) in enumerate(ops.items()):
        spec["paths"][f"/api/op-{i}"] = {
            "get": {
                "operationId": op_id,
                "tags": [tag],
                "responses": {"200": {"description": "ok"}},
            },
        }
    return spec


def _make_fake_repo(
    tmp_path: Path,
    *,
    ops: dict[str, str],
    web_uses: list[str],
    ios_uses: list[str],
    exceptions_text: str | None,
) -> Path:
    """Lay out a fake repo with openapi.json, exceptions file, and stub sources."""
    root = tmp_path / "repo"
    (root / "api").mkdir(parents=True)
    (root / "api" / "openapi.json").write_text(json.dumps(_make_openapi(ops)))
    if exceptions_text is not None:
        (root / "api" / "client-parity-exceptions.json5").write_text(exceptions_text)

    (root / "ramekin-ui" / "src").mkdir(parents=True)
    (root / "ramekin-ui" / "src" / "use.ts").write_text(
        "// stub web usage\n" + "\n".join(f"x.{m}();" for m in web_uses)
    )

    (root / "ramekin-ios" / "Ramekin").mkdir(parents=True)
    (root / "ramekin-ios" / "Ramekin" / "Use.swift").write_text(
        "// stub ios usage\n" + "\n".join(f"x.{m}()" for m in ios_uses)
    )

    # Drop a generated-client dir on each side that also references the method —
    # the detector must NOT count these as real usage.
    (root / "ramekin-ui" / "generated-client").mkdir(parents=True)
    (root / "ramekin-ui" / "generated-client" / "noise.ts").write_text(
        "\n".join(f"x.{op}();" for op in ops)
    )
    (root / "ramekin-ios" / "generated-client").mkdir(parents=True)
    (root / "ramekin-ios" / "generated-client" / "Noise.swift").write_text(
        "\n".join(f"x.{op}()" for op in ops)
    )

    (root / "scripts").mkdir()
    shutil.copy(SCRIPT_PATH, root / "scripts" / "check_client_parity.py")
    return root


def _run(repo: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["python3", "scripts/check_client_parity.py", *args],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )


def test_both_is_the_default_no_entry_needed(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings"],
        exceptions_text="{}",
    )
    result = _run(repo)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "Client parity OK" in result.stdout


def test_passes_with_documented_exception(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things", "sync_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings", "syncThings"],
        exceptions_text="""{
          sync_things: {
            platforms: "ios-only",
            reason: "offline sync is ios-specific",
          },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 0, result.stdout + result.stderr


def test_fails_when_asymmetric_op_is_missing_from_exceptions(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"new_op": "things"},
        web_uses=["newOp"],
        ios_uses=[],
        exceptions_text="{}",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "new_op: only used by 'web-only'" in result.stderr
    assert "missing from api/client-parity-exceptions.json5" in result.stderr


def test_fails_when_exception_lists_an_op_that_is_now_both(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"adopted": "things"},
        web_uses=["adopted"],
        ios_uses=["adopted"],
        exceptions_text="""{
          adopted: { platforms: "web-only", reason: "iOS adoption pending" },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "both clients now use it" in result.stderr
    assert "Delete the exception entry" in result.stderr


def test_fails_when_exception_platforms_mismatch_source(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"diverged": "things"},
        web_uses=[],
        ios_uses=["diverged"],
        exceptions_text="""{
          diverged: { platforms: "web-only", reason: "ios pending" },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "parity drift" in result.stderr
    assert "exception says 'web-only'" in result.stderr
    assert "source shows 'ios-only'" in result.stderr


def test_fails_when_exception_references_unknown_op(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings"],
        exceptions_text="""{
          removed_op: { platforms: "neither", reason: "old endpoint" },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert (
        "removed_op: listed in api/client-parity-exceptions.json5 but not in "
        "api/openapi.json" in result.stderr
    )


def test_fails_when_exception_has_no_reason(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"sync_things": "things"},
        web_uses=[],
        ios_uses=["syncThings"],
        exceptions_text="""{
          sync_things: { platforms: "ios-only" },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "every exception must include a reason" in result.stderr


def test_generated_client_dirs_are_excluded(tmp_path):
    # ops referenced ONLY inside generated-client must come back as "neither".
    repo = _make_fake_repo(
        tmp_path,
        ops={"only_in_generated": "things"},
        web_uses=[],
        ios_uses=[],
        exceptions_text="""{
          only_in_generated: { platforms: "neither", reason: "infra-only" },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 0, result.stdout + result.stderr


def test_fails_when_exceptions_file_is_missing(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings"],
        exceptions_text=None,
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "Missing api/client-parity-exceptions.json5" in result.stderr


def test_update_omits_both_ops_and_writes_only_exceptions(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things", "sync_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings", "syncThings"],
        exceptions_text="""{
          sync_things: {
            platforms: "ios-only",
            reason: "offline sync is ios-specific",
          },
        }""",
    )
    result = _run(repo, "--update")
    assert result.returncode == 0, result.stdout + result.stderr
    out = (repo / "api" / "client-parity-exceptions.json5").read_text()
    assert "list_things" not in out  # both-by-default, omitted
    assert "sync_things" in out
    assert "offline sync is ios-specific" in out


def test_update_drops_entry_when_op_becomes_both(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings"],
        exceptions_text="""{
          list_things: { platforms: "ios-only", reason: "old reason" },
        }""",
    )
    result = _run(repo, "--update")
    assert result.returncode == 0, result.stdout + result.stderr
    out = (repo / "api" / "client-parity-exceptions.json5").read_text()
    assert "list_things" not in out
    assert "old reason" not in out


@pytest.mark.parametrize(
    ("snake", "camel"),
    [
        ("list_recipes", "listRecipes"),
        ("sync_items", "syncItems"),
        ("import_from_photos", "importFromPhotos"),
        ("ping", "ping"),
        ("unauthed_ping", "unauthedPing"),
    ],
)
def test_snake_to_camel(snake, camel):
    assert _load_module().snake_to_camel(snake) == camel
