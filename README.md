# Ramekin

A web-based recipe application with a Rust backend and Solid frontend.

## Architecture

Ramekin consists of five main components:

1. **ramekin-ui** - Web frontend built with SolidJS
2. **ramekin-server** - REST API server built with Axum
3. **ramekin-lib** - Shared library for common types and logic
4. **ramekin-cli** - Command-line interface for interacting with the server or running local operations
5. **PostgreSQL** - Database for persistent storage

## Project Structure

```
ramekin/
├── crates/
│   ├── ramekin-lib/      # Shared types, models, and utilities
│   ├── ramekin-server/   # Axum web server
│   └── ramekin-cli/      # CLI tool
├── ramekin-ui/           # SolidJS frontend
├── migrations/           # Diesel database migrations
├── docker-compose.yml    # PostgreSQL container configuration
└── Cargo.toml           # Rust workspace configuration
```

## Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) (v18 or later)
- [Docker](https://www.docker.com/) and Docker Compose
- [Diesel CLI](https://diesel.rs/guides/getting-started) - Install with:
  ```bash
  cargo install diesel_cli --no-default-features --features postgres
  ```

## Getting Started

### 1. Run the backend

```bash
docker-compose up -d
```

### 2. Run the frontend

In a separate terminal:

```bash
cd ramekin-ui
npm install
npm run dev
```

The frontend will be available at http://localhost:5173

### 3. Use the CLI

```bash
cargo run --bin ramekin-cli -- --help
```

## Development

### Running tests

```bash
cargo test
```

### Creating a new migration

```bash
diesel migration generate <migration_name>
```

### Frontend development

The frontend uses Vite for hot module replacement. Changes will automatically reload in the browser.

### Checking code

```bash
# Check Rust code
cargo check

# Format Rust code
cargo fmt

# Lint Rust code
cargo clippy

# Check frontend
cd ramekin-ui
npm run build
```

## Database

The application uses PostgreSQL with Diesel ORM. The database schema is defined in migrations and automatically generates Rust types in `crates/ramekin-lib/src/schema.rs`.

Connection details:
- Host: localhost:5432
- Database: ramekin
- Username: ramekin
- Password: ramekin

## License

MIT OR Apache-2.0
