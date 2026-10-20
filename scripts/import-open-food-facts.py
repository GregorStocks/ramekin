#!/usr/bin/env python3
"""
Import shopping aisles for food names from Open Food Facts.

Writes ramekin-core/src/catalog/data/open_food_facts.json: a normalized name ->
shopping aisle index the categorizer consults for names the catalog doesn't
know ("cheerios", "cheez it", "rice cakes"). Names come from two sources:

- the category taxonomy's English names and synonyms ("Breakfast cereals",
  "Oat rings");
- US products' names and brands, where at least MIN_PRODUCTS products give an
  aisle and at least MIN_SHARE of them agree on it. A brand other brands'
  product names use as an ordinary word ("Sprout", "Fresh") is left out.

A category's aisle is that of its nearest ancestor (itself first) listed in
open_food_facts_aisles.json; among listed ancestors equally near, the earlier
entry wins. The taxonomy wins over product names, which win over brands.

Open Food Facts publishes daily exports rather than pinned releases, so the
downloads are cached in .cache/ and their sha256s recorded in the output; rerun
with --refresh to download them again.

Contains data from Open Food Facts, available under the Open Database License
(https://opendatacommons.org/licenses/odbl/1.0/). The generated file is a
derived database under the same license.

Usage:
    make catalog-import-off [REFRESH=1]
"""

import collections
import csv
import gzip
import hashlib
import json
import re
import shutil
import sys
import urllib.request
from collections.abc import Iterable
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TAXONOMY_SOURCE = (
    "https://static.openfoodfacts.org/data/taxonomies/categories.full.json"
)
PRODUCTS_SOURCE = (
    "https://static.openfoodfacts.org/data/en.openfoodfacts.org.products.csv.gz"
)
TAXONOMY_CACHE = ROOT / ".cache/open-food-facts-categories.full.json"
PRODUCTS_CACHE = ROOT / ".cache/open-food-facts-products.csv.gz"
MAPPING = ROOT / "ramekin-core/src/catalog/data/open_food_facts_aisles.json"
CATEGORIZER = ROOT / "ramekin-core/src/ingredient_categorizer.rs"
OUTPUT = ROOT / "ramekin-core/src/catalog/data/open_food_facts.json"
ATTRIBUTION = (
    "Contains data from Open Food Facts (https://world.openfoodfacts.org), "
    "available under the Open Database License "
    "(https://opendatacommons.org/licenses/odbl/1.0/)."
)
COUNTRY = "en:united-states"
# A product name or brand needs this many products with an aisle, and this
# share of them agreeing, to name an aisle.
MIN_PRODUCTS = 3
MIN_SHARE = 0.6
# A brand is a generic word ("Sprout" baby food against brussels sprouts, "Fresh",
# "Garden") when other brands' product names use it at least this often per
# product of its own; its key would misplace the ordinary food. Real brands stay
# under 7 (a parent company's products name them: Kraft's say "Velveeta"), and
# generic words measured 22 and up.
GENERIC_BRAND_USE = 10


def name_key(text: str) -> str:
    """Lowercase, drop apostrophes, and turn other punctuation into spaces,
    matching `open_food_facts::key` in the catalog."""
    text = text.lower().replace("'", "").replace("’", "")
    return " ".join(re.sub(r"[^\w]|_", " ", text).split())


def shopping_categories(source: str) -> set[str]:
    """The CATEGORIES array from ingredient_categorizer.rs."""
    match = re.search(r"pub const CATEGORIES: \[&str; \d+\] = \[(.*?)\];", source, re.S)
    if match is None:
        raise ValueError("CATEGORIES not found in ingredient_categorizer.rs")
    return set(re.findall(r'"([^"]+)"', match.group(1)))


def load_mapping(
    mapping: dict, taxonomy: dict, categories: set[str]
) -> list[tuple[str, str]]:
    pairs = [tuple(pair) for pair in mapping["aisles"]]
    seen = set()
    for category, aisle in pairs:
        if category in seen:
            raise ValueError(f"{category} is listed twice")
        seen.add(category)
        if category not in taxonomy:
            raise ValueError(f"{category} is not an Open Food Facts category")
        if aisle not in categories:
            raise ValueError(f"{category} maps to unknown aisle {aisle!r}")
    return pairs


class Aisles:
    """Each taxonomy category's aisle, by its nearest listed ancestor."""

    def __init__(self, taxonomy: dict, pairs: list[tuple[str, str]]):
        self.taxonomy = taxonomy
        self.aisle = dict(pairs)
        self.rank = {category: index for index, (category, _) in enumerate(pairs)}
        self.memo: dict[str, str | None] = {}

    def of(self, category: str) -> str | None:
        if category not in self.memo:
            self.memo[category] = self._nearest(category)
        return self.memo[category]

    def _nearest(self, category: str) -> str | None:
        frontier, seen = [category], {category}
        while frontier:
            listed = [c for c in frontier if c in self.aisle]
            if listed:
                return self.aisle[min(listed, key=self.rank.__getitem__)]
            parents = []
            for c in frontier:
                for parent in self.taxonomy.get(c, {}).get("parents", []):
                    if parent not in seen:
                        seen.add(parent)
                        parents.append(parent)
            frontier = parents
        return None


