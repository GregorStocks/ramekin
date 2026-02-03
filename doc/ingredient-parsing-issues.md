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
8. **Document ALL issues you discover** in Open Issues, even if you're only fixing one. Future Claudes benefit from this documentation!
9. Create a PR, then stop - leave remaining issues for the next Claude

### Is It Worth Fixing?

Not every parsing quirk deserves a fix. For issues that seem one-in-a-million (e.g. an uncommon typo) or where it's not realistically possible to determine the original author's intent, it's fine to give up and say "we parse the entire ingredient string into `ingredient` and leave the amount, note, etc, blank". That's better than being _wrong_. Our #1 goal is not to be wrong.

### Curated vs Pipeline Fixtures

- **Curated** (`curated/`): Hand-picked test cases representing important scenarios. Update these manually when fixing issues.
- **Pipeline** (`pipeline/`): ~5500 auto-generated fixtures from real recipe sites. Run `make ingredient-tests-update` to sync these with current parser behavior.

## Known Issues

Issues are roughly ordered by potential impact. Update this list as you fix things or discover new issues.

### Open Issues

Updated with examples from `data/ingredient-categories.csv` (pipeline audit).

- [ ] **Leading list markers (&, -, +, *) before amounts** (~115 lines in ingredient-categories.csv) - Examples like "& 1/2 cups milk", "- 3 tablespoons ice water", "+ 1/3 cup panko breadcrumbs", "*5 cups flour". These look like list/bullet artifacts or HTML leftovers and should be stripped before parsing.

- [ ] **Leading parenthetical quantities/descriptors** (~27 lines in ingredient-categories.csv) - Examples like "(half block) cream cheese", "(two 6-oz./170g) salmon filets", "(about) parsley", "(one envelope unflavored powdered gelatin)". When the line begins with a parenthetical, try parsing it as amount/unit and move any leftovers into the note.

- [ ] **Standalone continuation lines starting with "and", "or", or "plus"** (~20 lines in ingredient-categories.csv) - Examples like "and 2 Tbsp sugar", "or 2 regular carrots", "plus 1 tablespoon extra-virgin olive oil". These likely belong to the previous ingredient as an alternative/addition but currently parse as orphaned lines.

- [ ] **Non-ingredient or equipment lines leaking into ingredient lists** (spotty but recurring) - Examples like "A 12-cup Bundt pan, a pastry bag, and a large star tip", "9x13 metal baking pans or large roasting pan lined with foil", "Burger accompaniments, as you like". Should be filtered upstream or flagged as section headers.

- [ ] **Trailing colons (section headers)** (~50 fixtures) - Items like "DRIZZLE:", "FILLING:", "For the dough:" are section headers that ended up in the ingredient list. These have no measurements and end with colons. Should be handled more principally by filtering them out as non-ingredients (or flagging them as section headers), rather than just stripping the colon. Examples from tasteofhome, barefeetinthekitchen.

- [ ] **Decimal amounts not converted to fractions** (~1100+ fixtures) - Amounts like "0.5", "0.75", "1.5" could be displayed as "1/2", "3/4", "1 1/2" for readability. This is a presentation choice - the current behavior isn't wrong, just less idiomatic for recipes. Would need a conversion function for common decimals.

- [ ] **Footnote asterisks in item names** (~7 fixtures) - "4-5 cherry tomatoes*" produces item="cherry tomatoes*". The asterisk is a footnote marker. Low impact and potentially risky to strip (what if someone writes "brand*name"?). Probably acceptable to leave as-is.

- [ ] **Concatenated ingredient lines** (unknown count) - "3/4 cup (180ml) milk 1/4 cup (60ml) vegetable oil" gets parsed as a single ingredient. This is an extraction bug upstream (ingredients should be on separate lines), not a parser bug. The parser can't reasonably split these without risking false positives.

