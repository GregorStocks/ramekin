# AI eval: Tags

Written by `make ai-eval`. Tags picked from a user's tags for a recipe (`suggest_tags`), for 20 pipeline snapshots and a fixed vocabulary of 33 tags (`TAG_VOCABULARY`), against the tags a person ticked as applying (`data/ai-evals/judgments/tags.json`). Exact is the recipes answered with exactly the judged tags; precision and recall count tags over all judged recipes. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun.

Golden set: `data/ai-evals/golden/tags.json` (20 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries, and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Exact | Precision | Recall | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 5% | 68% | 74% | 0 | 0% | 0 | 0 | $0.0061 |
| google/gemini-3.8-flash | 20% | 74% | 97% | 0 | 0% | 0 | 0 | $0.0553 |
| google/gemini-3.5-flash-lite | 10% | 73% | 82% | 0 | 0% | 0 | 0 | $0.0062 |
| anthropic/claude-haiku-4.5 | 0% | 77% | 74% | 0 | 0% | 0 | 0 | $0.0223 |
| anthropic/claude-sonnet-5.5 | 30% | 81% | 93% | 0 | 0% | 0 | 0 | $0.0686 |
| deepseek/deepseek-v4.1-flash | 0% | 64% | 93% | 0 | 0% | 0 | 0 | $0.0646 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- smittenkitchen-com_2015-10-salted-peanut-butter-cookies: missed ["dairy-free", "gluten-free"], extra ["comfort food", "make-ahead", "snack"]
- seriouseats-com_loubia-moroccan-white-bean-stew-recipe-11685571: missed ["dairy-free", "gluten-free", "make-ahead", "one-pot"], extra ["soup"]
- smittenkitchen-com_2024-11-halloumi-and-fall-vegetable-roast: missed ["gluten-free"], extra ["lunch", "one-pot", "side dish"]
- virtualweberbullet-com_brisket-high-heat: missed ["dairy-free", "gluten-free"], extra ["comfort food", "make-ahead"]
- smittenkitchen-com_2021-02-baked-feta-with-tomatoes-and-chickpeas: missed [], extra ["comfort food", "gluten-free", "lunch", "side dish"]

### google/gemini-3.8-flash

- smittenkitchen-com_2020-10-skillet-turkey-chili: missed [], extra ["dairy-free", "lunch", "make-ahead", "mexican", "soup"]
- smittenkitchen-com_2015-10-salted-peanut-butter-cookies: missed [], extra ["comfort food", "eggs", "make-ahead", "snack"]
- smittenkitchen-com_2018-03-sweet-potato-tacos: missed [], extra ["comfort food", "dairy-free", "lunch", "vegan"]
- smittenkitchen-com_2021-02-baked-feta-with-tomatoes-and-chickpeas: missed [], extra ["comfort food", "gluten-free", "lunch"]
- cooking-nytimes-com_recipes-1022068-skillet-chicken-with-mushrooms-and-caramelized-onions: missed ["dairy-free", "gluten-free"], extra ["comfort food"]

### google/gemini-3.5-flash-lite

- smittenkitchen-com_2025-01-potato-leek-soup: missed ["gluten-free"], extra ["appetizer", "lunch", "make-ahead", "side dish"]
- seriouseats-com_loubia-moroccan-white-bean-stew-recipe-11685571: missed ["dairy-free", "gluten-free", "make-ahead"], extra ["lunch", "soup"]
- smittenkitchen-com_2015-10-salted-peanut-butter-cookies: missed [], extra ["comfort food", "eggs", "make-ahead", "snack"]
- smittenkitchen-com_2024-11-halloumi-and-fall-vegetable-roast: missed ["gluten-free"], extra ["lunch", "one-pot", "side dish"]
- seriouseats-com_rice-krispie-treats-recipe-7107746: missed ["make-ahead"], extra ["baking", "comfort food", "vegetarian"]

### anthropic/claude-haiku-4.5

- smittenkitchen-com_2025-01-potato-leek-soup: missed ["gluten-free"], extra ["lunch", "make-ahead", "one-pot"]
- virtualweberbullet-com_brisket-high-heat: missed ["dairy-free", "gluten-free"], extra ["comfort food", "make-ahead"]
- smittenkitchen-com_2021-02-baked-feta-with-tomatoes-and-chickpeas: missed ["dinner", "one-pot", "weeknight"], extra ["baking"]
- smittenkitchen-com_2020-10-skillet-turkey-chili: missed ["gluten-free"], extra ["lunch", "mexican"]
- smittenkitchen-com_2015-10-salted-peanut-butter-cookies: missed ["dairy-free", "gluten-free"], extra ["make-ahead"]

### anthropic/claude-sonnet-5.5

- smittenkitchen-com_2025-01-potato-leek-soup: missed ["gluten-free"], extra ["lunch", "make-ahead", "one-pot", "weeknight"]
- smittenkitchen-com_2015-04-salted-chocolate-chunk-cookies: missed [], extra ["comfort food", "eggs", "make-ahead", "snack"]
- smittenkitchen-com_2015-10-salted-peanut-butter-cookies: missed [], extra ["eggs", "make-ahead", "snack"]
- smittenkitchen-com_2020-10-skillet-turkey-chili: missed [], extra ["make-ahead", "soup"]
- cooking-nytimes-com_recipes-1022024-crispy-gnocchi-with-burst-tomatoes-and-mozzarella: missed ["pasta"], extra ["comfort food"]

### deepseek/deepseek-v4.1-flash

- smittenkitchen-com_2021-02-baked-feta-with-tomatoes-and-chickpeas: missed [], extra ["baking", "comfort food", "gluten-free", "lunch", "side dish"]
- smittenkitchen-com_2020-10-skillet-turkey-chili: missed ["gluten-free"], extra ["lunch", "mexican", "soup"]
- smittenkitchen-com_2015-10-salted-peanut-butter-cookies: missed [], extra ["comfort food", "eggs", "make-ahead", "snack"]
- smittenkitchen-com_2024-11-halloumi-and-fall-vegetable-roast: missed [], extra ["comfort food", "holiday", "lunch", "side dish"]
- cooking-nytimes-com_recipes-1022068-skillet-chicken-with-mushrooms-and-caramelized-onions: missed ["dairy-free", "gluten-free"], extra ["comfort food", "lunch"]
