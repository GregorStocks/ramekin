# AI evals

Every AI use case should have a suite here: a fixed, committed input set and a
scorer, so a model change ships with its numbers.

- `golden/<suite>.json`: the inputs and expected answers. The ingredient suites
  are generated from the committed catalog and `text-extraction` from the
  pipeline snapshots (`make ai-eval-golden`); regenerate only on purpose, since
  it changes what earlier results measured. `photo-extraction` is transcribed by
  hand from the photos in `photos/` and isn't regenerated: to add a case, add
  the photo and its case (with a `note` on what makes it hard). The judged
  suites (below) are sampled from the pipeline snapshots, except
  `custom-enrich`, whose recipes and instructions are paired by hand in
  `cli/src/ai_eval/judged.rs`.
- `judgments/<suite>.json`: a person's judgments for the judged suites, which
  are their golden answers.
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
answered is cached. The exception is a photo that outlasts photo generation's 60 s timeout:
production fails that photo too, so it counts as the model's rejected answer
and the run goes on.

| Suite | Use case | Scored against |
| --- | --- | --- |
| `ingredient-weights` | `estimate_ingredient_weights` | USDA densities and piece weights |
| `food-estimates` | `resolve_ingredient_names` (estimate) | USDA calories, densities, default pieces |
| `ingredient-names` | `resolve_ingredient_names` (entry, not food) | Curated aliases and not-food names |
| `text-extraction` | `extract_recipe_from_text` | Pipeline snapshots rendered as pasted text, plus non-recipes |
| `photo-extraction` | `extract_recipe_from_photos` | Hand-checked transcriptions of real recipe photos |
| `tags` | `suggest_tags` | The tags a person ticked for each recipe |
| `titles` | `normalize_title` | Judged answers |
| `descriptions` | `generate_description` | Judged answers |
| `custom-enrich` | `custom_enrich` | Judged answers |
| `recipe-photos` | `generate_recipe_photo` (image models) | Judged photos |

`SUITE=all` runs every suite but `recipe-photos`, which takes image models.

## Judged suites

No rule can score a tag set, a title, prose or a photo, so a person judges
them. Each run writes a blind page per suite,
`logs/ai-evals/<suite>/judge.html`. It shows each case's source and the answers
nobody has judged yet, shuffled, with model names hidden and identical answers
merged. `make ai-eval-judge` serves the pages (`HOST=` to reach them from
another machine); mark each answer best, good or bad (for tags, tick the tags
that apply to each recipe), then click Save, which writes
`judgments/<suite>.json` (opened as a file instead, the page copies the JSON
for you to paste there). Rerun: the report then scores every
model, and the page holds only what's still unjudged. Your clicks survive a
reload of the page. Beside it, `results.html` shows every model's answers
named and with their judgments, one model at a time or side by side.

Who judged: titles, descriptions, recipe photos and 13 of the 49
custom-enrich answers were judged by the maintainer; the tag sets and the
other 36 custom-enrich answers by Claude, at the maintainer's request.

Pick image candidates from the public blind-vote leaderboards (the LMArena
and Artificial Analysis text-to-image arenas), not from OpenRouter's catalog
alone: its default model list leaves out image-only models.

Answers are keyed by a hash of the answer (the photo's bytes for photos), so a
new model only adds its own new answers to the page. Tags are judged once per
recipe, so every later model is scored against that set with nothing more to
judge. Generated photos are cached under the AI cache directory by model and
prompt, since OpenRouter doesn't cache them.
