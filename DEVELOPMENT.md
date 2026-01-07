# Development Guide

## Quick Start

```bash
make db-up   # Start postgres (shared across environments, only needed once)
make dev     # Starts server, UI with hot-reload
```

Edit code in `server/src/` and it auto-recompiles. Stop with `make dev-down`.

## Database Migrations

```bash
cd server && diesel migration generate migration_name   # Create new migration
cd server && diesel migration run                       # Run migrations
make generate-schema                                    # Regenerate schema.rs
```

## Client Regeneration

API clients regenerate automatically when server code changes. To force:

```bash
make clean-api && make test
```

## Tips

- Changes to Cargo.toml may require `make dev-down && make dev`
- Database persists via Docker volume
- Generated clients are checked in, so CLI builds without running server
