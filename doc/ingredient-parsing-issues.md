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

### Open Issues

#### Low Impact / Edge Cases

- [ ] **Double-encoded HTML entities** - "&amp;#8531;" (double-encoded 1/3 fraction) not decoded.
  - Curated fixture: `edge--double_encoded_entity--01.json`
  - This is rare. Might be worth adding more decode passes, or might be "just give up" territory.

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
