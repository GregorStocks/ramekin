# Classifying unresolved ingredient names into the catalog

How to grow `ramekin-core/src/catalog/data/curated.json` with Claude Code
subagents, so more ingredient lines resolve for calories and density. First run:
step 2 tier 1 (2026-09-28, top 2,000 names). See the "Classification passes"
section of `ramekin-core/src/catalog/README.md` for the log of passes.

## 1. Export the work queue

```
make ingredient-catalog-unresolved PROD_RECIPES=<prod dump, optional>
```

This writes `logs/catalog-unresolved.json`: every normalized name that
`catalog::resolve` leaves `Unresolved`, most frequent first. Each name has:
- per-corpus counts (pipeline fixtures, Paprika fixtures, prod);
- up to 3 raw example lines.

Ambiguous names are deliberate and excluded. The prod dump format is in
`data/README.md` (ingredient-catalog-audit.md section).

## 2. Classify with a workflow

Run one Workflow with two phases. The tier-1 script is the reference; copy it from the
session's workflow scripts directory or rebuild it from the description below.

### Classify

N agents, each reading its slice of the queue:

```
jq '.[START:END] | map({name, count, examples})' logs/catalog-unresolved.json
```

Each agent returns one schema-validated decision per name:

| action | fields | meaning |
| --- | --- | --- |
| `alias` | `target` | a curated entry id, USDA stripped name (`usda.json` `.names`), or unique USDA description; never another alias |
| `entry` | `fdc_id`, optional `grams_per_cup_value` + `grams_per_cup_source` | a real USDA food whose density must be borrowed (like "dried thyme") |
| `product` | `category` | a purchasable non-food (Household, …) |
| `not_food` | `reason` | a note or heading, not an ingredient |
| `ambiguous` | `reason` | spans foods with very different calories ("cheese") |
| `skip` | `reason` | not confident, no USDA match, or a compound ("x and y") |

The prompt carries the README's alias rules plus the lessons from review:
- use the real food, not a stand-in, when USDA has it;
- alternatives ("x or y") take the first-listed food only if they are close in calories
  or trace amounts; otherwise `ambiguous` ("heavy cream or milk");
- bone-in cuts and whole birds are `ambiguous`: the recipe weight includes bones USDA
  doesn't count;
- fresh vs dried;
- specific parts (egg yolk, lemon peel);
- never invent numbers;
- prefer `skip` over a guess.

Agents are read-only and look things up with `jq`/`grep` on `usda.json` and
`curated.json`.

### Verify

Three skeptical agents run over the merged decisions. Their corrections replace the original
decision.

- **High-calorie and new entries.** Every mapping to a food with ≥ 300 kcal/100 g, plus every
  entry or product.
- **Sample.** A deterministic ~1/7 sample of the other mapped names.
- **Consistency.** Cross-shard consistency: the same food mapped differently, fresh/dried or
  raw/cooked mix-ups, and singular/plural disagreements.

Review every correction by hand before applying.

## 3. Apply

```
make catalog-apply-classification FILE=<decisions.json>
```

`scripts/apply-catalog-classification.py` validates everything first. It rejects:
- names that aren't normalized, or already exist as catalog names;
- targets that aren't valid (including alias-to-alias);
- unknown FDC ids;
- products without a known category;
- borrowed densities without a source.

Any rejection fails the run and writes nothing. The Rust catalog loader's asserts
(`make test-core`) are the final gate.

## 4. Regenerate and review

```
make ingredient-tests-update
make pipeline
make ingredient-catalog-audit
```

- Fixture churn is expected: gram alternatives are added for newly resolved names. Existing
  gram values should only change where a previously unresolved name now resolves.
- Check `logs/ingredient-catalog-audit-local.md` for what's still unresolved, and the
  committed audit for the coverage change.
- Recompute `tests/ui/test_recipe_scale.py`'s expected subtotals if the scale-test recipe's
  ingredients change status.
- Record the pass (date, model, tier, counts, before/after numbers) in the catalog README.

## Promoting step 3 resolutions

Once runtime LLM resolution exists (`ingredient-catalog-step3-ingest-resolution`),
its `ingredient_resolutions` rows can be exported into the same decisions format.
Review them with the verify phase, then apply them the same way, so the runtime
table shrinks into committed data.
