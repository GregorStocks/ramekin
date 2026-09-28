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
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOG = ROOT / "ramekin-core/src/catalog/data"
CATEGORIZER = ROOT / "ramekin-core/src/ingredient_categorizer.rs"
ACTIONS = {"alias", "entry", "product", "not_food", "ambiguous", "skip"}


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


def apply(curated: dict, usda: dict, categories: set[str], decisions: list[dict]):
    """Return (updated curated, counts, rejections) without mutating inputs."""
    curated = json.loads(json.dumps(curated))
    fdc_ids = {food["fdc_id"] for food in usda["foods"]}
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
        decisions, key=lambda d: d.get("action") not in ("entry", "product")
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
            value = decision.get("grams_per_cup_value")
            if value is not None:
                source = decision.get("grams_per_cup_source")
                if not (isinstance(value, (int, float)) and value > 0 and source):
                    reject("grams_per_cup needs a positive value and a source")
                    continue
                entry["grams_per_cup"] = {"value": float(value), "source": source}
            curated["entries"][name] = entry
            targets.add(name)
    return curated, counts, rejections


def main() -> None:
    if len(sys.argv) != 2:
        sys.exit("usage: apply-catalog-classification.py DECISIONS.json")
    curated_path = CATALOG / "curated.json"
    curated = json.loads(curated_path.read_text(encoding="utf-8"))
    usda = json.loads((CATALOG / "usda.json").read_text(encoding="utf-8"))
    categories = shopping_categories(CATEGORIZER.read_text(encoding="utf-8"))
    updated, counts, rejections = apply(
        curated, usda, categories, load_decisions(Path(sys.argv[1]))
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
