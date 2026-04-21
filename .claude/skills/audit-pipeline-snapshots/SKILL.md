---
name: audit-pipeline-snapshots
description: Pull the next recipe(s) from the pipeline snapshot audit queue, cross-reference each against its cached source HTML, and file issues for any real conversion bugs. Use for a spot-check of extraction/parsing quality, or after changing extraction code to hunt for regressions.
---

# Audit Pipeline Snapshots

Pick recipe snapshot(s) from the ordered queue in
`docs/agent/pipeline-snapshot-audit-queue.md`, compare each to the original
source HTML in `~/.ramekin/http-cache/`, and file issues for real bugs. Takes
an optional N (default 1) = how many snapshots to look at.

The queue is meant to be stateful: it tracks what we've already audited, and
the default workflow is "top N not-yet-audited snapshots," not fresh random
sampling every time.

## 1. Pull the next snapshots from the queue

Open `docs/agent/pipeline-snapshot-audit-queue.md`. By default, take the top
`N` entries still marked `[ ]`.

The queue is already ordered to round-robin across sites, with a deterministic
pseudo-random order within each site, so don't reshuffle it during normal use.

If you want a quick shell view of the next candidates:

```
rg '^- \\[ \\]' docs/agent/pipeline-snapshot-audit-queue.md | head -n "${N:-1}"
```

For each snapshot you audit, update that same queue entry in-place:

- Change `[ ]` to `[x]`.
- Append a brief audit note with the date and outcome.
- If you filed or updated issue(s), name the issue file(s).
- If no bug was found, say `no issue`.

Example:

```
- [x] 007. `cooking-nytimes-com_recipes-1019683-mozzarella-in-carrozza-fried-mozzarella-sandwiches.json` (cooking-nytimes-com) - 2026-04-21: no issue
```

## 2. For each sampled snapshot, cross-reference the source

The source HTML for `foo-com_some-recipe-slug.json` lives at
`~/.ramekin/http-cache/foo-com_some-recipe-slug/response.bin` — same basename,
minus `.json`, as a directory containing `response.bin` (HTML despite the
extension) and `metadata.json`.

Read the snapshot JSON. Then grep the HTML for the recipe body —
ingredient/method paragraphs usually live in the `entry-content` div; useful
anchors are measurement words (`teaspoon`, `tablespoon`, `cup`, `ounce`,
`pound`) and section headings (`<b>`, `<h2>`, `<h3>`). Sniff-test every field:

- **Title**: matches the recipe's heading on the page?
- **Instructions**: every method paragraph from the source captured? Nothing
  from sidebars/comments leaking in?
- **Ingredients**: right count, right order, grouped into `section`s when the
  source has multiple sub-recipes (marinade + sauce, cake + frosting, etc.)?
- **`item` names**: faithful to `raw`, or hallucinated extras tacked on?
- **`measurements`**: do any entries look like they came from parentheticals
  that are really yield hints (`(2 lemons)`, `(about 3 cups)`) rather than
  alternate quantities?
- **`image_urls`**: HTML-entity-decoded (no `&#038;` etc.)?
- **Multi-recipe pages**: did two recipes on one URL get flattened into one
  record with parts missing?

Don't trust your first read. Before writing up an issue, go back to the HTML
and confirm the pipeline is actually wrong (not just differently formatted).
Withdraw any claim the source doesn't support.

## 3. Check for existing issues BEFORE filing

For each candidate issue, search existing ones:

```
issue-query --search "<keyword>"
```

Search on the symptom, not your phrasing — e.g. for a parenthetical-parsing
bug, try `parenthetical`, `lemon`, `measurement`, `unitless`. If there's
already an open issue covering the same behavior, add your example to its
`description` (Edit the file) instead of filing a duplicate. Skip filing if
covered.

## 4. File the new issues

Format is in `doc/issues.md`. Filename: `pN-short-kebab-title.json5`
(priority 1 highest, 4 lowest). Use real ISO-8601 timestamps (`date -Iseconds`).
Include a **concrete example**: snapshot path, the offending raw text, and
what the snapshot produced. Reference the cached HTML path so a future agent
can re-verify.

Priority rubric for pipeline issues:

- **p2**: content is missing or wrong in a way a user would notice (dropped
  instructions, mis-scaled quantities, merged recipes).
- **p3**: parsing/normalization warts that produce junk fields or dumb item
  names but don't break the recipe.
- **p4**: cosmetic (entity encoding, whitespace, suggested-tag quality).

After writing each file, run `issue-fmt` from the repo root; it reflows long
strings into json5's line-continuation style and lint will reject anything
it hasn't touched. Then `make lint` to confirm the Issues linter is green.

## Gotchas

- **`response.bin` is HTML**, not a binary. The `Read` tool refuses it; use
  Grep / `sed -n` ranges / `Bash` instead.
- **Keep the queue authoritative.** If you audited a snapshot, mark it in
  `docs/agent/pipeline-snapshot-audit-queue.md` before you stop.
- **Don't over-file.** If the snapshot matches the source faithfully but the
  source itself is weirdly written, that's not a pipeline bug.
- **Don't conflate related bugs.** Two symptoms from one root cause should be
  one issue; two independent bugs in the same recipe should be two. When in
  doubt, ask.
- **The pipeline is LLM-backed**, so some "hallucinations" are actual features
  (brand guesses, tag suggestions). Confirm with the user before filing an
  issue that removes a feature rather than fixing a bug.
- **Ingredient duplication is a signal.** Two identical `item`s with
  `section: null` in the same record almost always means a multi-recipe page
  got merged — check the source for a second `<b>`/`<h3>` heading.
