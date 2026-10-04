# AI eval: Recipe photos

Written by `make ai-eval`. A generated recipe photo (`generate_recipe_photo`; the models are image models) for 8 pipeline snapshots. Photos are cached under the AI cache directory by model and prompt, since the provider doesn't cache them. Cost is what OpenRouter reported for each photo. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/recipe-photos.json` (8 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash-image | – | – | 8 | 0% | 0 | 0 | $0.3113 |
| google/gemini-3.1-flash-image | – | – | 8 | 0% | 0 | 0 | $0.5405 |
| google/gemini-3.1-flash-lite-image | – | – | 8 | 0% | 0 | 0 | $0.2703 |
| google/gemini-3-pro-image | – | – | 8 | 0% | 0 | 0 | $1.1226 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash-image

None.

### google/gemini-3.1-flash-image

None.

### google/gemini-3.1-flash-lite-image

None.

### google/gemini-3-pro-image

None.
