## Verification issues discovered while fixing mobile nav

- `make lint` currently fails on `origin/master` because multiple issue files in `issues/` have descriptions longer than 120 characters. The failure is emitted by the issue validation step inside `scripts/lint.py`, not by the mobile nav change.
- `make test-ui` currently exits early from the process-compose harness with only one line in `logs/test-ui.log`:
  `Mock OpenRouter server running on port ...`
  The new mobile-nav Playwright test was added in `tests/ui/test_smoke.py`, but the UI orchestration failure prevents the suite from reaching test execution in this worktree.
- GitHub Actions currently fails the shared `Lint and Test` workflow for reasons unrelated to this fix:
  - `p4-extract-footnote-text.json5` is malformed on the base branch and fails issue JSON5 validation in CI.
  - the Linux runner is missing the Rust `clippy` component for toolchain `1.92.0`, so all Rust lint jobs fail before evaluating the mobile-nav change.
