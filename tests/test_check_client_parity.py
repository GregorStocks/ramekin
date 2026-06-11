"""Tests for scripts/check_client_parity.py.

We drive the script against a synthetic fake-repo laid out in tmp_path so we
can assert that drift, missing entries, missing reasons, and stale entries
all surface as errors.
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
    parity_text: str,
) -> Path:
    """Lay out a fake repo with openapi.json, parity file, and stub client sources."""
    root = tmp_path / "repo"
    (root / "api").mkdir(parents=True)
    (root / "api" / "openapi.json").write_text(json.dumps(_make_openapi(ops)))
    (root / "api" / "client-parity.json5").write_text(parity_text)

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

    # Copy the script into the fake repo so its project_root() resolves there.
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


def test_passes_when_matrix_matches_observed(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things", "sync_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings", "syncThings"],
        parity_text="""{
          operations: {
            list_things: "both",
            sync_things: {
              platforms: "ios-only",
              reason: "offline sync is ios-specific",
            },
          },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 0, result.stdout + result.stderr
    assert "Client parity OK" in result.stdout


def test_fails_on_drift_when_matrix_disagrees_with_source(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things"},
        web_uses=["listThings"],  # only web uses it
        ios_uses=[],
        parity_text="""{ operations: { list_things: "both" } }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "list_things: parity drift" in result.stderr
    assert "matrix says 'both'" in result.stderr
    assert "source says 'web-only'" in result.stderr


def test_fails_when_operation_is_missing_from_matrix(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things", "new_op": "things"},
        web_uses=["listThings", "newOp"],
        ios_uses=["listThings", "newOp"],
        parity_text="""{ operations: { list_things: "both" } }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "new_op: present in api/openapi.json but missing" in result.stderr


def test_fails_when_matrix_entry_is_stale(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings"],
        parity_text="""{
          operations: {
            list_things: "both",
            removed_op: { platforms: "neither", reason: "old endpoint" },
          },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert (
        "removed_op: listed in api/client-parity.json5 but not in api/openapi.json"
        in result.stderr
    )


def test_fails_when_asymmetric_entry_has_no_reason(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"sync_things": "things"},
        web_uses=[],
        ios_uses=["syncThings"],
        parity_text="""{ operations: { sync_things: "ios-only" } }""",
    )
    result = _run(repo)
    assert result.returncode == 1
    assert "sync_things: 'ios-only' requires a reason" in result.stderr


def test_generated_client_dirs_are_excluded(tmp_path):
    # ops referenced ONLY inside generated-client must come back as "neither".
    repo = _make_fake_repo(
        tmp_path,
        ops={"only_in_generated": "things"},
        web_uses=[],
        ios_uses=[],
        parity_text="""{
          operations: {
            only_in_generated: { platforms: "neither", reason: "infra-only" },
          },
        }""",
    )
    result = _run(repo)
    assert result.returncode == 0, result.stdout + result.stderr


def test_update_seeds_matrix_from_observed_and_preserves_reasons(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things", "sync_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings", "syncThings"],
        # Pre-existing matrix with a reason on sync_things; --update keeps it.
        parity_text="""{
          operations: {
            list_things: "both",
            sync_things: {
              platforms: "ios-only",
              reason: "offline sync is ios-specific",
            },
          },
        }""",
    )
    result = _run(repo, "--update")
    assert result.returncode == 0, result.stdout + result.stderr
    out = (repo / "api" / "client-parity.json5").read_text()
    assert 'list_things: "both"' in out
    assert "sync_things:" in out
    assert "offline sync is ios-specific" in out


def test_update_drops_reason_when_platforms_change(tmp_path):
    repo = _make_fake_repo(
        tmp_path,
        ops={"list_things": "things"},
        web_uses=["listThings"],
        ios_uses=["listThings"],
        # Old matrix said ios-only with a reason; both clients now use it.
        parity_text="""{
          operations: {
            list_things: { platforms: "ios-only", reason: "old reason" },
          },
        }""",
    )
    result = _run(repo, "--update")
    assert result.returncode == 0, result.stdout + result.stderr
    out = (repo / "api" / "client-parity.json5").read_text()
    assert 'list_things: "both"' in out
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
