# Issues

Issues are stored as individual JSON files in the `issues/` directory.

## Format

Each file is named `{id}.json` and contains:

```json
{
  "id": "ramekin-1bh",
  "title": "Decimal amounts not converted to fractions",
  "description": "Full description...",
  "status": "open",
  "priority": 3,
  "type": "task",
  "labels": ["ingredient-parser"],
  "created_at": "2026-02-03T13:00:23.491746-08:00",
  "updated_at": "2026-02-03T13:00:23.491746-08:00",
  "closed_at": null
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (e.g., "ramekin-1bh") |
| `title` | string | Short summary |
| `description` | string | Full description with context |
| `status` | string | "open" or "closed" |
| `priority` | int | 1 (highest) to 4 (lowest) |
| `type` | string | Usually "task" |
| `labels` | string[] | Tags like "ingredient-parser", "upstream" |
| `created_at` | string | ISO 8601 timestamp |
| `updated_at` | string | ISO 8601 timestamp |
| `closed_at` | string\|null | ISO 8601 timestamp or null if open |

## Querying

### List all issues

```bash
ls issues/
```

### List open issues

```bash
grep -l '"status": "open"' issues/*.json
```

### List closed issues

```bash
grep -l '"status": "closed"' issues/*.json
```

### View an issue

```bash
jq . issues/ramekin-1bh.json
```

### List all issue titles with status

```bash
jq -r '[.id, .status, .title] | @tsv' issues/*.json
```

### Find issues by label

```bash
jq -r 'select(.labels | index("upstream")) | .id' issues/*.json
```

### Find high priority open issues (priority 1-2)

```bash
jq -r 'select(.status == "open" and .priority <= 2) | [.id, .priority, .title] | @tsv' issues/*.json
```

### Count by status

```bash
jq -s 'group_by(.status) | map({status: .[0].status, count: length})' issues/*.json
```

### Search descriptions

```bash
grep -l "upstream" issues/*.json
# Or with context:
grep -r "upstream" issues/
```
