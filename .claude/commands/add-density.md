# Add Density for a High-Priority Ingredient

Pick one high-impact ingredient from the density gap report, find a reliable density, and add it.

## Workflow

1. **Read the gap report** (`data/density-gap-report.txt`) and identify the top ~20 entries
2. **Skip** ingredients that are intentionally null/ambiguous in `curated.json` (black pepper variants, sea salt, fine salt, etc.)
3. **Pick one** high-occurrence ingredient (or family of related variants) that can be resolved, and **ask the user to confirm your choice before proceeding**
4. **Research the density:**
   - Best: USDA FoodData Central (`fdc.nal.usda.gov`) with FDC ID and URL
   - Good: reputable cooking reference (King Arthur, Serious Eats) with URL
   - OK: estimation with documented rationale (e.g. "comparable to X which is Y g/cup")
   - Check if the ingredient is already in `usda.json` under a different name (just needs an alias)
5. **Edit `ingredient-density/src/data/curated.json`:**
   - If it's a new density: add to `"ingredients"` with `grams_per_cup`, `source`, and `url`
   - If it's an alias to existing data: add to `"aliases"`
   - Add all common spelling variants you see in the gap report
6. **Add tests** in `ingredient-density/src/density_lookup.rs` in the appropriate test function
7. **Update golden files:** `make ingredient-tests-update`
8. **Verify:** `make test` and `make lint`
9. **Regenerate gap report:** `make pipeline`
10. **Commit and create a PR**

## Key files

- `ingredient-density/src/data/curated.json` — hand-curated densities and aliases (overrides USDA)
- `ingredient-density/src/data/usda.json` — USDA FoodData Central densities (auto-imported)
- `ingredient-density/src/density_lookup.rs` — lookup logic, modifier stripping, tests
- `data/density-gap-report.txt` — ingredients missing density data, sorted by frequency

## Density format

All densities are **grams per US cup** (236.588 ml). To convert:
- From g/tbsp: multiply by 16
- From g/tsp: multiply by 48

## Important

- Never guess. If you can't find a reliable source, mark the ingredient as `null` (ambiguous) rather than estimating poorly.
- One ingredient family per PR.
- Stop after creating the PR.
