# Development Guide

## Quick Start

```bash
make db-up   # Start postgres (shared across environments, only needed once)
make dev     # Starts server, UI with hot-reload
```

Edit code in `server/src/` and it auto-recompiles. Stop with `make dev-down`.

## Database Migrations

```bash
make db-migrate        # Run pending migrations
make generate-schema   # Regenerate schema.rs after schema changes
```

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
- Database persists via Docker volume
- Generated clients are checked in, so CLI builds without running server
