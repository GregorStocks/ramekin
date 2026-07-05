from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def _read(path: str) -> str:
    return (REPO_ROOT / path).read_text(encoding="utf-8")


def test_test_compose_files_extend_shared_test_base():
    assert "extends: compose/test-base.yaml" in _read("test-compose.yaml")
    assert "extends: compose/test-base.yaml" in _read("test-ui-compose.yaml")


def test_shared_test_base_owns_fixture_and_mock_processes():
    test_base = _read("compose/test-base.yaml")

    assert "extends: base.yaml" in test_base
    assert "\n  fixture:\n" in test_base
    assert "\n  fixture-server:\n" not in test_base
    assert "working_dir: ${PWD}/tests/scrape_fixtures" in test_base
    assert "\n  mock-openrouter:\n" in test_base
    assert "command: python3 tests/mock_openrouter.py" in test_base
    assert "working_dir: ${PWD}" in test_base
    assert "working_dir: ${PWD}/server" in test_base
    assert "SCRAPE_ALLOWED_HOSTS=localhost:${FIXTURE_PORT}" in test_base
    assert (
        "RAMEKIN_AI_BASE_URL=http://localhost:${MOCK_OPENROUTER_PORT}/v1" in test_base
    )


def test_test_compose_overlays_do_not_redeclare_shared_process_blocks():
    for path in ["test-compose.yaml", "test-ui-compose.yaml"]:
        contents = _read(path)

        assert "\n  fixture:\n" not in contents
        assert "\n  fixture-server:\n" not in contents
        assert "\n  mock-openrouter:\n" not in contents
        assert "SCRAPE_ALLOWED_HOSTS=localhost:${FIXTURE_PORT}" not in contents
        assert (
            "RAMEKIN_AI_BASE_URL=http://localhost:${MOCK_OPENROUTER_PORT}/v1"
            not in contents
        )
