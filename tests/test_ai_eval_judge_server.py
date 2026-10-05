import importlib.util
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "scripts" / "ai-eval-judge-server.py"


def _server():
    spec = importlib.util.spec_from_file_location("ai_eval_judge_server", SCRIPT)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_a_stale_page_keeps_verdicts_saved_since_it_opened():
    on_disk = {"case": {"a": {"verdict": "good"}, "b": {"verdict": "bad"}}}
    # Opened before "b" was judged, this page knows only "a" and its own "c".
    saved = {"case": {"a": {"verdict": "good"}, "c": {"verdict": "best"}}}
    assert _server().merge(on_disk, saved) == {
        "case": {
            "a": {"verdict": "good"},
            "b": {"verdict": "bad"},
            "c": {"verdict": "best"},
        }
    }


def test_a_saved_verdict_replaces_the_old_one():
    on_disk = {"case": {"a": {"verdict": "good"}}}
    saved = {"case": {"a": {"verdict": "bad"}}}
    assert _server().merge(on_disk, saved) == saved


def test_a_tag_set_is_replaced_and_other_cases_kept():
    on_disk = {"one": ["Dinner"], "two": ["Dessert"]}
    saved = {"one": ["Lunch"]}
    assert _server().merge(on_disk, saved) == {"one": ["Lunch"], "two": ["Dessert"]}
