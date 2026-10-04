# AI eval: Tags

Written by `make ai-eval`. Tags picked from a user's tags for a recipe (`suggest_tags`), for 20 pipeline snapshots and a fixed vocabulary of 33 tags (`TAG_VOCABULARY`), against the tags a person ticked as applying (`data/ai-evals/judgments/tags.json`). Exact is the recipes answered with exactly the judged tags; precision and recall count tags over all judged recipes. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun.

Golden set: `data/ai-evals/golden/tags.json` (20 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Exact | Precision | Recall | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | – | – | – | 20 | 0% | 0 | 0 | $0.0061 |
| google/gemini-3.8-flash | – | – | – | 20 | 0% | 0 | 0 | $0.0553 |
| google/gemini-3.5-flash-lite | – | – | – | 20 | 0% | 0 | 0 | $0.0062 |
| anthropic/claude-haiku-4.5 | – | – | – | 20 | 0% | 0 | 0 | $0.0223 |
| anthropic/claude-sonnet-5.5 | – | – | – | 20 | 0% | 0 | 0 | $0.0686 |
| deepseek/deepseek-v4.1-flash | – | – | – | 20 | 0% | 0 | 0 | $0.0646 |

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

None.

### deepseek/deepseek-v4.1-flash

None.
