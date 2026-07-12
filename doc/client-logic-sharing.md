# Sharing Business Logic Between Web and iOS Clients

Decision doc for issue `shared-client-business-logic-strategy` (2026-06-11).

Business logic that runs on both clients is currently written twice (sometimes
three times, counting the server) and drifts. This doc inventories the
duplicated areas, evaluates the three strategies from the issue, and makes a
per-area call. It is a decision record, not a refactor plan — the work items
are filed as separate issues (listed at the bottom).

## Inventory of duplicated logic

| Area | Web | iOS | Server (canonical?) | Pure-logic LOC/side | Known drift |
|---|---|---|---|---|---|
| Recipe scaling | `ramekin-ui/src/utils/scaleAmount.ts` | `ramekin-ios/Ramekin/RecipeScaleSupport.swift` | none | ~100 | Pinned by `shared-test-vectors/scale-amount.json`, including comma-decimal inputs and locale-independent output. |
| Tag hierarchy | `ramekin-ui/src/utils/tagHierarchy.ts` | `ramekin-ios/Shared/TagHierarchySupport.swift` | `ramekin-core/src/tags.rs` (yes — validation lives here) | ~100–200 | Pinned by `shared-test-vectors/tag-hierarchy.json`, including seeded namespace and group-member ordering. |
| Ingredient display formatting | centralized: `utils/ingredientFormatting.ts` (+ callers) | centralized: `Ramekin/IngredientFormatting.swift` (+ `IngredientSectionGrouping.swift`) | none | ~60 + ~30 | Pinned by `shared-test-vectors/ingredient-formatting.json`. |
| Meal plan date helpers | `utils/mealPlanHelpers.ts` | `Ramekin/MealPlanDateSupport.swift` | none | ~40–50 | Monday week-start and local `YYYY-MM-DD` formatting are pinned by `shared-test-vectors/meal-plan-dates.json`; display formatting remains platform-specific. |
| Shopping list category order | API response `categoryOrder` | API response `categoryOrder` | `server/src/api/shopping_list/list.rs` (yes) | ~15 | None — the canonical order now comes from the API. |
| Created-date day filters | delegated to the server | `Ramekin/RecipeSummaryCacheSupport.swift` (cache filter) | `ramekin-core/src/created_date_filter.rs` (yes) | ~15 | Pinned by `shared-test-vectors/created-date-filter.json`, including end-of-day fractional seconds. |

Test coverage is still lopsided, but less than it was when this decision was
written. iOS has XCTest units for most of these (`RecipeScaleSupportTests`,
`TagHierarchySupportTests`, `IngredientSectionGroupingTests`,
`DateFormatterCachingTests`); ramekin-core has Rust tests for tags and
categorization; and the web client now has Vitest wired through
`make ui-unit-test`. Web-side pure-logic drift can now be pinned with fast unit
tests instead of relying only on the Playwright UI suite.

## Options evaluated

### 1. Push logic to the server

Works when the data already round-trips through an endpoint and doesn't need
to react instantly to client-side state. Categories already work this way
(server computes `category` per item; clients only order/group). Category
*order* was the one clear remaining candidate, and that work has landed: the
shopping-list list and sync responses now include `category_order`.

Doesn't fit the other areas: scaling reacts to a client-side multiplier
without a server round-trip; display formatting and date grouping are
presentation-local and timezone-local.

### 2. Share code across clients (Rust→WASM/UniFFI, or a JS core via JavaScriptCore)

Evaluated honestly against this repo's build reality:

- No WASM or UniFFI exists anywhere today; both would be net-new toolchains.
- `ramekin-core` depends on `tokio`, `reqwest`, `image`, `openai-api-rs` — it
  does not compile to WASM as-is. Sharing it would mean carving out a new
  pure-logic sub-crate, then maintaining a wasm-pack/wasm-bindgen step for the
  web bundle and a UniFFI + static-lib (or XCFramework) step for iOS.
- iOS CI only runs on PRs touching `ramekin-ios/**`, on a Mac runner; a Rust
  change feeding generated bindings would need CI path/trigger rework.
- The repo already carries a heavy codegen pipeline (OpenAPI → Rust/TS/
  Python/Swift clients). Another generated-artifact toolchain is real ongoing
  maintenance weight.
