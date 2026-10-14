"""The server's search must normalize text exactly as the contract says.

shared-test-vectors/search-normalization.json is the frozen source of truth
for search normalization: a per-codepoint unaccent mapping (replacements may
be empty or several characters) then a per-codepoint lowercase mapping. It was
generated from PostgreSQL's unaccent and lower() before the move to SQLite
and is now edited deliberately, with a version bump. The server indexes and
matches text through ramekin_core::search::normalize_for_search, and iOS local
search mirrors the same asset, so any drift between the asset and what the
server stores silently breaks local-versus-server search parity.
"""

import json
from pathlib import Path

import pytest
from conftest import connect_db, make_ingredient
from ramekin_client.api import RecipesApi
from ramekin_client.models import CreateRecipeRequest

REPO_ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = REPO_ROOT / "shared-test-vectors" / "search-normalization.json"

# Strings mixing precomposed accents, decomposed combining marks, ligatures,
# presentation forms, vulgar fractions, fullwidth forms, caseful scripts the
# unaccent mapping does not touch, and characters with no mapping at all.
COMPOSITION_SAMPLES = [
    "Crème Brûlée",
    "Créme Brûlée",
    "Œufs en Meurette",
    "ﬁnely chopped, 1½ cups",
    "ＦＵＬＬＷＩＤＴＨ ﬀﬁﬂ",
    "İstanbul ΣΟΥΠΕΣ σούπες ß ẞ",
    "Mom’s “Sweet–and–Sour” Apple Pie",
    "plain ascii 123 %_\\",
    "no mapping: 日本語 한국어 🍞",
]


@pytest.fixture(scope="module")
def contract():
    return json.loads(CONTRACT_PATH.read_text())


def _codepoint(key: str) -> str:
    return chr(int(key, 16))


def _normalize(contract, text: str) -> str:
    unaccent = {_codepoint(k): v for k, v in contract["unaccent"].items()}
    lower = {_codepoint(k): v for k, v in contract["lower"].items()}
    return "".join(lower.get(c, c) for c in "".join(unaccent.get(c, c) for c in text))


def _indexed_title(authed_api_client, database_url, title: str) -> str:
    """Save a recipe titled `title`; return the title text the search index holds."""
    client, _user_id = authed_api_client
    created = RecipesApi(client).create_recipe(
        CreateRecipeRequest(
            title=title,
            instructions="Cook it.",
            ingredients=[make_ingredient(item="water")],
        )
    )
    conn = connect_db(database_url)
    try:
        (indexed,) = conn.execute(
            "SELECT title FROM recipe_search"
            " WHERE rowid = (SELECT seq FROM recipes WHERE id = ?)",
            (str(created.id),),
        ).fetchone()
    finally:
        conn.close()
    return indexed


def test_server_normalizes_every_mapped_codepoint_like_the_contract(
    authed_api_client, database_url, contract
):
    """Every codepoint either table maps, through the server's normalizer."""
    keys = sorted(
        set(contract["unaccent"]) | set(contract["lower"]), key=lambda k: int(k, 16)
    )
    title = " ".join(_codepoint(k) for k in keys)
    assert _indexed_title(authed_api_client, database_url, title) == _normalize(
        contract, title
    )


@pytest.mark.parametrize("sample", COMPOSITION_SAMPLES)
def test_server_normalizes_whole_strings_like_the_contract(
    authed_api_client, database_url, contract, sample
):
    """The clients apply the contract codepoint by codepoint; the server must
    too, on strings mixing decomposed accents, ligatures and unmapped scripts."""
    assert _indexed_title(authed_api_client, database_url, sample) == _normalize(
        contract, sample
    )


def test_lower_replacements_are_single_codepoints(contract):
    """The clients store the lower table as a codepoint-to-codepoint map."""
    for key, replacement in contract["lower"].items():
        assert len(replacement) == 1, key


def test_contract_version_is_positive(contract):
    assert contract["version"] >= 1
