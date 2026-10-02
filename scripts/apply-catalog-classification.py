#!/usr/bin/env python3
"""
Merge ingredient classification decisions into the catalog's curated.json.

Decisions come from a classification pass (see
docs/agent/catalog-classification.md): a JSON list of objects, or an object
with a "decisions" list, each with "name", "action", and action-specific
fields. Every decision is validated before anything is written; a single
invalid decision fails the run and leaves curated.json untouched.

Usage:
    make catalog-apply-classification FILE=path/to/decisions.json
"""

import json
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "ramekin-core/src/catalog/data"
CATEGORIZER = ROOT / "ramekin-core/src/ingredient_categorizer.rs"
ACTIONS = {
    "alias",
    "entry",
    "food",
    "trace",
    "product",
    "not_food",
    "ambiguous",
    "skip",
}


def normalize(text: str) -> str:
    """Lowercase and collapse whitespace, matching catalog::normalize."""
    return " ".join(text.lower().split())


def shopping_categories(source: str) -> set[str]:
    """The CATEGORIES array from ingredient_categorizer.rs."""
    match = re.search(r"pub const CATEGORIES: \[&str; \d+\] = \[(.*?)\];", source, re.S)
    if match is None:
        raise ValueError("CATEGORIES not found in ingredient_categorizer.rs")
    return set(re.findall(r'"([^"]+)"', match.group(1)))


def load_decisions(path: Path) -> list[dict]:
    data = json.loads(path.read_text(encoding="utf-8"))
    decisions = data["decisions"] if isinstance(data, dict) else data
    if not isinstance(decisions, list):
        raise ValueError("expected a list of decisions")
    return decisions


def cited(decision: dict, field: str) -> dict | None:
    """A positive `<field>_value` with its `<field>_source` (and optional
    `<field>_url`), or None when the decision gives no value."""
    value = decision.get(f"{field}_value")
    if value is None:
        return None
    source = decision.get(f"{field}_source")
    if not (
        isinstance(value, (int, float))
        and not isinstance(value, bool)
        and math.isfinite(value)
        and value > 0
        and isinstance(source, str)
        and source.strip()
    ):
        raise ValueError(f"{field} needs a positive value and a source")
    out = {"value": float(value), "source": source}
    url = decision.get(f"{field}_url")
    if url is not None:
        if not (isinstance(url, str) and url.startswith("https://")):
            raise ValueError(f"{field} url must be an https:// string")
        out["url"] = url
    return out


def apply(
    curated: dict,
    usda: dict,
    categories: set[str],
    decisions: list[dict],
    fndds: dict | None = None,
):
    """Return (updated curated, counts, rejections) without mutating inputs.

    `entry` decisions may link an SR Legacy or FNDDS food; `food` decisions
    add a hand-curated food with cited calories for foods neither has.
    """
    curated = json.loads(json.dumps(curated))
    fdc_ids = {food["fdc_id"] for food in usda["foods"]}
    fdc_ids |= {food["fdc_id"] for food in (fndds or {"foods": []})["foods"]}
    description_counts: dict[str, int] = {}
    for food in usda["foods"]:
        description_counts[food["description"]] = (
            description_counts.get(food["description"], 0) + 1
        )
    unique_descriptions = {d for d, n in description_counts.items() if n == 1}
    # Names an alias may point at: never another alias (the resolver doesn't chain).
    targets = set(curated["entries"]) | set(usda["names"]) | unique_descriptions
    taken = (
        targets
        | set(description_counts)
        | set(curated["aliases"])
        | set(curated["not_food"])
    )

    counts = {action: 0 for action in sorted(ACTIONS)}
    rejections = []
    seen = set()
    # New entries and products first, so aliases in the same batch can target them.
    ordered = sorted(
        decisions,
        key=lambda d: d.get("action") not in ("entry", "food", "trace", "product"),
    )
    for decision in ordered:
        name = decision.get("name", "")
        action = decision.get("action")

        def reject(why: str, name=name) -> None:
            rejections.append(f"{name!r}: {why}")

        if action not in ACTIONS:
            reject(f"unknown action {action!r}")
            continue
        if name != normalize(name) or not name:
            reject("name is not normalized")
            continue
        if name in seen:
            reject("duplicate decision")
            continue
        seen.add(name)
        counts[action] += 1
        if action == "skip":
            continue
        if name in taken:
            reject("already a catalog name")
            continue
        if action == "alias":
            target = decision.get("target")
            if target not in targets:
                reject(
                    f"target {target!r} is not an entry, USDA name, "
                    "or unique description"
                )
                continue
            curated["aliases"][name] = target
        elif action == "ambiguous":
            curated["aliases"][name] = None
        elif action == "not_food":
            if not decision.get("reason"):
                reject("not_food needs a reason")
                continue
            curated["not_food"][name] = decision["reason"]
        elif action == "product":
            category = decision.get("category")
            if category not in categories:
                reject(f"unknown category {category!r}")
                continue
            curated["entries"][name] = {"kind": "product", "category": category}
            targets.add(name)
        elif action == "entry":
            fdc_id = decision.get("fdc_id")
            if fdc_id not in fdc_ids:
                reject(f"unknown fdc_id {fdc_id!r}")
                continue
            entry: dict = {"fdc_id": fdc_id}
            try:
                density = cited(decision, "grams_per_cup")
            except ValueError as error:
                reject(str(error))
                continue
            if density is not None:
                entry["grams_per_cup"] = density
            curated["entries"][name] = entry
            targets.add(name)
        elif action == "trace":
            # A spice or herb no source gives citable calories for: small
            # amounts are negligible, anything more stays unknown.
            if not decision.get("reason"):
                reject("trace needs a reason")
                continue
            curated["entries"][name] = {"trace_ok": True}
            targets.add(name)
        elif action == "food":
            try:
                kcal = cited(decision, "kcal_per_100g")
                density = cited(decision, "grams_per_cup")
            except ValueError as error:
                reject(str(error))
                continue
            if kcal is None or "url" not in kcal:
                reject("food needs kcal_per_100g with a source and url")
                continue
            entry = {"kcal_per_100g": kcal}
            if density is not None:
                entry["grams_per_cup"] = density
            trace_ok = decision.get("trace_ok", False)
            if not isinstance(trace_ok, bool):
                reject("trace_ok must be true or false")
                continue
            if trace_ok:
                entry["trace_ok"] = True
            curated["entries"][name] = entry
            targets.add(name)
    return curated, counts, rejections


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: apply-catalog-classification.py DECISIONS.json")
    curated_path = CATALOG / "curated.json"
    curated = json.loads(curated_path.read_text(encoding="utf-8"))
    usda = json.loads((CATALOG / "usda.json").read_text(encoding="utf-8"))
    fndds = json.loads((CATALOG / "fndds.json").read_text(encoding="utf-8"))
    categories = shopping_categories(CATEGORIZER.read_text(encoding="utf-8"))
    updated, counts, rejections = apply(
        curated, usda, categories, load_decisions(Path(sys.argv[1])), fndds
    )
    if rejections:
        print(f"{len(rejections)} invalid decision(s); curated.json not written:")
        for rejection in rejections:
            print(f"  {rejection}")
        sys.exit(1)
    curated_path.write_text(
        json.dumps(updated, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    applied = sum(n for action, n in counts.items() if action != "skip")
    print(f"Applied {applied} decisions ({counts}) to {curated_path.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
