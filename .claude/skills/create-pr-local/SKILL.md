---
name: create-pr-local
description: Repo-specific instructions to use alongside the global create-pr skill.
---

# Create a Pull Request: Local Notes

Use these notes together with the global `create-pr` skill.

## Pre-Validation

- Read `Makefile` first and use existing `make` targets instead of invoking `docker` or `cargo` directly.
- Before opening a PR, run `make test` and `make lint`.
- If you change ingredient parsing, extraction, or URL-pipeline behavior, also run `make pipeline` and commit the resulting data/report diffs.

## Repo-Specific Constraints

- Do not edit generated code.
- If validation or pipeline runs dirty tracked files, inspect the diff and commit intentional artifacts before creating the PR.
