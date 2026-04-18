# Hierarchical Tags Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce two-level `namespace:value` tags (e.g., `ingredient:chicken`, `course:breakfast`) as a string convention over the existing flat tag storage, with seeded + user-extensible namespaces and grouped web UI.

**Architecture:** A shared Rust module `ramekin-core/src/tags.rs` owns parsing and validation. `user_tags.name` stays a single CITEXT string — no schema migration. The API gains `namespace`/`value` fields on existing tag payloads. The web UI (SolidJS) groups tags by namespace in `TagInput`, `CookbookPage` filter panel, and `TagsPage`. iOS is display-only for v1 (two follow-up issues filed).

**Tech Stack:** Rust (axum + diesel), SolidJS, Playwright (Python) for UI tests, pytest for API tests.

**Reference spec:** `docs/superpowers/specs/2026-04-18-hierarchical-tags-design.md`

---

## File Structure

### Create

- `ramekin-core/src/tags.rs` — parse/format/validate helpers, seeded namespaces const.
- `ramekin-ui/src/utils/tagHierarchy.ts` — client-side parse/format/seeded-namespaces mirror.
- `issues/p3-ai-recategorize-tags.json5`
- `issues/p3-ios-hierarchical-tags-ui.json5`

### Modify

- `ramekin-core/src/lib.rs` — add `pub mod tags;` and re-exports.
- `server/src/api/tags/create.rs` — call `validate_tag_name`.
- `server/src/api/tags/rename.rs` — call `validate_tag_name`.
- `server/src/api/tags/list.rs` — add `namespace` + `value` fields to `TagItem`.
- `ramekin-core/src/ai/prompts/auto_tag.rs` — mention the namespace convention.
- `ramekin-ui/src/components/TagInput.tsx` — namespace dropdown + value field + grouped available list.
- `ramekin-ui/src/pages/CookbookPage.tsx` — grouped tag filter subsections.
- `ramekin-ui/src/pages/TagsPage.tsx` — grouped tag list.
- `tests/test_tags.py` — validation test cases + `namespace`/`value` field assertions.
- `tests/ui/test_smoke.py` — grouped filter smoke test.

---

## Task 1: Core tag helpers (parse/format/validate) with unit tests

**Files:**
- Create: `ramekin-core/src/tags.rs`
- Modify: `ramekin-core/src/lib.rs`

- [ ] **Step 1: Create `ramekin-core/src/tags.rs` with implementation and tests**