- [ ] **Trailing semicolon in items** (~3 fixtures) - Semicolons used as separators stick to item names (e.g., `"item": "Parmesan cheese, grated;"`). Example from kingarthurbaking. Low impact.

- [ ] **Comma-separated conditional quantities** (~2 fixtures in paprika) - Ingredient lines with multiple quantities for different use cases, like `"4 cups vegetable broth (for dried but soaked chickpeas), 1 1/2 cups vegetable broth (for cooked chickpeas)"`. Currently only the first measurement is extracted; the second alternative ends up in the item field. This is tricky because the comma normally separates item from preparation notes, not alternatives. May need special handling for patterns where text after comma starts with a number+unit. Low fixture count but confusing output when it happens.

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

**2026-01-28 (Claude Opus 4.5)** - Fixed the fresh/dried herb alternatives! The key insight: the existing Step 4.5 handles " or " at the START of remaining text (e.g., "or 3 cups pineapple"), but misses " or " in the MIDDLE (e.g., "fresh dill or 1 teaspoon dried dill"). Added Step 5.5 to search for " or " anywhere in remaining and, if followed by a valid measurement, split there - the text before " or " becomes the item, and the alternative goes into the note. This also fixes ingredient-level alternatives like "1/2 cup water or 1/2 cup beef broth". 112 fixtures updated. The "vanilla or chocolate ice cream" case is still safe because "chocolate ice cream" has no unit, so we don't split. Only two low-priority issues remain: footnote asterisks (7 fixtures) and concatenated lines (upstream bug).

**2026-01-29 (Claude Opus 4.5)** - Fixed leading commas in parenthetical notes. When raw text has `"tomato (, sliced)"`, the comma was ending up in the note as `", sliced"` instead of `"sliced"`. One-liner fix: `.trim_start_matches(',').trim()` when extracting note from paren content. 247 fixtures updated - whiteonricecouple.com really likes this format! While exploring, I also documented two new issues: trailing ` )` in items (~111 fixtures) and trailing semicolons (~3 fixtures). Added a note to the workflow about documenting ALL issues found during exploration, not just the one you're fixing - future Claudes benefit from the breadcrumbs we leave behind. The parser journey continues!

**2026-01-29 (Claude Opus 4.5, cont'd)** - Fixed the trailing ` )` issue I documented earlier. When raw has `((45ml) )` (double parens with space before final paren), after normalization the orphaned `)` was sticking to the item. Simple fix: `.trim_end_matches(" )")` in Step 7. 111 fixtures updated. Only 2 low-impact issues remain: footnote asterisks (~7) and trailing semicolons (~3). The parser is looking solid!

**2026-01-29 (Claude Opus 4.5, cont'd)** - Fixed trailing commas in items (32 fixtures). While exploring for more issues, found two new high-impact patterns: (1) **trailing colons** (~50 fixtures) - section headers like "DRIZZLE:", "FILLING:" ending up as ingredients, mostly from tasteofhome; (2) **decimal amounts** (~1100+ fixtures) - amounts like "0.5" could be "1/2" for readability. The trailing colon issue is interesting - could strip the colon, or filter these out as non-ingredients entirely. Documented both for future Claudes to consider.

**2026-01-30 (Claude Opus 4.5)** - Fixed the "space before comma" issue! When parentheticals like `(about 2 ounces; 56 g)` are extracted, they leave a trailing space before any following comma: `"scallions , thinly sliced"`. Simple fix: `.replace(" ,", ",")` in Step 7. 154 fixtures updated (44 paprika + 110 pipeline). Found this by focusing on the user's actual paprika imports rather than the pipeline - good reminder that impact is measured by what people actually use. Also discovered some other patterns during exploration: pipe-separated measurements (11 fixtures), leading parentheticals in items (21 fixtures), but the space-comma issue had the best "fixtures affected in paprika data" to "complexity" ratio. Future Claude: the trailing colon issue should probably be handled by filtering out section headers entirely rather than just stripping the colon - they're not real ingredients.