- The prize is small: every duplicated area above is ~15–200 lines of simple
  pure logic. The toolchain would cost far more than rewriting any of these
  from scratch.

A shared JS core via JavaScriptCore on iOS was also considered: it avoids the
Rust packaging problem but adds a runtime bridge, awkward debugging, and a
second place where "Swift-native" conventions break down. Same poor
cost/benefit at this scale.

**Verdict: not now.** Revisit if a genuinely large piece of core logic needs
to run client-side — the concrete trigger would be wanting `ramekin-core`'s
ingredient parser/categorizer on-device (offline capture, instant
categorization while typing). At that point compile-shared Rust is the only
option that doesn't mean re-implementing a parser twice, and the sub-crate
split sketched above is the way to do it.

### 3. Accept duplication, pin it with shared test vectors

A checked-in JSON file of input/expected pairs per area, consumed by the TS
unit tests, the Swift XCTests, and (for three-way areas like tags) the Rust
tests. Divergence fails CI instead of being discovered by users.

This fits this repo well:

- Rust already has exactly this pattern: `ramekin-core/tests/fixtures/
  ingredient_parsing/` is checked-in JSON loaded by
  `ramekin-core/tests/ingredient_parsing_tests.rs`.
- XCTest can load JSON fixtures as test resources (xcodegen `project.yml`
  resource entry pointing at the shared directory).
- The original prerequisite has landed: ramekin-ui has a Vitest unit-test
  runner exposed as `make ui-unit-test`, and web logic tests already consume
  shared JSON vectors.

Cost is low, and it's incremental: vectors only encode behavior both sides
*should* share, so writing them forces the divergence decisions (e.g. is
`"1,5"` a parseable amount? what's the canonical formatting of 4/3?) instead
of leaving them latent.

## Per-area recommendations

| Area | Decision |
|---|---|
| Shopping list category order | **Option 1, done** — the API now serves the canonical ordered list in shopping-list list and sync responses. |
| Recipe scaling | **Option 3, pilot done** — `shared-test-vectors/scale-amount.json` is consumed by both web Vitest and iOS XCTest. |
| Tag hierarchy | **Option 3, done** — vectors for parse/format/normalize/group-order are consumed by Rust, TS, and Swift tests, with `tags.rs` as the source of truth for semantics. The 6-entry seeded-namespace list stays duplicated (it's tiny and CI-pinned); serving it from the API is not worth an endpoint. |
| Ingredient display formatting | **Option 3, done** — vectors pin `formatted()`-equivalence across web and iOS. |
| Meal plan date helpers | **Option 3, done** — narrow vectors pin week-start and `YYYY-MM-DD` formatting. Display formatting such as day headers remains platform-specific. |

## Preventing future drift

The decisions above clean up today's duplication; new features can quietly
re-create it. Three mechanisms keep the pattern alive:

1. **AGENTS.md rule.** Development in this repo is largely agent-driven, and
   AGENTS.md is loaded at the start of every session — it is the closest
   thing we have to authoring-time enforcement. It requires new dual-client
   pure logic to either live on the server or ship with shared test vectors in
   the same PR.

2. **The vectors themselves are the regression net.** Once an area is
   vectored, changing behavior on one client fails that client's tests until
   the vector file is updated, and updating the vector file exercises the
   other client's tests. iOS CI (`.github/workflows/ios.yml`) now includes
   `shared-test-vectors/**` in its trigger paths, so vector edits run the
   Swift side as well.

3. **Review-time check.** When a PR adds parallel logic to `ramekin-ui/src`
   and `ramekin-ios` without touching `shared-test-vectors/`, that is the
   smell to look for in review. If drift keeps slipping through anyway, the
   escalation is a periodic parity audit (compare the support files listed
   in the inventory above) — not worth automating until there's evidence
   it's needed.

## Follow-up issues filed

Done and deleted per `doc/issues.md`: the web Vitest runner, the recipe-scaling
shared-vector pilot, centralized web ingredient formatting, and API-provided
shopping-list category order.

The shared-vector follow-ups are complete: recipe scaling, tag hierarchy,
meal-plan week-start, and ingredient formatting all consume checked-in vectors.
