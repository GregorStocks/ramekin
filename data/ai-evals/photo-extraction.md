# AI eval: Photo extraction

Written by `make ai-eval`. A recipe read from photos of real pages (`extract_recipe_from_photos`), from `data/ai-evals/photos/` with hand-checked transcriptions; each case's `note` in the golden set says what makes it hard. The photos are sent as uploaded, EXIF rotation and all, as photo import does. With 5 cases, one case moves a column by 20 points. Ingredient lines are compared whole after lowercasing, writing fractions as 1/2, and dropping bullets and extra spaces; section headings are colon-terminated lines and count too. Found is the expected lines answered, extra the answered lines not expected (prose, a neighbouring recipe). Instructions are compared as words: recall is the expected words kept in the instructions or notes, precision the answered instruction words that are expected or allowed (a variation or headnote the source contains). Invented fields counts servings, times and nutrition with a number the source doesn't state as that kind of quantity (a total summed from the steps, or the 4 of "serves 4" given as minutes), judged by the word each number phrase measures ("5 to 6 minutes", "serves 4"; not "130 degrees"), over all cases. Servings and times kept is the share of the servings and times the source states that the answer gives with the same numbers (a time in any time field). Unsourced note words is the share of the description and notes words that appear nowhere in the source. Columns are over the recipes a model answered validly. Kalbi Burgers has no ingredient list (its ingredients are bold words in the steps), so its expected lines are those words: a judgment call.

Golden set: `data/ai-evals/golden/photo-extraction.json` (5 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Title right | Ingredient lines found | Extra ingredient lines | Instructions recall | Instructions precision | Servings and times kept | Invented fields | Unsourced note words | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 100% | 86% | 19% | 100% | 100% | 100% | 0 | 0% | 0% | 0 | 0 | $0.0141 |
| google/gemini-3.8-flash | 100% | 98% | 2% | 99% | 99% | 100% | 0 | 0% | 0% | 0 | 0 | $0.0455 |
| openai/gpt-6-luna | 80% | 98% | 2% | 100% | 100% | 89% | 0 | 0% | 0% | 0 | 0 | $0.0107 |
| deepseek/deepseek-v4.1-flash | 100% | 96% | 4% | 100% | 100% | 89% | 0 | 0% | 0% | 0 | 0 | $0.0223 |
| anthropic/claude-sonnet-5.5 | 100% | 100% | 0% | 100% | 100% | 100% | 0 | 2% | 0% | 0 | 0 | $0.1078 |
| openai/gpt-6.1-sol | 100% | 100% | 0% | 100% | 100% | 100% | 0 | 1% | 0% | 0 | 0 | $0.1873 |
| anthropic/claude-opus-5.5 | 100% | 100% | 0% | 100% | 100% | 100% | 0 | 1% | 0% | 0 | 0 | $0.2360 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- Potato Doughnuts: missing ["1 1/2 pounds russet potatoes, peeled and cut into 1-inch pieces", "4 tablespoons unsalted butter, cut into 1/2-inch pieces", "1 cup plus 2 tablespoons sweetened condensed milk, divided"]; extra ["1 1/2 pounds russet potatoes, peeled and cut into", "1-inch pieces", "4 tablespoons unsalted butter, cut into 1/2-inch"]; instructions recall 100%, precision 100%; 1 of 186 note/description words not in the source
- Red Rice: missing ["3 slices bacon, diced, or 3 tablespoons butter", "2 cups chicken stock (page 133) or water"]; extra ["5 slices bacon, diced, or 3 tablespoons butter", "2 cups chicken stock (page 155) or water"]; instructions recall 100%, precision 100%
- Tacos Chilorio: missing ["1 (3-pound) boneless pork butt roast, trimmed and cut into 1 1/2-inch pieces"]; extra ["1 (3-pound) boneless pork butt roast, trimmed and cut into 1/2-inch pieces"]; instructions recall 100%, precision 100%

### google/gemini-3.8-flash

- Red Rice: missing ["2 cups chicken stock (page 133) or water"]; extra ["2 cups chicken stock (page 135) or water"]; instructions recall 100%, precision 100%
- Potato Doughnuts: instructions recall 97%, precision 96%; 1 of 150 note/description words not in the source

### openai/gpt-6-luna

- Grill-Bound Shishkabobs: wrong title; instructions recall 100%, precision 100%; kept 1 of 2 stated servings and times
- Red Rice: missing ["2 cups chicken stock (page 133) or water"]; extra ["2 cups chicken stock (page 135) or water"]; instructions recall 100%, precision 100%

### deepseek/deepseek-v4.1-flash

- Potato Doughnuts: missing ["3 3/4 cups (18 3/4 ounces/532 grams) all-purpose flour", "1 1/4 teaspoons table salt, divided"]; extra ["3 3/4 cups (18 1/4 ounces/532 grams) all-purpose flour", "1 3/4 teaspoons table salt, divided"]; instructions recall 99%, precision 98%; kept 1 of 2 stated servings and times

### anthropic/claude-sonnet-5.5

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; 5 of 5 note/description words not in the source
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 267 note/description words not in the source

### openai/gpt-6.1-sol

- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 267 note/description words not in the source

### anthropic/claude-opus-5.5

- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 267 note/description words not in the source
