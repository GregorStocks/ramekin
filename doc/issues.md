# Issues

Use the shared `agent-issues` workflow for issue file format, claiming, linting,
and PR submission. Keep this document limited to Ramekin-specific issue guidance.

Resolved issues are deleted in the PR that fixes them.

## Ramekin Notes

- Prefer issue labels that match the affected area, such as `ingredient-parser`,
  `ios`, `web`, `pipeline`, `api`, or `agent-issues`.
- For bugs or features that affect both clients, describe the expected web and
  iOS behavior in the issue unless the work is explicitly scoped to one client.
- For deterministic client logic, call out whether the fix should move to the
  server or needs mirrored web/iOS tests as described in
  `doc/client-logic-sharing.md`.
- For extraction, parsing, or URL-pipeline behavior, state whether `make
  pipeline` is expected to produce intentional `data/` diffs.
