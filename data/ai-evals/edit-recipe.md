# AI eval: Edit with AI

Written by `make ai-eval`. A recipe changed by a user's instruction (`edit_recipe`), for 13 pipeline snapshots each paired with an instruction that suits it (`EDIT_CASES`: make it vegetarian, halve it, convert to metric…; three need its tools: catalog weights, calories, scaling). The model edits with tools in a bounded loop (`MAX_EDIT_TURNS`), seeing no user tags or other recipes; answers end with its summary. An answer that doesn't parse as the server's recipe shape is invalid, as in production. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/edit-recipe.json` (13 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries, and are invalid: production doesn't retry them. Cost is what the accepted calls cost: what OpenRouter reported (prompt-cache discounts included) or, for answers cached before it reported costs, their tokens at its current prices; rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-3.8-flash | 82% | 0% | 0 | 15% | 2 | 0 | $0.1436 |
| anthropic/claude-sonnet-5.5 | 100% | 8% | 0 | 0% | 0 | 0 | $0.4969 |
| anthropic/claude-opus-5.5 | 100% | 100% | 0 | 0% | 0 | 0 | $1.0623 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-3.8-flash

- API error: The edit didn't finish within 20 turns
- API error: The edit didn't finish within 20 turns

## Worst misses

### google/gemini-3.8-flash

- smittenkitchen-com_2015-04-salted-chocolate-chunk-cookies: judged bad: "Salted Chocolate Chunk Cookies\nServings: yield: approximately 9 to 12 cookies\nSource: Smittenkitchen.com\nSource URL: htt"
- smittenkitchen-com_2008-11-cottage-cheese-pancakes: judged bad: "Cottage Cheese Pancakes\nServings: 3 servings (about 9 3-inch pancakes)\nSource: Smittenkitchen.com\nSource URL: https://sm"

### anthropic/claude-sonnet-5.5

None.

### anthropic/claude-opus-5.5

None.