def taxonomy_names(taxonomy: dict, aisles: Aisles) -> dict[str, str]:
    """English category names and synonyms whose categories agree on an aisle."""
    found: dict[str, set[str | None]] = collections.defaultdict(set)
    for category, node in taxonomy.items():
        english = [node.get("name", {}).get("en")] + node.get("synonyms", {}).get(
            "en", []
        )
        for name in filter(None, english):
            key = name_key(name)
            if key:
                found[key].add(aisles.of(category))
    return {
        key: next(iter(found_aisles))
        for key, found_aisles in found.items()
        if len(found_aisles) == 1 and None not in found_aisles
    }


def product_votes(
    rows: Iterable[dict], aisles: Aisles
) -> tuple[
    dict[str, collections.Counter],
    dict[str, collections.Counter],
    list[tuple[str, frozenset[str]]],
]:
    """Aisle votes per US product name and per brand, by main category, and
    every US product's name and brands."""
    names: dict[str, collections.Counter] = collections.defaultdict(collections.Counter)
    brands: dict[str, collections.Counter] = collections.defaultdict(
        collections.Counter
    )
    products = []
    for row in rows:
        if COUNTRY not in (row.get("countries_tags") or "").split(","):
            continue
        name = name_key(row.get("product_name") or "")
        product_brands = frozenset(
            filter(None, map(name_key, (row.get("brands_en") or "").split(",")))
        )
        products.append((name, product_brands))
        aisle = aisles.of(row.get("main_category") or "")
        if aisle is None:
            continue
        if name:
            names[name][aisle] += 1
        for brand in product_brands:
            brands[brand][aisle] += 1
    return names, brands, products


def generic_brands(
    brands: dict[str, collections.Counter], products: list[tuple[str, frozenset[str]]]
) -> set[str]:
    """Brands other brands' product names use as a word, as often as
    GENERIC_BRAND_USE per product of their own, singular or plural."""
    forms = {}
    for brand in brands:
        for form in (brand, brand + "s", brand.removesuffix("s")):
            forms.setdefault(form, set()).add(brand)
    longest = max((len(form.split()) for form in forms), default=0)
    used: collections.Counter = collections.Counter()
    for name, product_brands in products:
        words = name.split()
        mentioned = set()
        for start in range(len(words)):
            for end in range(start + 1, min(len(words), start + longest) + 1):
                mentioned |= forms.get(" ".join(words[start:end]), set())
        used.update(mentioned - product_brands)
    return {
        brand
        for brand, votes in brands.items()
        if used[brand] >= GENERIC_BRAND_USE * sum(votes.values())
    }


def agreed(votes: collections.Counter) -> str | None:
    total = sum(votes.values())
    if total < MIN_PRODUCTS:
        return None
    aisle, count = votes.most_common(1)[0]
    return aisle if count / total >= MIN_SHARE else None


def build_names(
    taxonomy: dict, pairs: list[tuple[str, str]], rows: Iterable[dict]
) -> dict[str, str]:
    aisles = Aisles(taxonomy, pairs)
    index = taxonomy_names(taxonomy, aisles)
    names, brands, products = product_votes(rows, aisles)
    brands = {brand: votes for brand, votes in brands.items() if agreed(votes)}
    for generic in generic_brands(brands, products):
        del brands[generic]
    for votes_by_name in (names, brands):
        for name, votes in votes_by_name.items():
            aisle = agreed(votes)
            if aisle is not None:
                index.setdefault(name, aisle)
    return dict(sorted(index.items()))


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def fetch(source: str, cache: Path, refresh: bool) -> Path:
    if refresh or not cache.exists():
        cache.parent.mkdir(parents=True, exist_ok=True)
        partial = cache.with_suffix(cache.suffix + ".partial")
        with (
            urllib.request.urlopen(source, timeout=120) as response,
            partial.open("wb") as out,
        ):
            shutil.copyfileobj(response, out, 1 << 20)
        partial.replace(cache)
    return cache


def product_rows(path: Path) -> Iterable[dict]:
    csv.field_size_limit(1 << 30)
    with gzip.open(
        path, "rt", encoding="utf-8", errors="replace", newline=""
    ) as stream:
        yield from csv.DictReader(stream, delimiter="\t", quoting=csv.QUOTE_NONE)


def main(argv: list[str]) -> None:
    refresh = "--refresh" in argv
    taxonomy_path = fetch(TAXONOMY_SOURCE, TAXONOMY_CACHE, refresh)
    products_path = fetch(PRODUCTS_SOURCE, PRODUCTS_CACHE, refresh)
    taxonomy = json.loads(taxonomy_path.read_text(encoding="utf-8"))
    pairs = load_mapping(
        json.loads(MAPPING.read_text(encoding="utf-8")),
        taxonomy,
        shopping_categories(CATEGORIZER.read_text(encoding="utf-8")),
    )
    names = build_names(taxonomy, pairs, product_rows(products_path))
    data = {
        "attribution": ATTRIBUTION,
        "sources": {
            TAXONOMY_SOURCE: sha256(taxonomy_path),
            PRODUCTS_SOURCE: sha256(products_path),
        },
        "generation_notes": (
            "Generated by scripts/import-open-food-facts.py (make catalog-import-off). "
            "See ramekin-core/src/catalog/README.md."
        ),
        "names": names,
    }
    OUTPUT.write_text(
        json.dumps(data, indent=1, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    print(f"Imported {len(names)} names into {OUTPUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main(sys.argv[1:])
