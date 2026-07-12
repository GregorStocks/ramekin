Shared test vectors pin deterministic client logic that is implemented in more
than one runtime.

Each JSON file contains named cases consumed directly by the relevant platform
unit tests. When a behavior changes, update the vector first and make every
client match it in the same PR.

- `scale-amount.json`: web and iOS recipe amount scaling.
- `tag-hierarchy.json`: Rust, web, and iOS tag parsing and client ordering.
- `meal-plan-dates.json`: web and iOS local date formatting and Monday starts.
- `ingredient-formatting.json`: web and iOS ingredient display formatting.
- `recipe-title-sort.json`: Rust server and iOS case-folded recipe title ordering.
- `created-date-filter.json`: Rust server and iOS inclusive UTC-day created-date
  filtering. Timestamps stop at millisecond precision because Foundation's date
  parsing truncates below that; the Rust unit tests additionally pin the
  microsecond boundary PostgreSQL can store.
