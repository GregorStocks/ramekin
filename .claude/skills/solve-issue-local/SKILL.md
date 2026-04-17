---
name: solve-issue-local
description: Repo-specific instructions to use alongside the global solve-issue skill.
---

# Solve an Issue: Local Notes

Use these notes together with the global `solve-issue` skill.

## Repo-Specific Commands

- Read `Makefile` first and use existing `make` targets instead of invoking `docker` or `cargo` directly.
- Baseline verification is `make test` and `make lint`.
- If you change ingredient parsing, extraction, or URL-pipeline behavior, also run `make pipeline` and spot-check the resulting diff for regressions.
- `make setup-claude-web` is safe to run anywhere and is already covered by `make check-deps`.

## Repo-Specific Constraints

- Issues live in `issues/` as `pN-*.json5` or `blocked-*.json5`; see `doc/issues.md`.
- Do not edit generated code.
- Prefer Diesel DSL over raw SQL; ask before using raw SQL.
- Soft-delete data instead of hard-deleting it.
- Add end-to-end tests before using new API endpoints in the UI.
