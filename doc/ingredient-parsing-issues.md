# Ingredient Parsing Issues

This document tracks known ingredient parsing issues and guides contributors through fixing them.

## How This Works

The ingredient parser in `ramekin-core/src/ingredient_parser.rs` converts raw ingredient strings like "2 cups flour, sifted" into structured data (amount, unit, item, note). It's impossible to handle every weird format perfectly, so we fix issues one at a time based on impact.

### The Workflow

1. Look at the curated test fixtures in `ramekin-core/tests/fixtures/ingredient_parsing/curated/`
2. These fixtures document both working behavior and known issues
3. Pick an issue that seems worth fixing (see criteria below - you can add new issues that you discover, past and future Claudes have access to this file.)
4. Implement the fix in `ingredient_parser.rs`
5. Update the curated fixture to expect the correct behavior
6. Run `make ingredient-tests-update` to update bulk fixtures
7. Run `make ingredient-tests` and `make lint` to verify

### Is It Worth Fixing?

Not every parsing quirk deserves a fix. For issues that seem one-in-a-million (e.g. an uncommon typo) or where it's not realistically possible to determine the original author's intent, it's fine to give up and say "we parse the entire ingredient string into `ingredient` and leave the amount, note, etc, blank". That's better than being _wrong_. Our #1 goal is not to be wrong.

### Curated vs Bulk Fixtures

- **Curated** (`curated/`): Hand-picked test cases representing important scenarios. Update these manually when fixing issues.
- **Bulk** (`bulk/`): ~5500 auto-generated fixtures from real recipe sites. Run `make ingredient-tests-update` to sync these with current parser behavior.

## Known Issues

Issues are roughly ordered by potential impact. Update this list as you fix things or discover new issues.

### Fixed

- [x] **"of" not stripped after units** - "4 cloves of garlic" produced item="of garlic". Fixed by stripping "of " after recognized units in `extract_unit()`.

### Open Issues

#### High Impact (Common Patterns)

- [ ] **"scant" prefix breaks parsing** - "scant 1 teaspoon salt" fails to parse entirely (item becomes the whole string, no measurements). The word "scant" before the amount confuses the parser.
  - Curated fixture: `edge--scant--01.json`
  - Potential fix: Recognize "scant" as a measurement qualifier and strip it before parsing amount

- [ ] **"heaping" prefix breaks parsing** - "2 heaping tablespoons miso" extracts only "2" with no unit. The word "heaping" between amount and unit breaks unit extraction.
  - Curated fixture: `edge--heaping--01.json`
  - Potential fix: Recognize "heaping" as a unit qualifier, either strip it or include it in the unit

- [ ] **"and" in mixed numbers** - "2 and 1/2 teaspoons cinnamon" extracts only "2". The parser doesn't recognize "X and Y/Z" as a mixed number.
  - Curated fixture: `edge--and_mixed_number--01.json`
  - Potential fix: Handle "and" between whole number and fraction in `extract_amount()`

#### Medium Impact

- [ ] **Hyphen range with spaces** - "1 - 2 potatoes" extracts only "1". The parser handles "1-2" but not "1 - 2" (spaces around hyphen).
  - Curated fixture: `edge--hyphen_range--01.json`
  - Potential fix: Normalize " - " to "-" before range detection, or handle spaced hyphens explicitly

- [ ] **"or" alternatives in item** - "1 pound or 3 cups frozen pineapple" puts "or 3 cups frozen pineapple" in the item name.
  - Curated fixture: `edge--or_alternative--01.json`
  - Potential fix: Could extract the "or X" as an alternative measurement, or just truncate at "or". Tricky because "or" can appear in item names too.

- [ ] **Slash-separated metrics in item** - "3.5 oz / 100g celery root" puts "/ 100g celery root" in the item.
  - Curated fixture: `edge--slash_metric--01.json`
  - Potential fix: Recognize " / " followed by a measurement as an alternative, similar to parenthetical handling

#### Low Impact / Edge Cases

- [ ] **Double-encoded HTML entities** - "&amp;#8531;" (double-encoded 1/3 fraction) not decoded.
  - Curated fixture: `edge--double_encoded_entity--01.json`
  - This is rare. Might be worth adding more decode passes, or might be "just give up" territory.

## Useful Commands

```bash
# Run ingredient parsing tests
make ingredient-tests

# Update bulk fixtures after changing parser
make ingredient-tests-update

# Run just the curated tests (faster iteration)
cd ramekin-core && cargo test ingredient_parsing_curated -- --nocapture

# Search bulk fixtures for a pattern
grep -r "pattern" ramekin-core/tests/fixtures/ingredient_parsing/bulk/
```

## File Locations

- Parser implementation: `ramekin-core/src/ingredient_parser.rs`
- Curated fixtures: `ramekin-core/tests/fixtures/ingredient_parsing/curated/`
- Bulk fixtures: `ramekin-core/tests/fixtures/ingredient_parsing/bulk/`
- Test runner: `ramekin-core/tests/ingredient_parsing_tests.rs`