```rust
//! Hierarchical tag helpers.
//!
//! Tags use a `namespace:value` string convention layered over the flat
//! `user_tags.name` column. A tag is hierarchical iff it contains exactly
//! one colon and both sides are non-empty. Multi-colon names are always
//! "uncategorized" — we never split them. Storage always uses the raw
//! `name`; `format_tag` is a UI convenience and must not round-trip
//! through `parse_tag` for persistence.

use std::sync::LazyLock;

use regex::Regex;

/// Namespaces shown in the UI even when the user has no tags in them yet.
pub const SEEDED_NAMESPACES: &[&str] =
    &["ingredient", "course", "cuisine", "diet", "method", "season"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTag {
    pub namespace: Option<String>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagNameError {
    Empty,
    MultipleColons,
    EmptyNamespace,
    EmptyValue,
    InvalidNamespace,
}

impl TagNameError {
    pub fn message(&self) -> &'static str {
        match self {
            TagNameError::Empty => "Tag name cannot be empty",
            TagNameError::MultipleColons => {
                "Tag name may contain at most one colon (namespace:value)"
            }
            TagNameError::EmptyNamespace => "Namespace cannot be empty",
            TagNameError::EmptyValue => "Tag value cannot be empty",
            TagNameError::InvalidNamespace => {
                "Namespace must be lowercase letters, digits, hyphen, or underscore, starting with a letter"
            }
        }
    }
}

static NAMESPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z0-9_-]*$").unwrap());

/// Parse a tag name into its optional namespace and value. Never returns
/// an error; inputs that don't match the `namespace:value` shape are
/// returned as a value with `namespace = None`.
pub fn parse_tag(name: &str) -> ParsedTag {
    let trimmed = name.trim();
    let colon_count = trimmed.matches(':').count();
    if colon_count != 1 {
        return ParsedTag {
            namespace: None,
            value: trimmed.to_string(),
        };
    }
    let (ns, value) = trimmed.split_once(':').unwrap();
    let ns = ns.trim();
    let value = value.trim();
    if ns.is_empty() || value.is_empty() {
        return ParsedTag {
            namespace: None,
            value: trimmed.to_string(),
        };
    }
    ParsedTag {
        namespace: Some(ns.to_string()),
        value: value.to_string(),
    }
}

/// Construct a tag name from an optional namespace and a value. Whitespace
/// is trimmed from both sides. Caller is responsible for having validated
/// the inputs — this is a formatter, not a validator.
pub fn format_tag(namespace: Option<&str>, value: &str) -> String {
    let value = value.trim();
    match namespace.map(str::trim).filter(|s| !s.is_empty()) {
        Some(ns) => format!("{ns}:{value}"),
        None => value.to_string(),
    }
}

/// Validate a tag name under the hierarchical tag rules.
pub fn validate_tag_name(name: &str) -> Result<(), TagNameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(TagNameError::Empty);
    }
    let colon_count = trimmed.matches(':').count();
    if colon_count == 0 {
        return Ok(());
    }
    if colon_count > 1 {
        return Err(TagNameError::MultipleColons);
    }
    let (ns, value) = trimmed.split_once(':').unwrap();
    let ns = ns.trim();
    let value = value.trim();
    if ns.is_empty() {
        return Err(TagNameError::EmptyNamespace);
    }
    if value.is_empty() {
        return Err(TagNameError::EmptyValue);
    }
    if !NAMESPACE_RE.is_match(ns) {
        return Err(TagNameError::InvalidNamespace);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_flat_name() {
        let p = parse_tag("dinner");
        assert_eq!(p.namespace, None);
        assert_eq!(p.value, "dinner");
    }

    #[test]
    fn parse_hierarchical() {
        let p = parse_tag("ingredient:chicken");
        assert_eq!(p.namespace, Some("ingredient".to_string()));
        assert_eq!(p.value, "chicken");
    }

    #[test]
    fn parse_multi_colon_stays_flat() {
        let p = parse_tag("a:b:c");
        assert_eq!(p.namespace, None);
        assert_eq!(p.value, "a:b:c");
    }

    #[test]
    fn parse_trims_whitespace() {
        let p = parse_tag("  course : breakfast  ");
        assert_eq!(p.namespace, Some("course".to_string()));
        assert_eq!(p.value, "breakfast");
    }

    #[test]
    fn parse_empty_side_is_flat() {
        assert_eq!(parse_tag(":foo").namespace, None);
        assert_eq!(parse_tag("foo:").namespace, None);
    }

    #[test]
    fn format_without_namespace() {
        assert_eq!(format_tag(None, "dinner"), "dinner");
        assert_eq!(format_tag(Some(""), "dinner"), "dinner");
    }

    #[test]
    fn format_with_namespace() {
        assert_eq!(format_tag(Some("course"), "breakfast"), "course:breakfast");
    }

    #[test]
    fn validate_empty_rejected() {
        assert_eq!(validate_tag_name(""), Err(TagNameError::Empty));
        assert_eq!(validate_tag_name("   "), Err(TagNameError::Empty));
    }

    #[test]
    fn validate_flat_ok() {
        assert!(validate_tag_name("dinner").is_ok());
        assert!(validate_tag_name("Quick Weeknight").is_ok());
    }

    #[test]
    fn validate_hierarchical_ok() {
        assert!(validate_tag_name("ingredient:chicken").is_ok());
        assert!(validate_tag_name("course:breakfast").is_ok());
        assert!(validate_tag_name("diet:gluten-free").is_ok());
    }

    #[test]
    fn validate_multi_colon_rejected() {
        assert_eq!(
            validate_tag_name("a:b:c"),
            Err(TagNameError::MultipleColons)
        );
    }

    #[test]
    fn validate_empty_sides_rejected() {
        assert_eq!(
            validate_tag_name(":chicken"),
            Err(TagNameError::EmptyNamespace)
        );
        assert_eq!(
            validate_tag_name("ingredient:"),
            Err(TagNameError::EmptyValue)
        );
    }

    #[test]
    fn validate_namespace_charset() {
        assert_eq!(
            validate_tag_name("Course:breakfast"),
            Err(TagNameError::InvalidNamespace)
        );
        assert_eq!(
            validate_tag_name("1course:breakfast"),
            Err(TagNameError::InvalidNamespace)
        );
        assert_eq!(
            validate_tag_name("course!:breakfast"),
            Err(TagNameError::InvalidNamespace)
        );
    }
}
```

- [ ] **Step 2: Wire the module into `ramekin-core/src/lib.rs`**

Insert `pub mod tags;` alphabetically in the module list at the top of `ramekin-core/src/lib.rs` (between `pub mod pipeline;` and `pub mod types;`), and add the re-export line after the existing `pub use types::{…};` block:

```rust
pub use tags::{
    format_tag, parse_tag, validate_tag_name, ParsedTag, TagNameError, SEEDED_NAMESPACES,
};
```

- [ ] **Step 3: Verify `regex` is already in `ramekin-core/Cargo.toml`**

Run: `grep -E '^regex' ramekin-core/Cargo.toml`

Expected: `regex = "1"` (already present). `LazyLock` lives in std since Rust 1.80 — no extra dependency needed (toolchain is 1.92.0).

- [ ] **Step 4: Run the test suite and confirm the new tests pass**

Run: `make test`

Expected: the `rust-tests-core` process-compose job passes and prints `test tags::tests::…` lines for each of the 12 new tests.

- [ ] **Step 5: Commit**

```bash
git add ramekin-core/src/tags.rs ramekin-core/src/lib.rs
git commit -m "Add hierarchical tag parse/format/validate helpers"
```

---

## Task 2: Server-side validation on create/rename

**Files:**
- Modify: `server/src/api/tags/create.rs`
- Modify: `server/src/api/tags/rename.rs`

- [ ] **Step 1: Write failing API tests first**

Append to `tests/test_tags.py`:

