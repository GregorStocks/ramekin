# AI evals

Every AI use case should have a suite here: a fixed, committed input set and a
scorer, so a model change ships with its numbers.

- `golden/<suite>.json`: the inputs and expected answers. The ingredient suites
  are generated from the committed catalog and `text-extraction` from the
  pipeline snapshots (`make ai-eval-golden`); regenerate only on purpose, since
  it changes what earlier results measured. `photo-extraction` is transcribed by
  hand from the photos in `photos/` and isn't regenerated: to add a case, add
  the photo and its case (with a `note` on what makes it hard).
- `<suite>.md`: the latest results, one row per model, at production's batch
  size; `<suite>-batch-<n>.md` at other sizes (`BATCH=n`).
- `logs/ai-evals/<suite>/<model>.json` (not committed): every case's answer,
  for checking whether a miss is the model's or the measurement's.

Run `make ai-eval MODELS=a,b,c [SUITE=…] [BATCH=n]` with `OPENROUTER_API_KEY`
set (it reads `cli.env`). Each call goes through the production prompt and
validation, and responses are cached by model and prompt, so a rerun is free
and a new model only pays for its own calls. Rejected answers aren't cached, so
a model that answers invalidly is asked (and billed) again on every run. Models come from OpenRouter's live
model list, which also supplies the prices in the cost column. A model whose
calls fail (a timeout) stops the run before any report is written; rerun
without it, or after fixing the cause, which is nearly free since everything
answered is cached.

| Suite | Use case | Scored against |
| --- | --- | --- |
| `ingredient-weights` | `estimate_ingredient_weights` | USDA densities and piece weights |
| `food-estimates` | `resolve_ingredient_names` (estimate) | USDA calories, densities, default pieces |
| `ingredient-names` | `resolve_ingredient_names` (entry, not food) | Curated aliases and not-food names |
| `text-extraction` | `extract_recipe_from_text` | Pipeline snapshots rendered as pasted text, plus non-recipes |
| `photo-extraction` | `extract_recipe_from_photos` | Hand-checked transcriptions of real recipe photos |

Still to add (`p2-upgrade-ai-models`): human-judged suites (tags, titles,
descriptions, custom enrich, recipe photos) that produce a blind side-by-side
page for review.
