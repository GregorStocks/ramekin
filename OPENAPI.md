# OpenAPI and Client Generation

## Overview

The Ramekin server now exposes an OpenAPI 3.0 specification that can be used to auto-generate TypeScript/JavaScript clients.

## Starting the Server

Start the server:

```bash
make dev
```

The server will be available at http://localhost:3000

## Accessing the API Documentation

Once the server is running:

- **Swagger UI (interactive docs)**: http://localhost:3000/swagger-ui/
- **OpenAPI spec (JSON)**: http://localhost:3000/api-docs/openapi.json

The Swagger UI provides an interactive interface where you can explore and test your API endpoints directly in the browser.

## Generating a JavaScript/TypeScript Client

### Option 1: Using openapi-generator-cli

```bash
# Install openapi-generator-cli
npm install -g @openapitools/openapi-generator-cli

# Generate TypeScript client
openapi-generator-cli generate \
  -i http://localhost:3000/api-docs/openapi.json \
  -g typescript-fetch \
  -o ramekin-ui/src/generated/api

# Or generate JavaScript client
openapi-generator-cli generate \
  -i http://localhost:3000/api-docs/openapi.json \
  -g javascript \
  -o ramekin-ui/src/generated/api
```

### Option 2: Using orval (recommended for TypeScript)

```bash
# Install orval
npm install -D orval

# Create orval.config.ts
cat > orval.config.ts << 'EOF'
module.exports = {
  ramekin: {
    input: 'http://localhost:3000/api-docs/openapi.json',
    output: {
      mode: 'single',
      target: 'ramekin-ui/src/generated/api.ts',
      client: 'fetch',
      override: {
        mutator: {
          path: './src/api-client.ts',
          name: 'customFetch',
        },
      },
    },
  },
};
EOF

# Generate client
npx orval
```

### Option 3: Using the OpenAPI spec file directly

```bash
# Download the spec to a file
curl http://localhost:3000/api-docs/openapi.json > openapi.json

# Then use any OpenAPI-compatible tool
openapi-generator-cli generate -i openapi.json -g typescript-fetch -o ./generated
```

## Adding New Endpoints

When adding new endpoints to the server:

1. Add `#[utoipa::path(...)]` attribute to the handler function
2. Add the handler to the `paths(...)` list in the `ApiDoc` struct
3. Add response types to `components(schemas(...))` if they're new
4. Regenerate the client

Example:
```rust
#[utoipa::path(
    get,
    path = "/api/items",
    responses(
        (status = 200, description = "List of items", body = ItemsResponse)
    )
)]
async fn get_items() -> Json<ItemsResponse> {
    // ...
}
```

## Exploring the API with Swagger UI

The server includes a built-in Swagger UI at http://localhost:3000/swagger-ui/ where you can:

- View all available endpoints and their documentation
- See request/response schemas
- Test endpoints directly from the browser
- Download the OpenAPI spec

Alternatively, you can use the online Swagger Editor at https://editor.swagger.io/ and paste the OpenAPI JSON from http://localhost:3000/api-docs/openapi.json.
