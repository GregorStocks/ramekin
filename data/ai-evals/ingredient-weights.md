# AI eval: Ingredient weights

Written by `make ai-eval`. Grams in one unit of a catalog food (`estimate_ingredient_weights`), against USDA: 60 densities (cup) and 60 piece weights in estimable units. Error is |estimate − USDA| / USDA; null means the model said there's no typical weight.

Golden set: `data/ai-evals/golden/ingredient-weights.json` (120 cases), asked 40 per call (production asks 40). Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), or a provider error that outlasted its retries; a rejected batch is retried as the production worker does (in halves for weights, item by item for names), and a rejected single item is invalid. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Within 20% | Within 50% | Median error | P90 error | Null | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 59% | 84% | 10% | 56% | 0% | 0% | 0 | 0 | $0.0152 |
| google/gemini-3.8-flash | 68% | 90% | 5% | 48% | 1% | 0% | 0 | 0 | $0.0428 |
| google/gemini-3.1-pro-preview | 68% | 88% | 6% | 52% | 0% | 0% | 0 | 0 | $0.1517 |
| anthropic/claude-sonnet-5.5 | 73% | 92% | 4% | 43% | 0% | 0% | 0 | 0 | $0.0662 |
| anthropic/claude-opus-5.5 | 82% | 93% | 3% | 34% | 0% | 0% | 0 | 0 | $0.2136 |
| openai/gpt-6.1-sol | 72% | 93% | 3% | 39% | 0% | 0% | 0 | 0 | $0.0634 |
| openai/gpt-6-luna | 67% | 86% | 8% | 60% | 0% | 0% | 0 | 0 | $0.0034 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- candies, hershey's, almond joy bites | piece: 10 g (USDA 2.2222222222222223 g)
- pork, oriental style, dehydrated | cup: 50 g (USDA 22 g)
- danish pastry, fruit, unenriched (includes apple, cinnamon, raisin, strawberry) | piece: 100 g (USDA 53 g)
- soybeans, mature seeds, sprouted, cooked, steamed, with salt | cup: 170 g (USDA 94 g)
- mustard spinach, (tendergreen), raw | cup: 30 g (USDA 150 g)

### google/gemini-3.8-flash

- pork, oriental style, dehydrated | cup: 90 g (USDA 22 g)
- candies, hershey's, almond joy bites | piece: 6 g (USDA 2.2222222222222223 g)
- game meat, elk, loin, separable lean only, cooked, broiled | steak: 218 g (USDA 114 g)
- pork, fresh, loin, center rib (chops), boneless, separable lean only, cooked, broiled | chop: 128 g (USDA 71 g)
- restaurant, latino, arroz con grandules (rice and pigeonpeas) | cup: 200 g (USDA 115 g)

### google/gemini-3.1-pro-preview

- pork, oriental style, dehydrated | cup: 100 g (USDA 22 g)
- candies, hershey's, almond joy bites | piece: 8 g (USDA 2.2222222222222223 g)
- beef, flank, steak, separable lean and fat, trimmed to 0" fat, choice, cooked, broiled | steak: 700 g (USDA 387 g)
- restaurant, latino, arroz con grandules (rice and pigeonpeas) | cup: 200 g (USDA 115 g)
- duck, young duckling, domesticated, white pekin, leg, meat and skin, bone in, cooked, roasted | leg: 160 g (USDA 92 g)

### anthropic/claude-sonnet-5.5

- candies, hershey's, almond joy bites | piece: 10 g (USDA 2.2222222222222223 g)
- pork, oriental style, dehydrated | cup: 85 g (USDA 22 g)
- duck, young duckling, domesticated, white pekin, leg, meat and skin, bone in, cooked, roasted | leg: 220 g (USDA 92 g)
- danish pastry, fruit, unenriched (includes apple, cinnamon, raisin, strawberry) | piece: 100 g (USDA 53 g)
- fish, pompano, florida, cooked, dry heat | fillet: 150 g (USDA 88 g)

### anthropic/claude-opus-5.5

- pork, oriental style, dehydrated | cup: 80 g (USDA 22 g)
- pork, fresh, loin, center rib (chops), boneless, separable lean only, cooked, broiled | chop: 130 g (USDA 71 g)
- coriander (cilantro) leaves, raw | sprig: 0.4 g (USDA 2.2222222222222223 g)
- restaurant, latino, arroz con grandules (rice and pigeonpeas) | cup: 200 g (USDA 115 g)
- duck, young duckling, domesticated, white pekin, leg, meat and skin, bone in, cooked, roasted | leg: 160 g (USDA 92 g)

### openai/gpt-6.1-sol

- pork, oriental style, dehydrated | cup: 70 g (USDA 22 g)
- candies, hershey's, almond joy bites | piece: 7 g (USDA 2.2222222222222223 g)
- restaurant, latino, arroz con grandules (rice and pigeonpeas) | cup: 200 g (USDA 115 g)
- beef, chuck eye country-style ribs, boneless, separable lean and fat, trimmed to 0" fat, choice, cooked, braised | piece: 85 g (USDA 224 g)
- pork, fresh, loin, center rib (chops), boneless, separable lean only, cooked, broiled | chop: 115 g (USDA 71 g)

### openai/gpt-6-luna

- candies, hershey's, almond joy bites | piece: 12 g (USDA 2.2222222222222223 g)
- duck, young duckling, domesticated, white pekin, leg, meat and skin, bone in, cooked, roasted | leg: 250 g (USDA 92 g)
- pork, oriental style, dehydrated | cup: 50 g (USDA 22 g)
- restaurant, latino, arroz con grandules (rice and pigeonpeas) | cup: 230 g (USDA 115 g)
- soybeans, mature seeds, sprouted, cooked, steamed, with salt | cup: 180 g (USDA 94 g)