```python
def test_create_tag_hierarchical(authed_api_client):
    """Tags with a namespace:value shape are accepted."""
    client, _ = authed_api_client
    tags_api = TagsApi(client)

    response = tags_api.create_tag(CreateTagRequest(name="ingredient:chicken"))
    assert response.name == "ingredient:chicken"


def test_create_tag_multi_colon_rejected(authed_api_client):
    client, _ = authed_api_client
    tags_api = TagsApi(client)

    with pytest.raises(ApiException) as exc_info:
        tags_api.create_tag(CreateTagRequest(name="a:b:c"))
    assert exc_info.value.status == 400


def test_create_tag_invalid_namespace_rejected(authed_api_client):
    client, _ = authed_api_client
    tags_api = TagsApi(client)

    with pytest.raises(ApiException) as exc_info:
        tags_api.create_tag(CreateTagRequest(name="Course:breakfast"))
    assert exc_info.value.status == 400


def test_create_tag_empty_sides_rejected(authed_api_client):
    client, _ = authed_api_client
    tags_api = TagsApi(client)

    for bad in (":chicken", "ingredient:"):
        with pytest.raises(ApiException) as exc_info:
            tags_api.create_tag(CreateTagRequest(name=bad))
        assert exc_info.value.status == 400


def test_rename_tag_validates(authed_api_client):
    client, _ = authed_api_client
    tags_api = TagsApi(client)

    created = tags_api.create_tag(CreateTagRequest(name="dinner"))
    with pytest.raises(ApiException) as exc_info:
        tags_api.rename_tag(created.id, RenameTagRequest(name="a:b:c"))
    assert exc_info.value.status == 400
```

- [ ] **Step 2: Run the new tests to verify they fail**

Run: `make test`

Expected: the five new tests fail with `AssertionError` on the status code (currently `a:b:c` etc. would be accepted).

- [ ] **Step 3: Replace the empty-name check in `server/src/api/tags/create.rs`**

In `server/src/api/tags/create.rs`, replace the current block:

```rust
    let name = request.name.trim();

    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Tag name cannot be empty".to_string(),
            }),
        )
            .into_response();
    }
```

with:

```rust
    let name = request.name.trim();

    if let Err(err) = ramekin_core::validate_tag_name(name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: err.message().to_string(),
            }),
        )
            .into_response();
    }
```

- [ ] **Step 4: Do the same swap in `server/src/api/tags/rename.rs`**

Replace the block at lines 54-64 (the `new_name` empty check) with:

```rust
    let new_name = request.name.trim();

    if let Err(err) = ramekin_core::validate_tag_name(new_name) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: err.message().to_string(),
            }),
        )
            .into_response();
    }
```

- [ ] **Step 5: Run tests and verify they pass**

Run: `make test`

Expected: `test_create_tag_hierarchical`, `test_create_tag_multi_colon_rejected`, `test_create_tag_invalid_namespace_rejected`, `test_create_tag_empty_sides_rejected`, `test_rename_tag_validates` all pass. Existing tag tests (including the empty-name and duplicate tests) still pass.

- [ ] **Step 6: Commit**

```bash
git add server/src/api/tags/create.rs server/src/api/tags/rename.rs tests/test_tags.py
git commit -m "Validate hierarchical tag names on create/rename"
```

---

## Task 3: Expose `namespace` + `value` on `GET /tags`

**Files:**
- Modify: `server/src/api/tags/list.rs`
- Modify: `tests/test_tags.py`

- [ ] **Step 1: Add a failing test that expects the new fields**

Append to `tests/test_tags.py`:

```python
def test_list_tags_exposes_namespace_and_value(authed_api_client):
    client, _ = authed_api_client
    tags_api = TagsApi(client)

    tags_api.create_tag(CreateTagRequest(name="ingredient:chicken"))
    tags_api.create_tag(CreateTagRequest(name="dinner"))

    response = tags_api.list_all_tags()
    by_name = {t.name: t for t in response.tags}

    assert by_name["ingredient:chicken"].namespace == "ingredient"
    assert by_name["ingredient:chicken"].value == "chicken"
    assert by_name["dinner"].namespace is None
    assert by_name["dinner"].value == "dinner"
```

- [ ] **Step 2: Run the test and verify it fails (attribute missing on generated model)**

Run: `make test`

Expected: `AttributeError: 'TagItem' has no attribute 'namespace'` (or a similar generated-client error).

- [ ] **Step 3: Modify `server/src/api/tags/list.rs` to add derived fields**

Replace the `TagItem` struct with:

```rust
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TagItem {
    pub id: Uuid,
    pub name: String,
    /// Namespace portion for `namespace:value`-shaped names, else null.
    pub namespace: Option<String>,
    /// Value portion. Equals `name` for flat tags.
    pub value: String,
    pub created_at: DateTime<Utc>,
    /// Number of recipes using this tag
    pub recipe_count: i64,
}
```

Replace the `map` call at the bottom (currently `|(id, name, created_at, recipe_count)| TagItem { id, name, created_at, recipe_count }`) with:

```rust
            .map(|(id, name, created_at, recipe_count)| {
                let parsed = ramekin_core::parse_tag(&name);
                TagItem {
                    id,
                    namespace: parsed.namespace,
                    value: parsed.value,
                    name,
                    created_at,
                    recipe_count,
                }
            })
```

- [ ] **Step 4: Regenerate the OpenAPI spec and clients**

Run: `make clean-api && make api/openapi.json`

Expected: `api/openapi.json` is regenerated and `cli/generated/ramekin-client/` is rebuilt. The diff on `api/openapi.json` shows `namespace` and `value` added to the `TagItem` schema.

- [ ] **Step 5: Run the test and verify it passes**

Run: `make test`

Expected: `test_list_tags_exposes_namespace_and_value` passes. All previous tag tests still pass.

- [ ] **Step 6: Commit**

```bash
git add server/src/api/tags/list.rs tests/test_tags.py api/openapi.json cli/generated ramekin-ui/generated-client
git commit -m "Expose namespace and value on GET /tags"
```

