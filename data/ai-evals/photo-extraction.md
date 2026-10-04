# AI eval: Photo extraction

Written by `make ai-eval`. A recipe read from photos of real pages (`extract_recipe_from_photos`), from `data/ai-evals/photos/` with hand-checked transcriptions; each case's `note` in the golden set says what makes it hard. The photos are sent as uploaded, EXIF rotation and all, as photo import does. With 5 cases, one case moves a column by 20 points. Ingredient lines are compared whole after lowercasing, writing fractions as 1/2, and dropping bullets and extra spaces; section headings are colon-terminated lines and count too. Found is the expected lines answered, extra the answered lines not expected (prose, a neighbouring recipe). Instructions are compared as words: recall is the expected words kept in the instructions (or notes, for a moved tip, once the instructions hold at least half), precision the answered instruction words that are expected or allowed (a variation or headnote the source contains). Other text kept is the share of the page's other text (a description, headnote or variation; not a nutrition panel, which photo import has no field for) found in the answer's description, notes or instructions. Invented fields counts difficulty, source, categories (the draft's tags) and rating with words the source never uses, rating unless the source rates it, and servings, times and nutrition with such words or a number the source doesn't state as that kind of quantity (a total summed from the steps, or the 4 of "serves 4" given as minutes), judged by the word each number phrase measures ("5 to 6 minutes", "serves 4"; not "130 degrees"). A time counts as stated only where the source labels one ("Prep time:", "Total time", "Ready in"), so a step's "chill 30 minutes" given as the total time is invented. Each stated quantity backs one answered value. Counted over all cases. Servings and times kept is the share of the servings and times the source states that the answer gives with the same numbers (a time in any time field). Unsourced note words is the share of the description and notes words that appear nowhere in the source. Columns are over the recipes a model answered validly. Kalbi Burgers has no ingredient list (its ingredients are bold words in the steps), so its expected lines are those words: a judgment call.

Golden set: `data/ai-evals/golden/photo-extraction.json` (5 cases), one call per case, as in production. Rejected calls got an answer that failed validation or ran out of the production max_tokens (counted again as truncated), and are invalid: production doesn't retry them. Cost is what the accepted calls cost at OpenRouter's current prices: rejected calls were billed too but carry no usage, so the cost understates models with many of them. A cached rerun spends nothing.

| Model | Title right | Ingredient lines found | Extra ingredient lines | Instructions recall | Instructions precision | Other text kept | Servings and times kept | Invented fields | Unsourced note words | Invalid | Rejected calls | Truncated calls | Cost of accepted calls |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| google/gemini-2.5-flash | 100% | 86% | 19% | 100% | 100% | 84% | 100% | 2 | 0% | 0% | 0 | 0 | $0.0141 |
| google/gemini-3.8-flash | 100% | 98% | 2% | 99% | 99% | 83% | 100% | 1 | 0% | 0% | 0 | 0 | $0.0455 |
| openai/gpt-6-luna | 80% | 98% | 2% | 100% | 100% | 89% | 100% | 0 | 0% | 0% | 0 | 0 | $0.0107 |
| deepseek/deepseek-v4.1-flash | 100% | 96% | 4% | 100% | 99% | 94% | 88% | 1 | 0% | 0% | 0 | 0 | $0.0223 |
| anthropic/claude-sonnet-5.5 | 100% | 100% | 0% | 100% | 100% | 85% | 100% | 1 | 2% | 0% | 0 | 0 | $0.1078 |
| openai/gpt-6.1-sol | 100% | 100% | 0% | 100% | 100% | 85% | 100% | 1 | 1% | 0% | 0 | 0 | $0.1873 |
| anthropic/claude-opus-5.5 | 100% | 100% | 0% | 100% | 100% | 100% | 100% | 1 | 1% | 0% | 0 | 0 | $0.2360 |

## Rejected answers

The first few items each model answered invalidly even alone.

## Worst misses

### google/gemini-2.5-flash

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 25% of the other text
- Potato Doughnuts: missing ["1 1/2 pounds russet potatoes, peeled and cut into 1-inch pieces", "4 tablespoons unsalted butter, cut into 1/2-inch pieces", "1 cup plus 2 tablespoons sweetened condensed milk, divided"]; extra ["1 1/2 pounds russet potatoes, peeled and cut into", "1-inch pieces", "4 tablespoons unsalted butter, cut into 1/2-inch"]; instructions recall 100%, precision 100%; kept 99% of the other text; 1 of 188 note/description words not in the source
- Red Rice: missing ["3 slices bacon, diced, or 3 tablespoons butter", "2 cups chicken stock (page 133) or water"]; extra ["5 slices bacon, diced, or 3 tablespoons butter", "2 cups chicken stock (page 155) or water"]; instructions recall 100%, precision 100%; invented cook_time; kept 97% of the other text
- Tacos Chilorio: missing ["1 (3-pound) boneless pork butt roast, trimmed and cut into 1 1/2-inch pieces"]; extra ["1 (3-pound) boneless pork butt roast, trimmed and cut into 1/2-inch pieces"]; instructions recall 100%, precision 100%

### google/gemini-3.8-flash

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 25% of the other text
- Potato Doughnuts: instructions recall 97%, precision 96%; kept 91% of the other text; 1 of 152 note/description words not in the source
- Red Rice: missing ["2 cups chicken stock (page 133) or water"]; extra ["2 cups chicken stock (page 135) or water"]; instructions recall 100%, precision 100%

### openai/gpt-6-luna

- Grill-Bound Shishkabobs: wrong title; instructions recall 100%, precision 100%; kept 50% of the other text
- Red Rice: missing ["2 cups chicken stock (page 133) or water"]; extra ["2 cups chicken stock (page 135) or water"]; instructions recall 100%, precision 100%
- Potato Doughnuts: instructions recall 100%, precision 100%; kept 93% of the other text

### deepseek/deepseek-v4.1-flash

- Potato Doughnuts: missing ["3 3/4 cups (18 3/4 ounces/532 grams) all-purpose flour", "1 1/4 teaspoons table salt, divided"]; extra ["3 3/4 cups (18 1/4 ounces/532 grams) all-purpose flour", "1 3/4 teaspoons table salt, divided"]; instructions recall 99%, precision 94%; kept 76% of the other text; kept 1 of 2 stated servings and times
- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time
- Red Rice: instructions recall 100%, precision 100%; kept 97% of the other text

### anthropic/claude-sonnet-5.5

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 25% of the other text; 5 of 5 note/description words not in the source
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 278 note/description words not in the source

### openai/gpt-6.1-sol

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time; kept 25% of the other text
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 278 note/description words not in the source

### anthropic/claude-opus-5.5

- Grill-Bound Shishkabobs: instructions recall 100%, precision 100%; invented cook_time
- Potato Doughnuts: instructions recall 100%, precision 100%; 4 of 278 note/description words not in the source
