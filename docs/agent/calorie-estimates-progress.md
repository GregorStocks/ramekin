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
All extraction/pipeline code is untouched; the quantity grammar belongs only to
the new estimator. Calculation and display wording are entirely server-side,
so no duplicated client business logic requires shared vectors.