---

## Task 4: Update the auto-tag prompt

**Files:**
- Modify: `ramekin-core/src/ai/prompts/auto_tag.rs`

- [ ] **Step 1: Update the prompt body**

In `ramekin-core/src/ai/prompts/auto_tag.rs`, replace the format-string line:

```rust
        r#"You are a recipe tagging assistant. Given a recipe and the user's existing tags, suggest which tags from their list would apply to this recipe.

IMPORTANT: Only suggest tags from the provided list. Never create new tags.
```

with:

```rust
        r#"You are a recipe tagging assistant. Given a recipe and the user's existing tags, suggest which tags from their list would apply to this recipe.

IMPORTANT: Only suggest tags from the provided list. Never create new tags.

Note: some tags use a `namespace:value` form (for example `ingredient:chicken`, `course:breakfast`). Treat the full string as the tag identifier — match the whole thing, do not split or strip the namespace prefix.
```

(The tests below already assert the existing suggestions still flow through, because filtering is done server-side on exact name.)

- [ ] **Step 2: Run the test suite**

Run: `make test`

Expected: existing auto-tag tests in `ramekin-core/src/ai/prompts/auto_tag.rs` (the `test_render_prompt` case) still pass.

- [ ] **Step 3: Commit**

```bash
git add ramekin-core/src/ai/prompts/auto_tag.rs
git commit -m "Teach auto-tag prompt about namespace:value tags"
```

---

## Task 5: UI tag-hierarchy utility

**Files:**
- Create: `ramekin-ui/src/utils/tagHierarchy.ts`

- [ ] **Step 1: Create the utility**

```typescript
// Mirrors ramekin-core/src/tags.rs. Keep in sync manually — the list is
// short and duplication is simpler than threading a generated constant
// through the client.

export const SEEDED_NAMESPACES = [
  "ingredient",
  "course",
  "cuisine",
  "diet",
  "method",
  "season",
] as const;

export type SeededNamespace = (typeof SEEDED_NAMESPACES)[number];

export interface ParsedTag {
  namespace: string | null;
  value: string;
}

const NAMESPACE_RE = /^[a-z][a-z0-9_-]*$/;

export function parseTag(name: string): ParsedTag {
  const trimmed = name.trim();
  const parts = trimmed.split(":");
  if (parts.length !== 2) {
    return { namespace: null, value: trimmed };
  }
  const ns = parts[0].trim();
  const value = parts[1].trim();
  if (!ns || !value) {
    return { namespace: null, value: trimmed };
  }
  return { namespace: ns, value };
}

export function formatTag(namespace: string | null, value: string): string {
  const v = value.trim();
  const ns = namespace?.trim() ?? "";
  return ns ? `${ns}:${v}` : v;
}

export function isValidNamespace(namespace: string): boolean {
  return NAMESPACE_RE.test(namespace);
}

export interface NamespaceGroup {
  namespace: string | null; // null == "Uncategorized"
  tags: string[]; // raw tag names, e.g. "ingredient:chicken" or "dinner"
}

/**
 * Group tag names by namespace. Order:
 *   1. Seeded namespaces (even if empty, if `includeEmptySeeded`).
 *   2. Other user namespaces, alphabetically.
 *   3. Uncategorized, last.
 */
export function groupTags(
  names: string[],
  { includeEmptySeeded = false }: { includeEmptySeeded?: boolean } = {},
): NamespaceGroup[] {
  const buckets = new Map<string | null, string[]>();
  for (const name of names) {
    const { namespace } = parseTag(name);
    const key = namespace;
    if (!buckets.has(key)) buckets.set(key, []);
    buckets.get(key)!.push(name);
  }

  const seeded = SEEDED_NAMESPACES.filter(
    (ns) => includeEmptySeeded || buckets.has(ns),
  );
  const seededSet = new Set<string>(seeded);
  const extras = [...buckets.keys()]
    .filter((k): k is string => k !== null && !seededSet.has(k))
    .sort((a, b) => a.localeCompare(b));

  const groups: NamespaceGroup[] = [];
  for (const ns of seeded) {
    groups.push({ namespace: ns, tags: (buckets.get(ns) ?? []).slice().sort() });
  }
  for (const ns of extras) {
    groups.push({ namespace: ns, tags: (buckets.get(ns) ?? []).slice().sort() });
  }
  if (buckets.has(null)) {
    groups.push({ namespace: null, tags: buckets.get(null)!.slice().sort() });
  }
  return groups;
}

export function knownNamespaces(names: string[]): string[] {
  const found = new Set<string>(SEEDED_NAMESPACES);
  for (const n of names) {
    const p = parseTag(n);
    if (p.namespace) found.add(p.namespace);
  }
  return [...found].sort();
}
```

- [ ] **Step 2: Run the TypeScript type check**

Run: `make lint`

Expected: no type errors in the new file.

- [ ] **Step 3: Commit**

```bash
git add ramekin-ui/src/utils/tagHierarchy.ts
git commit -m "Add ramekin-ui tagHierarchy utility"
```

---

## Task 6: Grouped available-tag list in `TagInput`

**Files:**
- Modify: `ramekin-ui/src/components/TagInput.tsx`

- [ ] **Step 1: Replace the available-tag rendering with a grouped layout and add namespace dropdown behavior**

Replace the entire contents of `ramekin-ui/src/components/TagInput.tsx` with:

