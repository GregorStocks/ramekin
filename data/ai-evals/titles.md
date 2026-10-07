# AI eval: Titles

Written by `make ai-eval`. A tidied recipe title (`normalize_title`) for 20 pipeline snapshots: 14 with decoration to remove (praise, "recipe", a parenthetical, a subtitle) and 6 already plain, which should come back unchanged. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/titles.json` (20 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries, and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 100% | 95% | 0 | 0% | 0 | 0 | $0.0085 |
| google/gemini-3.8-flash | 100% | 85% | 0 | 0% | 0 | 0 | $0.0529 |
| google/gemini-3.5-flash-lite | 100% | 100% | 0 | 0% | 0 | 0 | $0.0085 |
| anthropic/claude-haiku-4.5 | 100% | 100% | 0 | 0% | 0 | 0 | $0.0346 |
| anthropic/claude-sonnet-5.5 | 95% | 90% | 0 | 0% | 0 | 0 | $0.0966 |
| deepseek/deepseek-v4.1-flash | 100% | 90% | 0 | 0% | 0 | 0 | $0.0090 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

None.

### google/gemini-3.8-flash

None.

### google/gemini-3.5-flash-lite

None.

### anthropic/claude-haiku-4.5

None.

### anthropic/claude-sonnet-5.5

- seriouseats-com_sous-vide-carnitas-crispy-mexican-style-pulled-pork-recipe: judged bad: "Sous Vide Carnitas (Crispy Mexican-Style Pulled Pork) for Tacos"

### deepseek/deepseek-v4.1-flash

None.
