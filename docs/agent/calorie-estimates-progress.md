Claim: p3-deterministic-recipe-calorie-estimates

- [x] Implement backend, reproducible data, web and iOS
- [x] Add unit, API and UI tests
- [x] Run make test-core, make test (includes web units), make lint
- [x] Final make test-ui: 35 passed, including late-response regression
- [ ] iOS CI
- [x] Delete claimed issue
- [x] Review changes
- [ ] Submit through submit-pr; resolve CI/review feedback

Plan approved, including make nutrition-import. No task/update_plan tool available.
Use an authenticated POST accepting the displayed recipe ingredients, servings
and scale; avoids stale/version mismatches and keeps calculations server-side.

USDA import reproduces SHA-256 21ef907fcec4e0beb3b15910f45f04184ed0f720895df90234ddcb7adb5dc895.
All extraction/pipeline code is untouched; the new quantity grammar belongs to
the estimator. Calorie arithmetic and wording remain server-side. Review fixes
also extend both existing client scale helpers for ranges, mixed numbers,
compound measurements, and serving labels, pinned by shared scaling vectors.

PR #689. Addressed all three initial review findings. Local checks after fixes:
409 API/Python tests, 151 web unit tests, Rust suites, 35 browser tests, lint.
First-commit iOS unit CI passed; updated iOS/shared-vector changes await CI.

Second review: added comma-decimal quantities and colon serving prefixes on the
server and both client scale helpers, with shared vectors and API/browser checks.
All 412 API/Python tests, 155 web unit tests, Rust suites, 35 browser tests, and
lint pass after these changes. Both review rounds are addressed.
