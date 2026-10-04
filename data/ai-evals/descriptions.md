# AI eval: Descriptions

Written by `make ai-eval`. A menu-style description (`generate_description`) for 15 pipeline snapshots. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/descriptions.json` (15 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | – | – | 15 | 0% | 0 | 0 | $0.0040 |
| google/gemini-3.8-flash | – | – | 15 | 0% | 0 | 0 | $0.0956 |
| google/gemini-3.5-flash-lite | – | – | 15 | 0% | 0 | 0 | $0.0039 |
| anthropic/claude-haiku-4.5 | – | – | 15 | 0% | 0 | 0 | $0.0152 |
| anthropic/claude-sonnet-5.5 | – | – | 15 | 0% | 0 | 0 | $0.0653 |
| deepseek/deepseek-v4.1-flash | – | – | 15 | 0% | 0 | 0 | $0.0168 |

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