```tsx
import { createSignal, For, Show } from "solid-js";
import { useAuth } from "../context/AuthContext";
import {
  formatTag,
  groupTags,
  isValidNamespace,
  knownNamespaces,
  parseTag,
} from "../utils/tagHierarchy";

interface TagInputProps {
  tags: () => string[];
  onTagsChange: (tags: string[]) => void;
  placeholder?: string;
  id?: string;
}

const NONE = "";
const NEW_NS = "__new__";

export default function TagInput(props: TagInputProps) {
  const { tags: availableTags } = useAuth();

  let inputRef: HTMLInputElement | undefined;

  const [inputValue, setInputValue] = createSignal("");
  const [namespace, setNamespace] = createSignal<string>(NONE);
  const [newNsValue, setNewNsValue] = createSignal("");
  const [newNsError, setNewNsError] = createSignal<string | null>(null);

  const namespaces = () => knownNamespaces(availableTags());

  const unselectedGroups = () => {
    const input = inputValue().toLowerCase().trim();
    const selected = new Set(props.tags());
    const available = availableTags().filter((t) => !selected.has(t));
    const filtered = input
      ? available.filter((t) => t.toLowerCase().includes(input))
      : available;
    return groupTags(filtered);
  };

  const effectiveTagName = (raw: string): string => {
    const trimmed = raw.trim();
    if (!trimmed) return "";
    // If the user typed a colon, honor it as an inline namespace override.
    if (trimmed.includes(":")) return trimmed;
    const ns = namespace();
    if (!ns || ns === NEW_NS) return trimmed;
    return formatTag(ns, trimmed);
  };

  const showCreateOption = () => {
    const name = effectiveTagName(inputValue());
    if (!name) return false;
    return !availableTags().some(
      (t) => t.toLowerCase() === name.toLowerCase(),
    );
  };

  const addTag = (tagName: string) => {
    const normalized = tagName.trim();
    if (normalized && !props.tags().includes(normalized)) {
      props.onTagsChange([...props.tags(), normalized]);
    }
    setInputValue("");
  };

  const removeTag = (tagToRemove: string) => {
    props.onTagsChange(props.tags().filter((t) => t !== tagToRemove));
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    const input = inputValue().trim();
    switch (e.key) {
      case "Enter":
      case ",":
        e.preventDefault();
        if (input) addTag(effectiveTagName(input));
        break;
      case "Backspace":
        if (!inputValue() && props.tags().length > 0) {
          removeTag(props.tags()[props.tags().length - 1]);
        }
        break;
    }
  };

  const acceptNewNamespace = () => {
    const ns = newNsValue().trim();
    if (!ns) {
      setNewNsError("Namespace cannot be empty");
      return;
    }
    if (!isValidNamespace(ns)) {
      setNewNsError("Use lowercase letters, digits, - or _, starting with a letter");
      return;
    }
    setNewNsError(null);
    setNamespace(ns);
    setNewNsValue("");
  };

  const focusInput = () => inputRef?.focus();

  return (
    <div class="tag-input-container">
      <Show when={props.tags().length > 0}>
        <div class="tag-selected-list">
          <For each={props.tags()}>
            {(tag) => {
              const parsed = parseTag(tag);
              return (
                <span class="tag-chip tag-chip-selected">
                  <Show when={parsed.namespace}>
                    <span class="tag-chip-ns">{parsed.namespace}:</span>
                  </Show>
                  {parsed.value}
                  <button
                    type="button"
                    class="tag-chip-remove"
                    onClick={() => removeTag(tag)}
                    aria-label={`Remove ${tag}`}
                  >
                    &times;
                  </button>
                </span>
              );
            }}
          </For>
        </div>
      </Show>

      <div class="tag-input-wrapper" onClick={focusInput}>
        <select
          class="tag-input-namespace"
          value={namespace()}
          onChange={(e) => {
            const val = e.currentTarget.value;
            setNamespace(val);
            if (val !== NEW_NS) setNewNsValue("");
          }}
          aria-label="Tag namespace"
        >
          <option value={NONE}>(none)</option>
          <For each={namespaces()}>
            {(ns) => <option value={ns}>{ns}</option>}
          </For>
          <option value={NEW_NS}>+ New namespace…</option>
        </select>
        <input
          ref={inputRef}
          type="text"
          class="tag-input-field"
          id={props.id}
          value={inputValue()}
          onInput={(e) => setInputValue(e.currentTarget.value)}
          onKeyDown={handleKeyDown}
          placeholder={props.placeholder ?? "Type to create new tag..."}
        />
        <Show when={showCreateOption()}>
          <button
            type="button"
            class="tag-create-btn"
            onMouseDown={() => addTag(effectiveTagName(inputValue()))}
          >
            + Create "{effectiveTagName(inputValue())}"
          </button>
        </Show>
      </div>

      <Show when={namespace() === NEW_NS}>
        <div class="tag-input-new-ns">
          <input
            type="text"
            value={newNsValue()}
            placeholder="new-namespace"
            onInput={(e) => setNewNsValue(e.currentTarget.value)}
          />
          <button type="button" onMouseDown={acceptNewNamespace}>
            Add namespace
          </button>
          <Show when={newNsError()}>
            <div class="tag-input-error">{newNsError()}</div>
          </Show>
        </div>
      </Show>

      <Show when={unselectedGroups().length > 0}>
        <div class="tag-available-list">
          <For each={unselectedGroups()}>
            {(group) => (
              <div class="tag-available-group">
                <div class="tag-available-group-label">
                  {group.namespace ?? "Uncategorized"}
                </div>
                <div class="tag-available-group-chips">
                  <For each={group.tags}>
                    {(tag) => {
                      const parsed = parseTag(tag);
                      return (
                        <button
                          type="button"
                          class="tag-chip tag-chip-available"
                          onClick={() => addTag(tag)}
                        >
                          <Show when={parsed.namespace}>
                            <span class="tag-chip-ns">{parsed.namespace}:</span>
                          </Show>
                          {parsed.value}
                        </button>
                      );
                    }}
                  </For>
                </div>
              </div>
            )}
          </For>
        </div>
      </Show>
    </div>
  );
}
```

