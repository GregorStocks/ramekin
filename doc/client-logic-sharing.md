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
| Recipe scaling | `ramekin-ui/src/utils/scaleAmount.ts` | `ramekin-ios/Ramekin/RecipeScaleSupport.swift` | none | ~100 | **Yes** — Swift accepts locale decimals (`"1,5"`) and formats results with a locale-aware `NumberFormatter`; TS only accepts/emits `"."` decimals. Same amount can scale differently per platform. |
| Tag hierarchy | `ramekin-ui/src/utils/tagHierarchy.ts` | `ramekin-ios/Shared/TagHierarchySupport.swift` | `ramekin-core/src/tags.rs` (yes — validation lives here) | ~100–200 | Subtle — in-group sort uses plain lexicographic `.sort()` on raw names (web) vs `localizedCaseInsensitiveCompare` on parsed values (iOS); namespace-list ordering differs (`knownNamespaces` sorts seeded+discovered together, iOS keeps seeded order first). Seeded-namespace list is hardcoded in all three places. |
| Ingredient display formatting | scattered: inline in `ViewRecipePage.tsx`, `AddToShoppingListModal.tsx`, `utils/formatRecipeForDiff.ts` | centralized: `Ramekin/IngredientFormatting.swift` (+ `IngredientSectionGrouping.swift`) | none | ~60 + ~30 | Yes — diff formatting includes `[Section]` headers on iOS but not web; note/alt-measurement inclusion is parameterized on iOS, hardcoded per call site on web. |
| Meal plan date helpers | `utils/mealPlanHelpers.ts` + locals in `MealPlanPage.tsx` | `Shared/SharedDateFormatters.swift` + locals in `MealPlanView.swift` | none | ~40–50 | Week-start (Monday) computed with different algorithms (manual `getDay()` arithmetic vs `Calendar` weekday math); both currently correct. Web mixes `formatDateLocal`/`formatDateUtc` in one comparison — self-consistent today but fragile. |
| Shopping list category order | `ShoppingListPage.tsx` `CATEGORY_ORDER` | `ShoppingListView.swift` `categoryOrder` | `ramekin-core/src/ingredient_categorizer.rs` (computes categories; order is client-side) | ~15 | None today, but the 16-entry list is hardcoded in 3 places and any edit must be made 3×. |

Test coverage is lopsided: iOS has XCTest units for most of these
(`RecipeScaleSupportTests`, `TagHierarchySupportTests`,
`IngredientSectionGroupingTests`, `DateFormatterCachingTests`); ramekin-core
has Rust tests for tags and categorization. **The web client has no unit test
framework at all** — its only coverage is the Playwright UI suite. Web-side
drift is currently invisible until a user notices.

## Options evaluated

### 1. Push logic to the server

Works when the data already round-trips through an endpoint and doesn't need
to react instantly to client-side state. Categories already work this way
(server computes `category` per item; clients only order/group). Category
*order* is the one clear remaining candidate — already filed as
`p3-shopping-list-category-order-from-api`.

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
- The one real prerequisite: **ramekin-ui needs a unit test runner**. Vitest
  is the obvious choice (Vite is already the build tool). This is a modest,
  general-purpose investment — plenty of other web logic deserves unit tests
  too.

Cost is low, and it's incremental: vectors only encode behavior both sides
*should* share, so writing them forces the divergence decisions (e.g. is
`"1,5"` a parseable amount? what's the canonical formatting of 4/3?) instead
of leaving them latent.

## Per-area recommendations

| Area | Decision |
|---|---|
| Shopping list category order | **Option 1** — serve the canonical ordered list from the API. Already filed: `p3-shopping-list-category-order-from-api`. No new issue. |
| Recipe scaling | **Option 3** — pilot area for shared vectors. Reconcile the locale-decimal divergence while writing the vectors (decide: accept `","` decimals on both, and emit `"."` canonically on both). |
| Tag hierarchy | **Option 3, three-way** — vectors for parse/format/normalize/group-order consumed by Rust, TS, and Swift tests, with `tags.rs` as the source of truth for semantics. Keep the 6-entry seeded-namespace list duplicated (it's tiny and now CI-pinned); serving it from the API is not worth an endpoint today. |
| Ingredient display formatting | **Centralize web first** (it's scattered across three files; iOS already has the right shape), then add vectors for `formatted()`-equivalence in the same series as the other areas. |
| Meal plan date helpers | **Option 3, narrow** — vectors for week-start (date string → Monday date string) and `YYYY-MM-DD` formatting only. Display formatting (day headers) is allowed to differ per platform-idiom; don't vector it. |

## Follow-up issues filed

1. `p3-web-unit-test-runner` — add Vitest to ramekin-ui, wire into
   `make test`/CI. Prerequisite for everything below.
2. `blocked-shared-test-vectors-recipe-scaling` — pilot: vector file layout
   (`shared-test-vectors/` at repo root), TS + Swift consumption, scaling
   divergence reconciliation. Blocked on (1).
3. `p3-centralize-web-ingredient-formatting` — web-only refactor mirroring
   iOS's `IngredientFormatting`; independent of the test-runner work.
4. `blocked-shared-test-vectors-remaining-areas` — extend the pilot's
   conventions to tag hierarchy (three-way incl. Rust), meal-plan week-start,
   and ingredient formatting (after 3). Blocked on (2).

Plus the pre-existing `p3-shopping-list-category-order-from-api` for the
option-1 move.
