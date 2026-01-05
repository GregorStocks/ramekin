# Development Guide

## Development Environment

The project runs natively with hot-reload for development. Only PostgreSQL runs in Docker.

### Starting the Development Server

```bash
# Start postgres (if not already running)
make db-up

# Start dev environment with hot-reload
make dev
```

The development setup:
- **Auto-recompiles** when you change code (using cargo-watch)
- **Fast incremental builds** using native cargo
- **Debug logging** enabled by default

### Making Changes

1. Edit code in `server/src/`
2. cargo-watch detects changes automatically
3. Server recompiles and restarts (usually takes 1-2 seconds for small changes)

### Stopping

```bash
# Stop dev processes
make dev-down

# Stop postgres
make db-down

# Clean build artifacts
make clean
```

## CLI Development

The CLI uses a generated client from the server's OpenAPI spec.

### Regenerating the Client

When you add or change API endpoints, the clients are regenerated automatically on next build.

```bash
# Force client regeneration
make clean-api
make test
```

### Running the CLI

```bash
cargo run -p ramekin-cli -- garbages
```

## Database Migrations

```bash
# Run migrations (diesel CLI must be installed)
cd server && diesel migration run

# Create a new migration
cd server && diesel migration generate migration_name

# Regenerate schema.rs after migration
make generate-schema
```

## Useful Commands

```bash
# Run linter
make lint

# Run tests
make test

# See all available commands
make help
```

## Tips

- **Changes to Cargo.toml** may require restarting: `make dev-down` then `make dev`
- **Database persists** between restarts via Docker volume
- **The CLI can be built** without running the server (uses checked-in generated client)
