# AI eval: Custom enrich

Written by `make ai-eval`. A recipe changed by a user's instruction (`custom_enrich`), for 10 pipeline snapshots each paired with an instruction that suits it (`ENRICH_CASES`: make it vegetarian, halve it, convert to metric…). An answer that doesn't parse as the server's recipe shape is invalid, as in production. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/custom-enrich.json` (10 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 90% | 10% | 0 | 0% | 0 | 0 | $0.0457 |
| google/gemini-3.8-flash | 90% | 60% | 0 | 0% | 0 | 0 | $0.1066 |
| google/gemini-3.5-flash-lite | 90% | 10% | 0 | 0% | 0 | 0 | $0.0458 |
| anthropic/claude-haiku-4.5 | 78% | 11% | 0 | 10% | 1 | 0 | $0.0920 |
| anthropic/claude-sonnet-5.5 | 90% | 60% | 0 | 0% | 0 | 0 | $0.2797 |

## Rejected answers

The first few items each model answered invalidly even alone.

### anthropic/claude-haiku-4.5

- Failed to parse response: Unusable model response (finish_reason=Some(stop)): "```json\n{\n  \"title\": \"Baked Ziti With Sausage Meatballs and Spinach\",\n  \"description\": \"Baked ziti is meant to feed a crowd, and this one surely does. \"Cheater\" meatballs made with uncased Italian sau"

## Worst misses

### google/gemini-2.5-flash

- smittenkitchen-com_2016-10-pumpkin-bread: judged bad: "Pumpkin Bread\nServings: Servings: 8\nSource: Smittenkitchen.com\nSource URL: https://smittenkitchen.com/2016/10/pumpkin-br"

### google/gemini-3.8-flash

- americastestkitchen-com_recipes-5027-best-beef-stew: judged bad: "Best Vegetarian Stew\nDescription: Rich, deeply savory, and comforting, this hearty vegetarian stew uses browned mushroom"

### google/gemini-3.5-flash-lite

- cooking-nytimes-com_recipes-1020129-baked-ziti-with-sausage-meatballs-and-spinach: judged bad: "Baked Ziti With Sausage Meatballs and Spinach\nDescription: Baked ziti is meant to feed a crowd, and this one surely does"

### anthropic/claude-haiku-4.5

- americastestkitchen-com_recipes-5027-best-beef-stew: judged bad: "Best Vegetarian Stew\nDescription: Despite hours of simmering, most vegetable stews fall flat. With its rich gravy and te"
- americastestkitchen-com_recipes-6682-weeknight-roast-chicken: judged bad: "Weeknight Roast Chicken\nDescription: If you can plan ahead, by all means brine or salt your bird. But when you want dinn"

### anthropic/claude-sonnet-5.5

- smittenkitchen-com_2019-03-perfect-meatballs-and-spaghetti: judged bad: "Perfect Meatballs and Spaghetti (Gluten-Free)\nServings: Servings: 4 and up to 6 with sides\nSource: Smittenkitchen.com\nSo"
