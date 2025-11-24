# Development Guide

## Development Environment

The project uses Docker Compose with hot-reload for development.

### Starting the Development Server

```bash
# Start dev environment with hot-reload
make dev
```

The development setup:
- **Auto-recompiles** when you change code (using cargo-watch)
- **Mounts source code** as volumes so changes are picked up immediately
- **Fast incremental builds** using a Docker volume for the target directory
- **Debug logging** enabled by default

### Making Changes

1. Edit code in `crates/ramekin-server/src/`
2. cargo-watch detects changes automatically
3. Server recompiles and restarts (usually takes 1-2 seconds for small changes)
4. Check logs: `make logs-server`

### Stopping

```bash
# Stop all services
make down

# Stop and remove volumes (full cleanup)
make clean
```

## CLI Development

The CLI uses a generated client from the server's OpenAPI spec.

### Regenerating the Client

When you add or change API endpoints:

```bash
# Make sure dev server is running first
make dev

# In another terminal, regenerate clients
make generate-clients

# Or just the Rust client
make generate-rust-client

# Commit the generated code
git add crates/ramekin-client
git commit -m "Regenerate API client"
```

### Running the CLI

```bash
cargo run -p ramekin-cli -- garbages
```

## Database Migrations

```bash
# Run migrations (if needed)
docker exec ramekin-server diesel migration run

# Create a new migration
docker exec ramekin-server diesel migration generate migration_name
```

## Useful Commands

```bash
# View all logs
make logs

# View server logs only
make logs-server

# View database logs only
make logs-db

# Run linter
make lint

# Run tests
cargo test

# See all available commands
make help
```

## Tips

- **First build takes ~45 seconds** (installs cargo-watch), subsequent builds are much faster
- **Changes to Cargo.toml** may require restarting: `make down` then `make dev`
- **Database persists** between restarts via Docker volume
- **The CLI can be built** without running the server (uses checked-in generated client)