- [ ] **Step 2: Run the linter**

Run: `make lint`

Expected: no TypeScript or eslint errors.

- [ ] **Step 3: Smoke-test manually in the browser**

Run the dev server: `make dev` (in a separate terminal). Navigate to the create-recipe page and verify:
- The namespace dropdown appears with `(none)`, seeded namespaces, and `+ New namespace…`.
- Selecting `course` and typing `breakfast` then pressing Enter adds `course:breakfast` to the selected list.
- Typing `ingredient:chicken` directly (with the namespace dropdown on `(none)`) also adds `ingredient:chicken`.
- `+ New namespace…` shows the secondary input and rejects `Course` with an error.
- The available-tag grid is grouped by namespace with headers.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/components/TagInput.tsx
git commit -m "Group TagInput available list and add namespace dropdown"
```

---

## Task 7: Grouped tag filter in `CookbookPage`

**Files:**
- Modify: `ramekin-ui/src/pages/CookbookPage.tsx`

- [ ] **Step 1: Replace the Tags filter block**

In `ramekin-ui/src/pages/CookbookPage.tsx`, find the block that starts at line ~1025:

```tsx
            <div class="cookbook-filter-label">Tags</div>
            <Show
              when={sortedTags().length > 0}
              fallback={<span class="cookbook-filter-empty">No tags yet</span>}
            >
              <div class="cookbook-filter-chips">
                <For each={sortedTags()}>
                  {(tag) => (
                    <button
                      ...
                      onClick={() => toggleTag(tag)}
                    >
                      {tag}
                    </button>
                  )}
                </For>
              </div>
            </Show>
```

Replace the `<For each={sortedTags()}>` body with a grouped rendering. First, above the return statement, add:

```tsx
import { groupTags, parseTag } from "../utils/tagHierarchy";

// inside the component, near sortedTags():
const groupedTags = () => groupTags(sortedTags());
```

(Remove the existing `sortedTags` re-use inside the Tags block — keep the `sortedTags` memo; `groupedTags` reads from it implicitly.)

Then replace the `<div class="cookbook-filter-chips">` block inside the Tags `<Show>` with:

```tsx
              <div class="cookbook-filter-tag-groups">
                <For each={groupedTags()}>
                  {(group) => (
                    <details class="cookbook-filter-tag-group" open>
                      <summary>
                        {group.namespace ?? "Uncategorized"}
                        <span class="cookbook-filter-tag-group-count">
                          {" "}
                          ({group.tags.length})
                        </span>
                      </summary>
                      <div class="cookbook-filter-chips">
                        <For each={group.tags}>
                          {(tag) => {
                            const parsed = parseTag(tag);
                            return (
                              <button
                                type="button"
                                class="cookbook-filter-chip"
                                classList={{
                                  "cookbook-filter-chip-active":
                                    currentFilters().tags.includes(tag),
                                }}
                                aria-pressed={currentFilters().tags.includes(tag)}
                                onClick={() => toggleTag(tag)}
                              >
                                <Show when={parsed.namespace}>
                                  <span class="tag-chip-ns">
                                    {parsed.namespace}:
                                  </span>
                                </Show>
                                {parsed.value}
                              </button>
                            );
                          }}
                        </For>
                      </div>
                    </details>
                  )}
                </For>
              </div>
```

- [ ] **Step 2: Run the linter**

Run: `make lint`

Expected: no errors.

- [ ] **Step 3: Smoke-test in the browser**

With `make dev` running, open the cookbook page. Verify:
- Tag filter section shows collapsible subsections per namespace.
- Selecting `course:breakfast` then `ingredient:chicken` AND'd filters recipes as before.
- The URL still contains `tag:"course:breakfast"` and `tag:"ingredient:chicken"`.
- Flat tags appear under "Uncategorized".

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/pages/CookbookPage.tsx
git commit -m "Group cookbook tag filter by namespace"
```

---

## Task 8: Grouped `TagsPage`

**Files:**
- Modify: `ramekin-ui/src/pages/TagsPage.tsx`

- [ ] **Step 1: Add the import**

Change the first import line from:

```tsx
import { createSignal, createEffect, For, Show } from "solid-js";
```

(and add, below the existing imports):

```tsx
import { groupTags, parseTag } from "../utils/tagHierarchy";
```

- [ ] **Step 2: Add a grouped signal derived from `tags()`**

In `TagsPage`, after the `createSignal<TagItem[]>` declaration, add:

```tsx
const groupedTags = () => {
  const byName = new Map(tags().map((t) => [t.name, t] as const));
  return groupTags(tags().map((t) => t.name)).map((group) => ({
    namespace: group.namespace,
    items: group.tags
      .map((name) => byName.get(name)!)
      .filter(Boolean),
  }));
};
```

