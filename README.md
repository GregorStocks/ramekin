# Ramekin

Recipe management app with a Rust backend and SolidJS frontend.

## Quick Start

```bash
make dev   # Starts backend + frontend + database with hot-reload
```

Visit http://localhost:5173 (API docs at http://localhost:3000/swagger-ui/)

## Commands

Run `make help` to see all commands. Key ones:

```bash
make dev        # Start dev environment
make test       # Run tests
make lint       # Run linter
make seed       # Load sample recipes (requires dev server)
```

## Project Structure

```
server/       # Rust API server (Axum)
cli/          # Rust CLI client
ramekin-ui/   # SolidJS frontend
```

API clients are auto-generated from OpenAPI spec. Edit server code and clients regenerate on next build.
