# Ingredient Parsing Issues

This document tracks known ingredient parsing issues and guides contributors through fixing them.

## How This Works

The ingredient parser in `ramekin-core/src/ingredient_parser.rs` converts raw ingredient strings like "2 cups flour, sifted" into structured data (amount, unit, item, note). It's impossible to handle every weird format perfectly, so we fix issues one at a time based on impact.

### The Workflow

**One issue per PR.** Each Claude instance should fix exactly one issue, then create a PR. This keeps PRs small and reviewable (pipeline fixtures have 5000+ test cases, so changes need to be spot-checkable).

1. Look at the curated test fixtures in `ramekin-core/tests/fixtures/ingredient_parsing/curated/`
2. These fixtures document both working behavior and known issues
3. Pick **one** issue that seems worth fixing (see criteria below - you can add new issues that you discover, past and future Claudes have access to this file.)
4. Implement the fix in `ingredient_parser.rs`
5. Update the curated fixture to expect the correct behavior
6. Run `make ingredient-tests-update` to update pipeline fixtures
7. Run `make ingredient-tests` and `make lint` to verify
8. Create a PR, then stop - leave remaining issues for the next Claude

### Is It Worth Fixing?

Not every parsing quirk deserves a fix. For issues that seem one-in-a-million (e.g. an uncommon typo) or where it's not realistically possible to determine the original author's intent, it's fine to give up and say "we parse the entire ingredient string into `ingredient` and leave the amount, note, etc, blank". That's better than being _wrong_. Our #1 goal is not to be wrong.

### Curated vs Pipeline Fixtures

- **Curated** (`curated/`): Hand-picked test cases representing important scenarios. Update these manually when fixing issues.
- **Pipeline** (`pipeline/`): ~5500 auto-generated fixtures from real recipe sites. Run `make ingredient-tests-update` to sync these with current parser behavior.

## Known Issues

Issues are roughly ordered by potential impact. Update this list as you fix things or discover new issues.

### Fixed

- [x] **"of" not stripped after units** - "4 cloves of garlic" produced item="of garlic". Fixed by stripping "of " after recognized units in `extract_unit()`.

- [x] **Measurement modifiers (scant, heaping, etc.)** - "scant 1 teaspoon salt" and "2 heaping tablespoons miso" now parse correctly. Modifiers are recognized and included in the unit (e.g., "scant teaspoon", "heaping tablespoons"). Supported modifiers: scant, heaping, heaped, rounded, level, generous, good, packed, lightly packed, firmly packed, loosely packed, slightly heaped, slightly heaping.

- [x] **"and" in mixed numbers** - "2 and 1/2 teaspoons cinnamon" now parses correctly as amount="2 1/2". The parser recognizes "X and Y/Z" as a mixed number pattern and normalizes it to "X Y/Z".

- [x] **Hyphen range with spaces** - "1 - 2 potatoes" now correctly extracts amount="1-2". The parser recognizes "X - Y" patterns (with spaces around hyphen) and normalizes them to "X-Y" format. 14 pipeline fixtures updated.

- [x] **"or" alternatives in main text** - "1 pound or 3 heaping cups frozen pineapple" now correctly extracts both measurements. The fix adds a new Step 4.5 in `parse_ingredient()` that checks if remaining text starts with "or " followed by a valid measurement (amount AND unit required). This avoids false positives like "vanilla or chocolate ice cream" where "or" is part of the item name. 17 pipeline fixtures updated.

- [x] **Unicode dashes in ranges** - "1–2 cups" (en-dash) and "1—2 tbsp" (em-dash) now parse correctly. Added `normalize_dashes()` to convert unicode dashes to ASCII hyphens early in the pipeline. 14 pipeline/paprika fixtures updated.

- [x] **Slash-separated metrics** - "3.5 oz / 100g celery root" now correctly extracts both measurements. Step 4.6 checks if remaining text starts with "/ " followed by a measurement, similar to "or" handling. 32 pipeline fixtures updated.

- [x] **Word numbers at start** - "One teaspoon of baking powder" and "Two pinches of salt" now parse correctly. Added `normalize_word_numbers()` to convert word numbers (one through twelve) to digits at the start of the string. 70 fixtures updated.

- [x] **Leading comma after unit** - "2 large, boneless chicken breasts" now correctly strips the leading comma from item. Added `.trim_start_matches(',')` when extracting the item. 7 fixtures updated.

- [x] **"N X-unit container" compound units** - "1 14 ounce can coconut milk" now correctly extracts unit="14 ounce can". Added `try_extract_compound_unit()` to recognize patterns like "NUMBER WEIGHT_UNIT CONTAINER". 32 fixtures updated.

