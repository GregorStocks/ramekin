"""The checked-in search normalization contract must match the live database.

shared-test-vectors/search-normalization.json pins the complete per-codepoint
unaccent dictionary (what f_unaccent applies) and per-codepoint lower()
mapping (what ILIKE's case-insensitivity and CITEXT equality apply). The Rust
scorer and the iOS local search both normalize with that asset, so any drift
between the asset and the running PostgreSQL server silently breaks
local-versus-server search parity. Regenerate with
scripts/generate-search-normalization.sh and bump the version when this fails
after a PostgreSQL upgrade.
"""

import json
from pathlib import Path

import psycopg
import pytest

REPO_ROOT = Path(__file__).resolve().parents[1]
CONTRACT_PATH = REPO_ROOT / "shared-test-vectors" / "search-normalization.json"

# Every Unicode codepoint except the surrogate range, which chr() rejects.
ALL_CODEPOINTS_SQL = (
    "select cp from generate_series(1, 1114111) cp where cp < 55296 or cp > 57343"
)

# Strings mixing precomposed accents, decomposed combining marks, ligatures,
# presentation forms, vulgar fractions, fullwidth forms, caseful scripts the
# unaccent dictionary does not touch, and characters with no mapping at all.
COMPOSITION_SAMPLES = [
    "Crème Brûlée",
    "Créme Brûlée",
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


def _apply_unaccent(contract, text: str) -> str:
    unaccent = {_codepoint(k): v for k, v in contract["unaccent"].items()}
    return "".join(unaccent.get(c, c) for c in text)


def _apply_lower(contract, text: str) -> str:
    lower = {_codepoint(k): v for k, v in contract["lower"].items()}
    return "".join(lower.get(c, c) for c in text)


def test_unaccent_mapping_matches_database_for_every_codepoint(database_url, contract):
    """Both directions: every asset mapping is what f_unaccent does, and no
    codepoint the database rewrites is missing from the asset."""
    with psycopg.connect(database_url) as conn:
        rows = conn.execute(
            f"""
            with cps as ({ALL_CODEPOINTS_SQL})
            select to_hex(cp), unaccent('public.unaccent', chr(cp)) from cps
            where unaccent('public.unaccent', chr(cp)) is distinct from chr(cp)
            """
        ).fetchall()
    db_mapping = {key.rjust(4, "0"): replacement for key, replacement in rows}
    assert db_mapping == contract["unaccent"]


def test_lower_mapping_matches_database_for_every_codepoint(database_url, contract):
    with psycopg.connect(database_url) as conn:
        rows = conn.execute(
            f"""
            with cps as ({ALL_CODEPOINTS_SQL})
            select to_hex(cp), lower(chr(cp)) from cps
            where lower(chr(cp)) is distinct from chr(cp)
            """
        ).fetchall()
    db_mapping = {key.rjust(4, "0"): replacement for key, replacement in rows}
    assert db_mapping == contract["lower"]


def test_contract_applies_per_codepoint_to_whole_strings(database_url, contract):
    """The clients apply the contract codepoint by codepoint. That is only
    faithful if the database's functions also act per codepoint on full
    strings (no multi-character rules), so pin that property on strings that
    mix decomposed accents, ligatures, and unmapped scripts."""
    with psycopg.connect(database_url) as conn:
        for sample in COMPOSITION_SAMPLES:
            (unaccented,) = conn.execute(
                "select unaccent('public.unaccent', %s)", (sample,)
            ).fetchone()
            assert unaccented == _apply_unaccent(contract, sample), repr(sample)
            (lowered,) = conn.execute("select lower(%s)", (sample,)).fetchone()
            assert lowered == _apply_lower(contract, sample), repr(sample)
            # And the full pipeline the search filter uses:
            # lower(f_unaccent(text)) is what ILIKE compares.
            (normalized,) = conn.execute(
                "select lower(f_unaccent(%s))", (sample,)
            ).fetchone()
            assert normalized == _apply_lower(
                contract, _apply_unaccent(contract, sample)
            ), repr(sample)


def test_lower_replacements_are_single_codepoints(contract):
    """The clients store the lower table as a codepoint-to-codepoint map."""
    for key, replacement in contract["lower"].items():
        assert len(replacement) == 1, key


def test_contract_version_is_positive(contract):
    assert contract["version"] >= 1
