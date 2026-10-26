# AI eval: House style

Written by `make ai-eval`. A recipe rewritten into the house style (`house_style`, `doc/house-style.md`), for 10 pipeline snapshots: 4 with prep in their ingredient lines and sections, 4 with prep and no sections, and 2 with neither, which should change little. The model rewrites with the edit tools in a bounded loop (`MAX_EDIT_TURNS`), limited to ingredient and step edits; `finish` is refused until every original amount and catalog food is back, so an answer that never gets there (or doesn't parse as the server's recipe shape) is invalid, as in production. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/house-style.json` (10 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries, and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| anthropic/claude-opus-5.5 | 100% | 80% | 0 | 0% | 0 | 0 | $1.6922 |
| anthropic/claude-sonnet-5.5 | 100% | 10% | 0 | 0% | 0 | 0 | $0.8216 |
| google/gemini-3.8-flash | 86% | 14% | 0 | 30% | 3 | 0 | $0.4433 |

## Rejected answers

The first few items each model answered invalidly even alone.

### google/gemini-3.8-flash

- API error: The edit didn't finish within 20 turns
- API error: The edit didn't finish within 20 turns
- API error: The edit didn't finish within 20 turns

## Worst misses

### anthropic/claude-opus-5.5

None.

### anthropic/claude-sonnet-5.5

None.

### google/gemini-3.8-flash

- norecipes-com_sparerib-oden-recipe-japanese-comfort-food: judged bad: "Sparerib Oden\nDescription: Oden(おでん) is the quintessential Japanese comfort food and is super simple to make. With meat,"