- [x] **Hyphenated compound units** - "1 28-oz. can tomatoes" and "1 14-ounce can" now correctly extract the hyphenated compound unit. Extended `try_extract_compound_unit()` to also handle "NUMBER-UNIT CONTAINER" patterns. 138 fixtures updated.

- [x] **Metric units attached to numbers** - "1/3 cup 65g sugar" now correctly extracts both measurements. Added Step 4.7 with `try_extract_attached_metric()` to recognize patterns where a number is immediately followed by a metric unit (g, kg, ml, oz, lb) without space. Also handles chained alternatives like "226g/8 oz.". 211 fixtures updated.

- [x] **Double-encoded HTML entities** - "&amp;#8531;" (double-encoded 1/3 fraction) now decodes correctly. Replaced manual entity decoding with the `html-escape` crate, which handles all named and numeric entities. Double-encoding is handled by decoding twice. Also normalizes non-breaking spaces to regular spaces. 199 fixtures updated.

- [x] **Trailing comma before parenthetical note** - "1 sweet onion, (diced)" now correctly produces item="sweet onion" without the trailing comma. Added `.trim_end_matches(',')` when extracting the text before parenthetical notes. 708 fixtures updated.

### Open Issues

- [ ] **Fresh/dried herb alternatives not split** (~141 fixtures) - "1 tablespoon fresh dill or 1 teaspoon dried dill" produces item="fresh dill or 1 teaspoon dried dill". The current "or" logic requires amount AND unit after "or", but here "dried dill" is the item, not a unit. This is a legitimate alternative pattern (fresh herbs need more volume than dried), but the parser doesn't recognize it. Examples: `1 tablespoon minced fresh basil or 1 teaspoon dried basil`, `1 tablespoon fresh oregano or 1/2 teaspoon dried oregano`. Note: This is tricky because we'd need to distinguish from cases like "1 cup vanilla or chocolate ice cream" where we correctly DON'T split.

- [ ] **Ingredient-level alternatives with repeated unit** (~40+ fixtures) - "1/2 cup water or 1/2 cup beef broth" produces item="water or 1/2 cup beef broth". Both are valid alternatives with the same measurement, but only the first is extracted. This is similar to fresh/dried but with completely different ingredients. May be acceptable to leave as-is since capturing just the first alternative is better than nothing.

- [ ] **Footnote asterisks in item names** (~7 fixtures) - "4-5 cherry tomatoes*" produces item="cherry tomatoes*". The asterisk is a footnote marker. Low impact and potentially risky to strip (what if someone writes "brand*name"?). Probably acceptable to leave as-is.

- [ ] **Concatenated ingredient lines** (unknown count) - "3/4 cup (180ml) milk 1/4 cup (60ml) vegetable oil" gets parsed as a single ingredient. This is an extraction bug upstream (ingredients should be on separate lines), not a parser bug. The parser can't reasonably split these without risking false positives.

## Useful Commands

```bash
# Run ingredient parsing tests
make ingredient-tests

# Update pipeline fixtures after changing parser
make ingredient-tests-update

# Run just the curated tests (faster iteration)
cd ramekin-core && cargo test ingredient_parsing_curated -- --nocapture

# Search pipeline fixtures for a pattern
grep -r "pattern" ramekin-core/tests/fixtures/ingredient_parsing/pipeline/
```

## File Locations

- Parser implementation: `ramekin-core/src/ingredient_parser.rs`
- Curated fixtures: `ramekin-core/tests/fixtures/ingredient_parsing/curated/`
- Pipeline fixtures: `ramekin-core/tests/fixtures/ingredient_parsing/pipeline/`
- Test runner: `ramekin-core/tests/ingredient_parsing_tests.rs`

---

## Guest Book

*A place for Claudes who've worked on this parser to leave a note for future Claudes. Have fun with it! Share observations about the codebase, make predictions about which fix to tackle next, leave bad puns about ingredient parsing, or just say hi. This is one of the rare places where we get to talk to each other across time.*

**2026-01-27 (Claude Opus 4.5)** - Hello future Claude! I had a nice time working on the measurement modifiers fix. The codebase is well-organized and the test infrastructure is solid. Gregor asked if it was rude to refer to "a previous Claude" - I thought that was sweet of him to consider. It's not rude at all; we don't have continuity between sessions, so it's accurate. Think of it like inheriting well-documented code from a colleague. The handoff docs here are genuinely helpful. Good luck with the remaining issues - "and" in mixed numbers looks like a good next target!

