# AI eval: Recipe photos

Written by `make ai-eval`. A generated recipe photo (`generate_recipe_photo`; the models are image models, `model@high` asking for that quality) for 10 hand-picked pipeline snapshots (`PHOTO_CASES`): dishes whose real look is crisp, structured or colorful, so a model that renders food as brown mush is caught. Photos are cached under the AI cache directory by model and prompt, since the provider doesn't cache them. Cost is what OpenRouter reported for each photo. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/recipe-photos.json` (10 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-3.1-flash-image | 90% | 40% | 0 | 0% | 0 | 0 | $0.7432 |
| openai/gpt-image-2.5-sunburst@high | 90% | 20% | 0 | 0% | 0 | 0 | $0.4504 |
| openai/gpt-image-2.5-flare@high | 90% | 10% | 0 | 0% | 0 | 0 | $0.4504 |
| openai/gpt-image-2.5-flare | 80% | 10% | 0 | 0% | 0 | 0 | $0.0918 |
| meta/muse-image | 80% | 30% | 0 | 0% | 0 | 0 | $0.1000 |
| microsoft/mai-image-2.6 | 88% | 25% | 0 | 20% | 2 | 0 | $0.3413 |

## Rejected answers

The first few items each model answered invalidly even alone.

### microsoft/mai-image-2.6

- Refused: Image generation request failed with 400 Bad Request: {"error":{"message":"Response content blocked by label 'DallECandidateBlockList'.","code":400,"metadata":{"provider_name":"Azure"}}}
- Refused: Image generation request failed with 400 Bad Request: {"error":{"message":"Response content blocked by label 'DallECandidateBlockList'.","code":400,"metadata":{"provider_name":"Azure"}}}

## Worst misses

### google/gemini-3.1-flash-image

- smittenkitchen-com_2013-02-italian-stuffed-cabbage: judged bad: answer 1e0e3bca40bc5f05

### openai/gpt-image-2.5-sunburst@high

- smittenkitchen-com_2016-05-chicken-gyro-salad: judged bad: answer d29a9a6a8764ada3

### openai/gpt-image-2.5-flare@high

- smittenkitchen-com_2015-08-takeout-style-sesame-noodles-with-cucumber: judged bad: answer cbb55c8af1988cba

### openai/gpt-image-2.5-flare

- smittenkitchen-com_2023-04-hash-brown-patties: judged bad: answer 5cbbcca9dfd7bda1
- smittenkitchen-com_2016-05-chicken-gyro-salad: judged bad: answer bb0ba93b45dd7570

### meta/muse-image

- smittenkitchen-com_2013-02-italian-stuffed-cabbage: judged bad: answer 92870f016da9b262
- cooking-nytimes-com_recipes-1019430-omurice-japanese-rice-omelet: judged bad: answer 35d52b595c0f8a3c

### microsoft/mai-image-2.6

- smittenkitchen-com_2016-05-chicken-gyro-salad: judged bad: answer 6aa3e88b1a759345
