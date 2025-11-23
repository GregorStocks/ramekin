# Ramekin CLI

The Ramekin CLI is a client application that communicates with the Ramekin server via its REST API.

## Architecture

The CLI is designed as "just another client" - it uses an auto-generated client from the server's OpenAPI specification, just like the JavaScript client will. This ensures:

- Type safety between CLI and server
- No shared code/lib dependencies to maintain
- Easy iteration - regenerate when API changes
- Same workflow as other clients (JS, Python, etc.)

## Development Workflow

### 1. Start the server

```bash
docker compose up
```

### 2. Generate the client (when API changes)

```bash
cd crates/ramekin-cli
./generate-client.sh
```

This will:
- Fetch the OpenAPI spec from the running server
- Generate a Rust client in `src/generated/`
- Add necessary dependencies

### 3. Use the generated client

Update your CLI commands to use the generated client instead of manual HTTP calls.

## Current Implementation

Currently, the CLI makes direct HTTP calls with manually defined response types. These are marked as "Temporary" and will be replaced with the generated client.

## Usage

```bash
# List all garbages
cargo run -p ramekin-cli -- garbages

# With custom server URL
cargo run -p ramekin-cli -- garbages --server http://localhost:3000
```

## Regenerating the Client

Regenerate the client whenever:
- The server API changes (new endpoints, modified responses)
- You want to use new API features in the CLI

The generated client is checked into git so the CLI can be built without running the server first. When you regenerate it, commit the changes so everyone stays in sync with the latest API.