- [ ] **Step 3: Replace the `<div class="tags-list">` block**

Replace the block (lines ~140-207, inside `<Show when={!loading() && tags().length > 0}>`) currently reading:

```tsx
        <div class="tags-list">
          <For each={tags()}>
            {(tag) => (
              <div class="tag-row">
                <Show
                  when={editingId() === tag.id}
                  fallback={
                    <>
                      <span
                        class="tag-name"
                        onClick={() => navigateToFiltered(tag.name)}
                        title="Click to view recipes with this tag"
                      >
                        {tag.name}
                      </span>
                      ... (existing count + actions markup)
                    </>
                  }
                >
                  ... (existing edit markup)
                </Show>
              </div>
            )}
          </For>
        </div>
```

with:

```tsx
        <div class="tags-list">
          <For each={groupedTags()}>
            {(group) => (
              <section class="tags-group">
                <h3 class="tags-group-label">
                  {group.namespace ?? "Uncategorized"}
                </h3>
                <For each={group.items}>
                  {(tag) => {
                    const parsed = parseTag(tag.name);
                    return (
                      <div class="tag-row">
                        <Show
                          when={editingId() === tag.id}
                          fallback={
                            <>
                              <span
                                class="tag-name"
                                onClick={() => navigateToFiltered(tag.name)}
                                title="Click to view recipes with this tag"
                              >
                                <Show when={parsed.namespace}>
                                  <span class="tag-chip-ns">
                                    {parsed.namespace}:
                                  </span>
                                </Show>
                                {parsed.value}
                              </span>
                              <span class="tag-count">
                                {tag.recipeCount}{" "}
                                {tag.recipeCount === 1 ? "recipe" : "recipes"}
                              </span>
                              <div class="tag-actions">
                                <button
                                  class="btn btn-small"
                                  onClick={() => startEditing(tag)}
                                >
                                  Rename
                                </button>
                                <button
                                  class="btn btn-small btn-danger"
                                  onClick={() => confirmDelete(tag)}
                                >
                                  Delete
                                </button>
                              </div>
                            </>
                          }
                        >
                          <input
                            type="text"
                            class="tag-edit-input"
                            value={editName()}
                            onInput={(e) => setEditName(e.currentTarget.value)}
                            onKeyDown={(e) => handleKeyDown(e, tag.id)}
                            autofocus
                          />
                          <Show when={editError()}>
                            <span class="edit-error">{editError()}</span>
                          </Show>
                          <div class="tag-actions">
                            <button
                              class="btn btn-small btn-primary"
                              onClick={() => handleRename(tag.id)}
                              disabled={saving()}
                            >
                              {saving() ? "Saving..." : "Save"}
                            </button>
                            <button
                              class="btn btn-small"
                              onClick={cancelEditing}
                              disabled={saving()}
                            >
                              Cancel
                            </button>
                          </div>
                        </Show>
                      </div>
                    );
                  }}
                </For>
              </section>
            )}
          </For>
        </div>
```

- [ ] **Step 4: Run the linter**

Run: `make lint`

Expected: no errors.

- [ ] **Step 5: Smoke-test in the browser**

With `make dev` running, open `/tags`. Verify:
- Tags are grouped by namespace, matching the cookbook-filter ordering (seeded first, then extras alphabetically, then Uncategorized).
- Rename on `ingredient:chicken` → `chicken` moves it to Uncategorized after the in-component `loadTags()` refresh completes.
- Delete still works.

- [ ] **Step 6: Commit**

```bash
git add ramekin-ui/src/pages/TagsPage.tsx
git commit -m "Group TagsPage by namespace"
```

---

## Task 9: De-emphasize namespace prefix on recipe-card tag chips

**Files:**
- Modify: `ramekin-ui/src/pages/CookbookPage.tsx`
- Modify: `ramekin-ui/src/pages/ViewRecipePage.tsx`

- [ ] **Step 1: CookbookPage recipe cards**

Find the `<Show when={recipe.tags && recipe.tags.length > 0}>` block around line ~1215. Replace the inner tag loop to split namespace out visually:

```tsx
<For each={recipe.tags}>
  {(tag) => {
    const parsed = parseTag(tag);
    return (
      <span class="recipe-card-tag">
        <Show when={parsed.namespace}>
          <span class="tag-chip-ns">{parsed.namespace}:</span>
        </Show>
        {parsed.value}
      </span>
    );
  }}
</For>
```

- [ ] **Step 2: ViewRecipePage**

Apply the same structural change wherever `recipe.tags` is mapped in `ramekin-ui/src/pages/ViewRecipePage.tsx` — wrap the namespace portion in a `<span class="tag-chip-ns">` and render only the value outside it.

- [ ] **Step 3: Run the linter**

Run: `make lint`

Expected: no errors.

- [ ] **Step 4: Commit**

```bash
git add ramekin-ui/src/pages/CookbookPage.tsx ramekin-ui/src/pages/ViewRecipePage.tsx
git commit -m "Split namespace prefix in recipe-card tag chips"
```

*(Final visual styling — color, size, weight of `.tag-chip-ns` — is left to a later `frontend-design` pass. This task only wires the structure.)*

---

## Task 10: UI smoke test for grouped filter

**Files:**
- Modify: `tests/ui/test_smoke.py`

- [ ] **Step 1: Append a grouped-filter test**

Add to `tests/ui/test_smoke.py`:

