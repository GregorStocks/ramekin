# Ramekin

A web application with a Rust backend and SolidJS frontend.

## Architecture

- **ramekin-server** - REST API server (Rust + Axum)
- **ramekin-ui** - Web frontend (SolidJS)
- **ramekin-cli** - Command-line client (Rust)
- **ramekin-client** - Auto-generated Rust API client (from OpenAPI spec)
- **PostgreSQL** - Database

The API is defined once in the server using OpenAPI annotations, then clients (both Rust CLI and TypeScript UI) are auto-generated to stay in sync.

## Quick Start

```bash
# Start development environment (backend + database with hot-reload)
make dev

# In another terminal, start the UI
cd ramekin-ui
npm install
npm run dev
```

Visit:
- Frontend: http://localhost:5173
- API: http://localhost:3000
- API Docs: http://localhost:3000/swagger-ui/

## Common Commands

Run `make help` to see all available commands:

```bash
make dev              # Start dev environment (with hot-reload)
make dev-down         # Stop dev processes
make db-up            # Start postgres container
make db-down          # Stop postgres container
make test             # Run tests
make lint             # Run linter
make clean            # Clean generated files and build artifacts
```

## Development Workflow

### Making API Changes

1. Edit the server code in `crates/ramekin-server/`
2. The server auto-recompiles (~1-2 seconds)
3. Regenerate clients: `make generate-clients`
4. Commit the generated client code

The generated clients provide type-safe API access:
- **CLI**: Uses `ramekin-client` crate (Rust)
- **UI**: Uses `src/generated/api` (TypeScript)

### Running the CLI

```bash
cargo run -p ramekin-cli -- garbages
```

## Project Structure

```
ramekin/
├── crates/
│   ├── ramekin-server/   # API server with OpenAPI spec
│   ├── ramekin-cli/      # CLI that uses generated client
│   └── generated/
│       └── ramekin-client/  # Auto-generated Rust client
├── ramekin-ui/           # SolidJS frontend
│   └── src/generated/    # Auto-generated TypeScript client
└── Makefile              # All common commands
```

