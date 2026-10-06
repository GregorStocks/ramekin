# AI eval: Descriptions

Written by `make ai-eval`. A menu-style description (`generate_description`) for 15 pipeline snapshots. Judged by a person on a blind page (`logs/ai-evals/<suite>/judge.html`, written by each run): model names hidden, answers shuffled, identical answers merged. Unjudged counts the valid answers no judgment covers yet; judge them and rerun. Each distinct answer is judged best, good or bad (`data/ai-evals/judgments/<suite>.json`, keyed by a hash of the answer, so a model giving an answer already judged reuses its judgment). Good counts best and good answers, over the model's judged answers; best is the share judged among the best for their case.

Golden set: `data/ai-evals/golden/descriptions.json` (15 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries, and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Good | Best | Unjudged | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 87% | 33% | 0 | 0% | 0 | 0 | $0.0040 |
| google/gemini-3.8-flash | 80% | 33% | 0 | 0% | 0 | 0 | $0.0956 |
| google/gemini-3.5-flash-lite | 73% | 27% | 0 | 0% | 0 | 0 | $0.0039 |
| anthropic/claude-haiku-4.5 | 80% | 7% | 0 | 0% | 0 | 0 | $0.0152 |
| anthropic/claude-sonnet-5.5 | 80% | 13% | 0 | 0% | 0 | 0 | $0.0653 |
| deepseek/deepseek-v4.1-flash | 67% | 20% | 0 | 0% | 0 | 0 | $0.0168 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- smittenkitchen-com_2011-01-baked-potato-soup: judged bad: "Creamy potato soup with roasted garlic, leeks, and a hint of bay leaf."
- cocktailswithsuderman-substack-com_p-hot-chocolate-manhattan: judged bad: "Warm chocolate, whiskey, and sweet vermouth with aromatic bitters."

### google/gemini-3.8-flash

- americastestkitchen-com_recipes-11955-spinach-artichoke-dip: judged bad: "Baked until bubbling with melted gouda, parmesan, and garlic."
- cocktailswithsuderman-substack-com_p-hot-chocolate-manhattan: judged bad: "Warm chocolate laced with bourbon, sweet vermouth, and bitters."
- smittenkitchen-com_2025-05-one-pan-ditalini-and-peas: judged bad: "Ditalini tossed with sweet peas, parmesan cream, and lemon."

### google/gemini-3.5-flash-lite

- smittenkitchen-com_2022-04-simplest-mushroom-pasta: judged bad: "Pan-roasted cremini mushrooms and garlic in a rich marsala cream sauce."
- smittenkitchen-com_2024-12-invisible-apple-cake: judged bad: "Paper-thin apple slices bound in a delicate vanilla custard."
- cocktailswithsuderman-substack-com_p-hot-chocolate-manhattan: judged bad: "Bourbon and dark chocolate warmed with sweet vermouth and bitters."
- smittenkitchen-com_2025-05-one-pan-ditalini-and-peas: judged bad: "Ditalini simmered with sweet peas, cream, and parmesan."

### anthropic/claude-haiku-4.5

- americastestkitchen-com_recipes-11955-spinach-artichoke-dip: judged bad: "Creamy spinach and artichoke dip with Gouda, melted and golden."
- smittenkitchen-com_2011-01-baked-potato-soup: judged bad: "Roasted garlic and leek soup with creamy potato base."
- smittenkitchen-com_2025-05-one-pan-ditalini-and-peas: judged bad: "Creamy ditalini with peas, crispy salami, and lemon zest."

### anthropic/claude-sonnet-5.5

- seriouseats-com_better-no-knead-bread-recipe: judged bad: "Crackly crust and chewy crumb from a long, cold ferment."
- smittenkitchen-com_2007-01-really-simple-homemade-pizza: judged bad: "Chewy, blistered crust from a one-bowl dough with olive oil."
- cooking-nytimes-com_recipes-1019683-mozzarella-in-carrozza-fried-mozzarella-sandwiches: judged bad: "Panko-crusted white bread sandwiches with melted mozzarella."

### deepseek/deepseek-v4.1-flash

- seriouseats-com_better-no-knead-bread-recipe: judged bad: "Crackly crust, open crumb from a long, slow ferment."
- smittenkitchen-com_2019-07-crispy-oven-pulled-pork: judged bad: "Crisp-edged pork shoulder, smoky paprika, tangy cider vinegar."
- smittenkitchen-com_2019-05-braised-ginger-meatballs-in-coconut-broth: judged bad: "Pork meatballs in coconut broth with ginger, chiles, lime."
- smittenkitchen-com_2022-04-simplest-mushroom-pasta: judged bad: "Browned cremini mushrooms tossed with pasta, garlic, crème fraîche."
- cooking-nytimes-com_recipes-1019683-mozzarella-in-carrozza-fried-mozzarella-sandwiches: judged bad: "Crisp panko-crusted mozzarella, garlicky egg, golden fried."
