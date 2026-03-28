The `p4-orphaned-x-multiplier` fix now handles leading multiplier forms where the left-hand count is still present, including:

- `2x 14 ounce cans black beans`
- `2 x 400g cans cannellini beans drained`

One related shape is still present in pipeline fixtures and was intentionally left alone:

- `x packs  shortcrust pastry`

That line has no surviving left-hand count, so the current parser cannot safely infer whether it was originally `2 x packs`, `2 x 320g packs`, or something else. It likely needs upstream extraction work or a separate parser heuristic that only fires when a bare leading `x` package pattern is clearly recoverable.

Another nearby shape remains ambiguous:

- `10 x 10 inch sheet of frozen puff pastry`

This might represent a multiplier (`10` sheets) or a dimension (`10 x 10 inch`). I left it unchanged because the safe parser fix in this PR was specifically about multiplier wrappers around recognizable measurement/unit expressions.
