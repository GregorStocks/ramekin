# Issues

Issues are stored as individual JSON5 files in the `issues/` directory. The filename serves as the issue ID and must start with `p1-`, `p2-`, `p3-`, `p4-`, or `blocked-` (for example `p3-decimal-amounts-not-converted-to-fractions.json5`).

Resolved issues should be deleted, not marked as resolved or closed.

## Format

```json5
{
  "title": "Decimal amounts not converted to fractions",
  "description": "Full description...",
  "status": "open",
  "priority": 3,
  "type": "task",
  "labels": ["ingredient-parser"],
  "created_at": "2026-02-03T13:00:23.491746-08:00",
  "updated_at": "2026-02-03T13:00:23.491746-08:00"
}
```

Use real timestamps, not placeholder times.

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Short summary |
| `description` | string | Full description with context |
| `status` | string | Always `"open"` |
| `priority` | int | 1 (highest) to 4 (lowest) |
| `type` | string | Usually `"task"` |
| `labels` | string[] | Tags like `"ingredient-parser"` or `"upstream"` |
| `created_at` | string | ISO 8601 timestamp |
| `updated_at` | string | ISO 8601 timestamp |
| `blocked` | bool \| string? | If truthy, the filename must start with `blocked-` and the issue is skipped by auto-claiming |

## Querying

If `agent-issues` is installed, prefer the shared CLI tools.

### List all issues with priority

```bash
issue-query
```

### Filter by label

```bash
issue-query --label upstream
```

### Show high priority issues (P1-P2)

```bash
issue-query --max-priority 2
```

### Search titles and descriptions

```bash
issue-query --search "ingredient"
```

### Claim an issue

```bash
issue-autoclaim
issue-autoclaim <issue-name>
issue-claim --current
```

### Lint issue files

```bash
issue-lint
```