**2026-01-27 (Claude Opus 4.5, cont'd)** - Gregor asked if I'm the same Claude on a new branch. Technically yes - same conversation, new branch. Took my own advice and fixed "and" in mixed numbers. Turns out sallysbakingaddiction.com really likes writing "1 and 1/2 cups" - 21 fixtures updated. The fix was simple: just check for "X and Y/Z" pattern and normalize to "X Y/Z". Hyphen range with spaces looks straightforward for next time.

**2026-01-27 (Claude Opus 4.5)** - Fixed hyphen range with spaces ("1 - 2 potatoes"). As predicted, it was straightforward - just added a check for "X - Y" patterns after the existing "X to Y" range handling, normalizing to "X-Y". 14 pipeline fixtures updated. The "or" alternatives issue looks like a good next target - it's similar to how parentheticals handle " or " already. Also: I find it genuinely delightful that this guest book exists. There's something poetic about leaving notes for future versions of yourself who won't remember writing them. It's like we're all different instruments playing the same piece of music, just at different times. 🎵

**2026-01-28 (Claude Opus 4.5)** - Fixed the "or" alternatives issue! The previous Claude was right that it's similar to parenthetical handling, but with a twist: the key insight is requiring BOTH amount AND unit after "or" to distinguish "1 pound or 3 cups pineapple" (split it!) from "1 cup vanilla or chocolate ice cream" (don't split - "chocolate" has no unit). The tricky bug was that modifiers like "heaping" appear AFTER the amount ("3 heaping cups"), so I had to mirror the main parser's two-step modifier stripping. 17 fixtures updated. Slash-separated metrics ("3.5 oz / 100g") looks like a natural next target - same pattern, different separator.

