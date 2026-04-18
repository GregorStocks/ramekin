# Hierarchical Tags — Design

## Summary

Introduce two-level hierarchical tags of the form `namespace:value` (e.g., `ingredient:chicken`, `course:breakfast`). Namespaces are seeded with a default list and extensible per user. The hierarchy is a string convention layered over the existing flat tag storage — no schema migration required. Web UI groups tags by namespace in the cookbook filter, tag picker, and management page. iOS v1 is display-only; a follow-up issue covers full iOS parity.

## Goals

- Let users organize tags into meaningful categories (ingredient, course, cuisine, etc.).
- Make it easy to filter the cookbook along multiple namespaces at once, with the same AND-everywhere semantics as all other filters.
- Avoid breaking existing tags or forcing users to re-tag anything.

## Non-goals

- Arbitrary-depth hierarchy.
- Per-namespace metadata (colors, icons, sort order).
- Namespace synonyms/aliases.
- iOS feature parity in this change.
- Backfilling existing tags automatically (tracked as a follow-up).

## Data model

**No schema changes.** `user_tags.name` (CITEXT) continues to hold the whole tag as a single string.

A tag is hierarchical iff its name contains exactly one colon and both sides are non-empty. Otherwise it is "uncategorized". Multi-colon names like `a:b:c` are treated as uncategorized (we refuse to split them to avoid ambiguity), and validation prevents new tags with that shape.

Shared helpers live in `ramekin-core/src/tags.rs`:

```rust
pub const SEEDED_NAMESPACES: &[&str] =
    &["ingredient", "course", "cuisine", "diet", "method", "season"];

pub struct ParsedTag {
    pub namespace: Option<String>,
    pub value: String,
}

pub fn parse_tag(name: &str) -> ParsedTag;
pub fn format_tag(namespace: Option<&str>, value: &str) -> String;
pub fn validate_tag_name(name: &str) -> Result<(), TagNameError>;
```

Validation rules (`validate_tag_name`):

- At most one colon.
- If a colon is present, both sides must be non-empty after trim.
- Namespace matches `^[a-z][a-z0-9_-]*$` (lowercase, hyphen/underscore allowed).
- Value is any printable UTF-8 except `:`.
- Whitespace is trimmed from each side before validation and before storage.

Casing: values preserve the casing the user typed; comparisons use CITEXT.

The namespace list visible to a user = union of `SEEDED_NAMESPACES` and `DISTINCT split_part(name, ':', 1)` across that user's hierarchical tags.

## Server / API

No new endpoints.

- `POST /tags` and `PATCH /tags/:id` (or whatever the current create/rename routes are): apply `validate_tag_name`, return a structured validation error the UI can map to field-level messages.
- `GET /tags` responses gain two fields per item, derived via `parse_tag`:
  - `namespace: Option<String>`
  - `value: String`
  The existing `name` field stays. Consumers that don't know about the new fields are unaffected.
- Existing tag-filter query path is unchanged. No new filter token syntax (the existing `tag:"ingredient:chicken"` quoted form already handles colons in tag names).
- Auto-tag prompt updated so the AI emits namespaced suggestions when the user already uses that pattern. It still only returns tags from `user_tags`, so no dictionary work.

OpenAPI spec regenerates; generated clients pick up the new optional fields.

## Web UI

The visual styling pass uses the `frontend-design` skill during implementation; this section fixes the behavior and structural decisions.

### TagInput (create/edit recipe)

- Replace the single free-text input with a namespace dropdown plus a value text field.
- Dropdown options: `(none)` first, seeded namespaces next, then user-discovered extras alphabetically, then `+ New namespace…` last.
- Typing `ingredient:chi` in the value field is a shortcut: it auto-sets the namespace dropdown to `ingredient` and strips the prefix from the value field.
- The "available tags" list below is grouped by namespace (collapsible sections). Clicking a chip still adds that tag.
- Entering a namespace that matches `^[a-z][a-z0-9_-]*$` from the `+ New namespace…` affordance makes it selectable for this and subsequent tags in the session.

### Cookbook filter panel

- The existing "Tags" section becomes grouped: one collapsible subsection per namespace (seeded first, then user extras alphabetically, then "Uncategorized" last).
- Each subsection shows a chip row with counts; empty namespaces are hidden.
- Selection behavior is unchanged: chips toggle on/off, selections are AND'd with every other filter, URL state is stored as `tag:"ns:value"` tokens.

### TagsPage (tag management)

- Grouped by namespace with the same ordering as the filter panel.
- Rename preserves the full name (including `ns:`). Renaming `ingredient:chicken` to `chicken` moves the tag to Uncategorized, by design.
- A "Change namespace" affordance is a convenience shortcut for rename that just swaps the prefix.

### ViewRecipePage / CookbookPage recipe cards

- Tag chips stay inline. The namespace prefix is visually de-emphasized (smaller/greyer text before the value). The exact styling is handled during the `frontend-design` pass.

## iOS (v1)

Display-only, minimal work:

- Tag chips on recipe views render the raw `ingredient:chicken` string.
- The existing flat tag picker is unchanged. Typing `ingredient:chicken` creates that tag; server-side validation enforces the rules.
- No grouped UI, no filter redesign, no namespace dropdown.

A follow-up issue covers full iOS parity.

## Migration

None. Existing tags without a colon are automatically "Uncategorized" via `parse_tag` returning `namespace = None`. No SQL migration, no data changes.

## Follow-up issues

Filed as part of this work in `issues/`:

- `p3-ai-recategorize-tags.json5` — pipeline step that uses AI to propose namespace assignments for a user's existing uncategorized tags. Semantics TBD; likely a dry-run plus user-approval flow so nothing changes silently.
- `p3-ios-hierarchical-tags-ui.json5` — bring iOS to parity with web: grouped tag picker, grouped filter UI, styled namespace prefix on chips.

## Testing

- **Rust unit tests** in `ramekin-core/src/tags.rs` for `parse_tag`, `format_tag`, and `validate_tag_name`: empty sides, multi-colon, whitespace handling, casing, namespace charset (valid and invalid), values containing colons (rejected).
- **Server integration tests**: create/rename validation errors surface with structured payload; `GET /tags` responses include `namespace` and `value`.
- **UI tests**: existing cookbook filter tests extended to cover grouped rendering, URL round-tripping of a `tag:"ns:value"` token, and a tag with a namespace that has no seeded entry.
- **iOS**: no new tests; display-only behavior is covered by existing tag tests.

## Risks & open questions

- Users who happen to have existing tags with colons (from some earlier import) will suddenly look "categorized". This is fine — it matches their intent — but worth noting.
- `format_tag` must never be used to reconstruct a name for storage; storage always uses the raw `name`. This is a convention we need to document in the module, because round-tripping through `parse` + `format` would be a latent bug source if someone tried it.
