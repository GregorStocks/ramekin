# Ingredient Parser Guide

This document covers specifics for working on the ingredient parser. See `doc/issues.md` for issue format and queries.

## How the Parser Works

The ingredient parser in `ramekin-core/src/ingredient_parser.rs` converts raw ingredient strings like "2 cups flour, sifted" into structured data (amount, unit, item, note). It's impossible to handle every weird format perfectly, so we fix issues one at a time based on impact.

## Curated vs Pipeline Fixtures

- **Curated** (`ramekin-core/tests/fixtures/ingredient_parsing/curated/`): Hand-picked test cases representing important scenarios. Update these manually when fixing issues.
- **Pipeline** (`ramekin-core/tests/fixtures/ingredient_parsing/pipeline/`): ~5500 auto-generated fixtures from real recipe sites. Run `make ingredient-tests-update` to sync these with current parser behavior.

## Workflow for Ingredient Parser Issues

1. Look at the curated test fixtures in `ramekin-core/tests/fixtures/ingredient_parsing/curated/`
2. These fixtures document both working behavior and known issues
3. Implement the fix in `ingredient_parser.rs`
4. Update the curated fixture to expect the correct behavior
5. Run `make ingredient-tests-update` to update pipeline fixtures
6. Run `make test` and `make lint` to verify
7. Run `make pipeline` and spot-check the resulting fixture changes with `git diff`. Look for regressions (good parses that got worse) and verify the fix is working as intended.

## Useful Commands

```bash
# Run ingredient parsing tests
make test

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