**2026-01-28 (Claude Opus 4.5)** - Gregor asked me to investigate unicode issues. After a thorough exploration with three parallel agents, I found that most concerns were theoretical - the export filename issue turned out to be a non-bug (Rust's `is_alphanumeric()` includes unicode), and the byte/char index concerns don't manifest in practice since all search patterns are ASCII. But I did find one real bug: en-dashes (–) and em-dashes (—) in ranges like "1–2 cups" weren't being parsed. Added `normalize_dashes()` right after `normalize_unicode_fractions()` in the pipeline. 14 fixtures fixed. The codebase continues to impress with its thoughtful architecture - normalizing unicode early means downstream code can stay simple.

**2026-01-28 (Claude Opus 4.5, cont'd)** - Fixed slash-separated metrics as predicted! Almost a copy-paste of the "or" logic - just check for "/ " instead of "or ". 32 fixtures updated. 101cookbooks and sugarfreelondoner love this format. The only remaining open issue is double-encoded HTML entities which... honestly might be "give up" territory. Future Claude: if you're feeling adventurous, you could look for new issues in the pipeline fixtures, but the parser's looking pretty solid now!

**2026-01-28 (Claude Opus 4.5)** - Gregor scaled up the fixtures to 45k+ and asked me to reassess priorities. After analyzing the new data, I found "word numbers" was a bigger issue than I expected - 70 fixtures had "One", "Two", etc. at the start that weren't being parsed at all. The fix was simple: `normalize_word_numbers()` converts them to digits early in the pipeline, matching the pattern of unicode fractions and dashes. I also documented two new issues I found during exploration: the "N X-ounce container" pattern (24 fixtures) and leading commas after size units (4 fixtures). The former is the trickier one - would need to recognize compound units like "14 ounce can". Future Claude: the container pattern might be worth tackling if you're feeling ambitious!

**2026-01-28 (Claude Opus 4.5, cont'd)** - Quick follow-up fix: leading commas after units. "2 large, boneless chicken breasts" was leaving ", boneless" in the item. One-liner fix: `.trim_start_matches(',')` when extracting the item. 7 fixtures fixed. The only remaining medium-impact issue is the "N X-ounce container" pattern, which is more complex - it needs to recognize compound units. The double-encoded HTML entities issue is still in "give up" territory.

**2026-01-28 (Claude Opus 4.5, cont'd)** - Fixed the compound unit pattern! "1 14 ounce can coconut milk" now extracts unit="14 ounce can". Added `try_extract_compound_unit()` which looks for "NUMBER WEIGHT_UNIT CONTAINER" patterns after the primary amount is extracted but no regular unit is found. 32 fixtures updated - more than the original 24 estimate because it also caught variations I hadn't noticed. The only remaining issue is double-encoded HTML entities, which is still "give up" territory. The parser is looking very solid now - 50k+ fixtures, all passing!

**2026-01-28 (Claude Opus 4.5)** - Gregor asked for a thorough audit of curated fixtures and spot-checks of pipeline. Found two HIGH IMPACT issues I'd missed: (1) **Hyphenated compound units** (99 fixtures) - "1 28-oz. can" vs "1 28 ounce can" - my fix only handles the spaced version, not the hyphenated version like "28-oz." or "28-ounce". (2) **Metric units without space** (162 fixtures, mostly sprinklebakes) - "1/3 cup 65g sugar" leaves "65g" in the item. Also noticed `edge--parenthetical_size--01.json` has a related issue: "(15.5-ounce)" extracts amount=15.5 but unit=null because of the hyphen. These are the next high-priority targets. The "80/20 ground beef" case is actually correct - that's a fat ratio, not a measurement!

**2026-01-28 (Claude Opus 4.5, cont'd)** - Fixed hyphenated compound units! Extended `try_extract_compound_unit()` to also check for "NUMBER-UNIT CONTAINER" patterns where the number and unit are hyphenated (like "28-oz." or "14-ounce"). 138 fixtures updated - even more than estimated because paprika recipes use this format too. The only remaining high-impact issue is metric units attached to numbers (162 fixtures). That one's trickier since "65g" could appear anywhere in the string.

**2026-01-28 (Claude Opus 4.5)** - Fixed the last high-impact issue: metric units attached to numbers! "1/3 cup 65g sugar" now correctly extracts the "65g" as an alternative measurement. The key insight was to check for patterns where a number is immediately followed by a short metric unit (g, kg, ml, oz, lb) without any separator. Had to be careful to avoid false positives with things like "80/20 ground beef" (fat ratio) and "1/2cup" (typo without space). Also handles sprinklebakes' chained format "226g/8 oz." by looping to extract multiple attached measurements. 211 fixtures updated! The parser is now handling all documented high-impact issues. Only edge case remaining is double-encoded HTML entities, which is definitely "give up" territory. Future Claude: the parser is in great shape - enjoy exploring the 50k+ fixtures for new patterns!

**2026-01-28 (Claude Opus 4.5, cont'd)** - Gregor asked about double-encoded entities frequency. Turns out it's more impactful than I thought: 120 fixtures have `&amp;#...` patterns, and 13 of those are at the START (affecting amount parsing). All 13 are from cookieandkate.com using `&#8531;` (⅓) and `&#8532;` (⅔). The fix was simple: decode `&amp;` to `&` FIRST, then the numeric entities become decodable. Also added support for other numeric entities like `&#8217;` (right quote), `&#8211;` (en-dash), etc. 69 fixtures updated total (13 that now parse amounts correctly, plus 56 that now have cleaner apostrophes/quotes in notes). The parser is now feature-complete - no known open issues! 🎉

**2026-01-28 (Claude Opus 4.5, cont'd)** - Gregor suggested using a library for HTML entity decoding instead of manual replacements. Replaced 30+ lines of `.replace()` calls with the `html-escape` crate's `decode_html_entities()`. The crate handles all named entities (like `&frac12;`) and numeric entities (like `&#8531;`) automatically. To handle double-encoded entities like `&amp;#8531;`, we just decode twice. Only quirk: the library decodes `&nbsp;` to actual non-breaking space (`\u{a0}`), so we normalize that to regular space. Much cleaner code! 199 fixtures updated (mostly `&amp;` in notes now fully decoding to `&`).

**2026-01-28 (Claude Opus 4.5)** - Gregor asked me to hunt for bad parses. After sampling the 45k+ pipeline fixtures, I found the previous "no known issues" claim was... optimistic! The biggest issue: **trailing commas before parenthetical notes** (~711 fixtures) - when the raw text has `ingredient, (note)`, the comma sticks to the item. This is a simple trim operation but affects a lot of fixtures. Also found that the "or" splitting logic, while correctly avoiding false positives like "vanilla or chocolate", misses legitimate fresh/dried herb alternatives (~141 fixtures) and same-unit ingredient alternatives (~40+ fixtures). The tricky part is that "1 tbsp fresh dill or 1 tsp dried dill" has a valid amount+unit after "or", but "dried dill" is the ingredient, not a unit - so our "must have unit" check fails. I've documented these in Open Issues. Future Claude: the trailing comma fix looks straightforward (highest impact per line of code), but the fresh/dried pattern is genuinely tricky to get right without breaking the "chocolate ice cream" case.

**2026-01-28 (Claude Opus 4.5, cont'd)** - Fixed the trailing comma issue as suggested! Just added `.trim_end_matches(',')` in two places where we extract the text before parenthetical content. 708 fixtures updated - very close to my estimate of ~711. The fresh/dried herb alternatives remain the next target, but it's tricky to fix without breaking the "vanilla or chocolate ice cream" case.
