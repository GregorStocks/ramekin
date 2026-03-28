---
name: identify-density-gap
description: Analyze the density gap report to find the highest-impact ingredients missing density data, filtering out intentionally-null entries and existing aliases. Use when deciding which ingredient to add next.
disable-model-invocation: true
---

# Identify High-Impact Density Gaps

Analyze the gap report and curated data to surface the best candidates for adding density data.

## Workflow

1. **Read the gap report** (`data/density-gap-report.txt`) and note the top ~30 entries by occurrence count
2. **Read curated.json** (`ingredient-density/src/data/curated.json`) to identify:
   - Ingredients already set to `null` (intentionally ambiguous) — skip these
   - Existing aliases that might cover variants of gap-report entries
3. **Search USDA data** (`ingredient-density/src/data/usda.json`) for entries that already exist under a different name — these are easy alias-only wins
4. **Group related variants** (e.g. "ground coriander" + "coriander" = combined count)
5. **Classify each candidate** into one of:
   - **Alias only**: USDA or curated data already has the density under another name; just needs alias(es)
   - **New entry, USDA available**: needs a curated entry but USDA FoodData Central has the data
   - **New entry, research needed**: not in USDA, will require external research
   - **Ambiguous**: should probably be set to `null` (e.g. "parmesan cheese" without grated/shredded qualifier)
6. **Present a ranked table** to the user with columns: ingredient family, total occurrences, classification, and notes
7. **Recommend the top pick** — prefer high-occurrence alias-only fixes first (highest impact, lowest effort), then USDA-available entries, then research-needed entries
8. **Ask the user to confirm**, then hand off to `/add-density`

## Key files

- `data/density-gap-report.txt` — ingredients missing density data, sorted by frequency
- `ingredient-density/src/data/curated.json` — hand-curated densities, aliases, and null markers
- `ingredient-density/src/data/usda.json` — USDA FoodData Central densities (auto-imported)
- `ingredient-density/src/density_lookup.rs` — lookup logic (to understand how modifier stripping and plural fallback work)

## What to skip

These are intentionally null/ambiguous and should always be filtered out:
- Black pepper variants (black pepper, ground black pepper, freshly ground black pepper, pepper, ground pepper)
- Salt variants without brand (sea salt, fine salt, fine sea salt, fine grain sea salt, fine sea or table salt)
- Any entry already present as a `null` alias in curated.json

## Example output

| Ingredient family | Occurrences | Classification | Notes |
|---|---|---|---|
| ground coriander + coriander | 87 | Alias only | USDA has "spices, coriander seed" at 80 g/cup |
| crushed red pepper flakes | 66 | Alias only | "red pepper flakes" alias exists but doesn't cover this exact variant |
| Italian seasoning | 63 | Research needed | Dried herb blend, not in USDA |
| parmesan cheese | 66 | Ambiguous | Grated (100g) vs shredded (80g) — may need null |
| dried basil | 43 | New entry, USDA available | USDA FDC 172232 |
