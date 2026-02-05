# Issues

Issues are stored as individual JSON files in the `issues/` directory. The filename serves as the issue ID (e.g., `decimal-amounts-not-converted-to-fractions.json`).

Closed issues should be deleted, not marked as closed.

## Format

```json
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

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Short summary |
| `description` | string | Full description with context |
| `status` | string | Always "open" (delete closed issues) |
| `priority` | int | 1 (highest) to 4 (lowest) |
| `type` | string | Usually "task" |
| `labels` | string[] | Tags like "ingredient-parser", "upstream" |
| `created_at` | string | ISO 8601 timestamp |
| `updated_at` | string | ISO 8601 timestamp |

## Querying

### List all issues

```bash
ls issues/
```

### View an issue

```bash
jq . issues/decimal-amounts-not-converted-to-fractions.json
```

### List all issue titles with priority

```bash
for f in issues/*.json; do echo "$(basename "$f" .json): $(jq -r '[.priority, .title] | @tsv' "$f")"; done | sort -t$'\t' -k1 -n
```

### Find issues by label

```bash
for f in issues/*.json; do
  jq -e '.labels | index("upstream")' "$f" >/dev/null && basename "$f" .json
done
```

### Find high priority issues (priority 1-2)

```bash
for f in issues/*.json; do
  jq -e '.priority <= 2' "$f" >/dev/null && echo "$(basename "$f" .json): $(jq -r .title "$f")"
done
```

### Search descriptions

```bash
grep -l "upstream" issues/*.json
```
