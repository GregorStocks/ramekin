Shared test vectors pin deterministic client logic that is implemented in more
than one runtime.

Each JSON file contains named cases consumed directly by the relevant platform
unit tests. When a behavior changes, update the vector first and make every
client match it in the same PR.

- `scale-amount.json`: web and iOS recipe amount scaling.
- `tag-hierarchy.json`: Rust, web, and iOS tag parsing and client ordering.
- `meal-plan-dates.json`: web and iOS local date formatting and Monday starts.
- `ingredient-formatting.json`: web and iOS ingredient display formatting, and
  the Rust server's Paprika export lines (the cases without scaling or derived
  grams, which the export never applies).
- `ingredient-editor-rows.json`: web and iOS conversion between a recipe's
  ingredients and the recipe form's flat list of section-heading and
  ingredient rows.
- `recipe-title-sort.json`: Rust server and iOS case-folded recipe title ordering.
- `created-date-filter.json`: Rust server and iOS inclusive UTC-day created-date
  filtering. Timestamps stop at millisecond precision because Foundation's date
  parsing truncates below that; the Rust unit tests additionally pin the
  microsecond boundary the server can store.
- `calorie-poll-delays.json`: web and iOS waits between calorie-estimate polls
  while names are still being recognized: doubling from 2 s to 30 s, then
  stopping once the waits reach 5 minutes. The schedule ends after the last
  delay.