```python
def test_cookbook_grouped_tag_filter(page: Page, api_url: str):
    """Tags filter panel groups tags by namespace and filters correctly."""
    username = f"ui_tags_{uuid.uuid4().hex[:8]}"
    password = "testpass123"
    config = Configuration(host=api_url)

    with ApiClient(config) as client:
        auth_api = AuthApi(client)
        signup = auth_api.signup(SignupRequest(username=username, password=password))

    authed_config = Configuration(host=api_url)
    authed_config.access_token = signup.token

    with ApiClient(authed_config) as client:
        recipes_api = RecipesApi(client)
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Chicken dish",
                instructions="cook it",
                ingredients=[
                    Ingredient(item="chicken",
                               measurements=[Measurement(amount="1", unit="lb")])
                ],
                tags=["ingredient:chicken", "course:dinner"],
            )
        )
        recipes_api.create_recipe(
            CreateRecipeRequest(
                title="Morning oats",
                instructions="cook them",
                ingredients=[
                    Ingredient(item="oats",
                               measurements=[Measurement(amount="1", unit="cup")])
                ],
                tags=["course:breakfast", "quick"],
            )
        )

    # Log in via UI
    page.goto(f"{os.environ['UI_BASE_URL']}/login")
    page.fill("input[name='username']", username)
    page.fill("input[name='password']", password)
    page.click("button[type='submit']")
    expect(page).to_have_url(re.compile(r"/cookbook"))

    # Grouped sections are rendered.
    expect(page.locator(".cookbook-filter-tag-group summary",
                        has_text="course")).to_be_visible()
    expect(page.locator(".cookbook-filter-tag-group summary",
                        has_text="ingredient")).to_be_visible()
    expect(page.locator(".cookbook-filter-tag-group summary",
                        has_text="Uncategorized")).to_be_visible()

    # Click ingredient:chicken — only the chicken recipe remains.
    page.get_by_role("button", name="chicken").click()
    expect(page.get_by_text("Chicken dish")).to_be_visible()
    expect(page.get_by_text("Morning oats")).not_to_be_visible()
```

- [ ] **Step 2: Run UI tests**

Run: `make test-ui`

Expected: `test_cookbook_grouped_tag_filter` passes along with existing smoke tests.

- [ ] **Step 3: Commit**

```bash
git add tests/ui/test_smoke.py
git commit -m "Add UI smoke test for grouped tag filter"
```

---

## Task 11: Follow-up issues

**Files:**
- Create: `issues/p3-ai-recategorize-tags.json5`
- Create: `issues/p3-ios-hierarchical-tags-ui.json5`

- [ ] **Step 1: Check the `issues/` directory exists**

Run: `ls issues/ 2>/dev/null || mkdir issues`

Expected: directory exists (create it if missing — it may not exist on this branch yet).

- [ ] **Step 2: Create the AI recategorize issue**

`issues/p3-ai-recategorize-tags.json5`:

```json5
{
  "title": "Pipeline step to recategorize uncategorized tags via AI",
  "description": "After hierarchical tags landed, users still have flat tags like `breakfast` or `chicken`. Add a pipeline step (or one-shot job) that, for a given user, proposes namespace assignments for their uncategorized tags (e.g., `breakfast` → `course:breakfast`). Semantics TBD: most likely a dry-run that writes proposals to a review queue and a user-approval UI that applies the renames in batch. Must not silently mutate user data. See docs/superpowers/specs/2026-04-18-hierarchical-tags-design.md.",
  "status": "open",
  "priority": 3,
  "type": "task",
  "labels": ["tags", "ai"],
  "created_at": "2026-04-18T00:00:00-07:00",
  "updated_at": "2026-04-18T00:00:00-07:00"
}
```

- [ ] **Step 3: Create the iOS parity issue**

`issues/p3-ios-hierarchical-tags-ui.json5`:

```json5
{
  "title": "iOS: hierarchical tag UI parity with web",
  "description": "v1 of hierarchical tags is web-only. Bring iOS to parity: grouped tag picker (namespace dropdown + value), grouped filter UI in the cookbook, and visually de-emphasized namespace prefix on tag chips. See docs/superpowers/specs/2026-04-18-hierarchical-tags-design.md §4.",
  "status": "open",
  "priority": 3,
  "type": "task",
  "labels": ["tags", "ios"],
  "created_at": "2026-04-18T00:00:00-07:00",
  "updated_at": "2026-04-18T00:00:00-07:00"
}
```

- [ ] **Step 4: Commit**

```bash
git add issues/p3-ai-recategorize-tags.json5 issues/p3-ios-hierarchical-tags-ui.json5
git commit -m "File follow-up issues for hierarchical tags (AI recategorize, iOS parity)"
```

---

## Task 12: Final lint + full test pass

- [ ] **Step 1: Run the linter**

Run: `make lint`

Expected: clean.

- [ ] **Step 2: Run the full API + Rust test suite**

Run: `make test`

Expected: all green.

- [ ] **Step 3: Run UI tests**

Run: `make test-ui`

Expected: all green.

- [ ] **Step 4: If any step failed, fix and amend the relevant earlier task's commit — do NOT push through a broken state.**

---

## Out of scope (explicitly)

- Schema migrations for tags.
- Automatic AI recategorization of existing flat tags (filed as issue).
- iOS feature parity (filed as issue).
- Namespace metadata (colors, icons, sort order).
- Search-token syntax changes (the existing `tag:"ns:value"` quoted form is sufficient).
- Full visual styling pass on `.tag-chip-ns` — that's a later `frontend-design` task.
