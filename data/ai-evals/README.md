# AI evals

Every AI use case should have a suite here: a fixed, committed input set and a
scorer, so a model change ships with its numbers.

- `golden/<suite>.json`: the inputs and expected answers. The ingredient suites
  are generated from the committed catalog (`make ai-eval-golden`); regenerate
  only on purpose, since it changes what earlier results measured.
- `<suite>.md`: the latest results, one row per model, at production's batch
  size; `<suite>-batch-<n>.md` at other sizes.

Run `make ai-eval MODELS=a,b,c [SUITE=…] [BATCH=n]` with `OPENROUTER_API_KEY`
set (it reads `cli.env`). Each call goes through the production prompt and
validation, and responses are cached by model and prompt, so a rerun is free
and a new model only pays for its own calls. Rejected answers aren't cached, so
a model that answers invalidly is asked (and billed) again on every run. Models come from OpenRouter's live
model list, which also supplies the prices in the cost column. A model that
fails (a timeout) gets a row saying so rather than stopping the run.

| Suite | Use case | Scored against |
| --- | --- | --- |
| `ingredient-weights` | `estimate_ingredient_weights` | USDA densities and piece weights |
| `food-estimates` | `resolve_ingredient_names` (estimate) | USDA calories, densities, default pieces |
| `ingredient-names` | `resolve_ingredient_names` (entry, not food) | Curated aliases and not-food names |

Still to add (`p2-upgrade-ai-models`): text and photo extraction, and
human-judged suites (tags, titles, descriptions, custom enrich, recipe photos)
that produce a blind side-by-side page for review.
