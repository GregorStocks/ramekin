# Development Guide

## Quick Start

```bash
make dev     # Starts server, UI with hot-reload
```

The database is a SQLite file per worktree (`.db/dev.sqlite`, `.db/test.sqlite`,
paths in `dev.env` / `test.env`). The server creates and migrates it on startup. With
`RAMEKIN_BACKUP_DIR` set it also writes a daily snapshot there (see
doc/sqlite-cutover.md); `make db-backup` takes one on demand.

Edit code in `server/src/` and it auto-recompiles. Stop with `make dev-down`.

## Database Migrations

```bash
make new-migration NAME=add_widget_table   # Create migrations/<timestamp>_add_widget_table/up.sql
```

The server runs pending migrations at startup. `server/src/schema.rs` is
maintained by hand (Diesel can't tell UUID or JSON text from other TEXT
columns): update it alongside each migration, using `UuidText` for UUID columns
and `TimestamptzSqlite` for timestamps.

Create migration files through a Makefile-backed workflow. If no Makefile
target exists for the migration operation you need, add one before running the
tool directly. New migrations must preserve live data: use soft deletes and
additive/backfill-safe changes instead of destructively dropping data from live
tables. Historical migrations may predate this rule, but they are not a pattern
to copy.

## Client Regeneration

API clients regenerate automatically when server code changes. To force:

```bash
make clean-api && make test
```

## Tips

- Changes to Cargo.toml may require `make dev-down && make dev`
- Inspect the database with `sqlite3 .db/dev.sqlite` (the search index
  triggers need the server's `normalize_for_search` function, so write through
  the API, not the sqlite3 shell)
- Generated clients are checked in, so CLI builds without running server
