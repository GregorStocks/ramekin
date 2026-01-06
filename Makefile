.PHONY: help dev dev-headless dev-down check-deps lint clean clean-api generate-schema test venv venv-clean db-up db-down db-clean seed load-test install-hooks setup-claude-web

# Use bash with pipefail so piped commands propagate exit codes
SHELL := /bin/bash
.SHELLFLAGS := -o pipefail -c

# Timestamp wrapper for log output
TS := ./scripts/ts

# Source files that affect the OpenAPI spec
API_SOURCES := $(shell find server/src/api -type f -name '*.rs' 2>/dev/null) server/src/models.rs server/src/schema.rs

# Marker file for generated clients
CLIENT_MARKER := cli/generated/ramekin-client/Cargo.toml

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: check-deps $(CLIENT_MARKER) ## Start local dev environment (server + UI via process-compose)
	@echo "Starting dev environment (Ctrl+C to stop)..."
	@mkdir -p logs
	@process-compose up -e dev.env

dev-headless: check-deps $(CLIENT_MARKER) ## Start local dev environment without TUI
	@echo "Starting dev environment (headless)..."
	@mkdir -p logs
	@process-compose up -e dev.env -t=false

dev-down: ## Stop dev processes (not database)
	@process-compose down 2>/dev/null || true
	@pkill -f "cargo watch" 2>/dev/null || true

# Generate OpenAPI spec from Rust source
api/openapi.json: check-deps $(API_SOURCES)
	@echo "Building server and generating OpenAPI spec..." | $(TS)
	@mkdir -p api
	@cd server && cargo build --release -q
	@server/target/release/ramekin-server --openapi > api/openapi.json
	@echo "Generated api/openapi.json" | $(TS)

# Generate clients from OpenAPI spec
$(CLIENT_MARKER): api/openapi.json
	@./scripts/generate-clients.sh
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/lint.py 2>&1 | $(TS)

lint: venv $(CLIENT_MARKER) ## Run all linters (Rust, TypeScript, Python)
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/lint.py 2>&1 | $(TS)

clean: ## Clean generated files and build artifacts
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/
	@rm -rf server/target/ cli/target/
	@rm -rf ramekin-ui/node_modules/
	@rm -rf tests/__pycache__/ scripts/__pycache__/
	@rm -rf .cache/ logs/

clean-api: ## Force regeneration of OpenAPI spec and clients on next build
	@rm -f api/openapi.json
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/

generate-schema: ## Regenerate schema.rs from database (requires db-up and migrations run)
	@cd server && diesel print-schema > src/schema.rs
	@echo "Schema generated at server/src/schema.rs" | $(TS)
	$(MAKE) lint

setup-claude-web: ## Setup environment for Claude Code for Web (no-op elsewhere)
	@./scripts/setup-claude-web.sh

test: check-deps $(CLIENT_MARKER) ## Run tests natively
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-tests.sh

.venv/.installed: requirements-test.txt
	@uv venv
	@uv pip install -r requirements-test.txt
	@touch .venv/.installed

venv: .venv/.installed ## Create Python venv with test dependencies

check-deps: venv setup-claude-web ## Check that all dependencies are installed
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/check-deps.sh

venv-clean: ## Remove Python venv
	@rm -rf .venv

db-up: ## Start postgres container with dev and test databases
	@if ! docker ps --format '{{.Names}}' | grep -q '^ramekin-db$$'; then \
	  echo "Starting postgres..."; \
	  docker run -d --name ramekin-db \
	    -e POSTGRES_USER=ramekin \
	    -e POSTGRES_PASSWORD=ramekin \
	    -e POSTGRES_DB=ramekin \
	    -p 54321:5432 \
	    postgres:16-alpine >/dev/null; \
	  echo "Waiting for postgres..."; \
	  until docker exec ramekin-db pg_isready -U ramekin >/dev/null 2>&1; do sleep 0.2; done; \
	  echo "Creating test database..."; \
	  docker exec ramekin-db createdb -U ramekin ramekin_test 2>/dev/null || true; \
	  echo "Postgres ready on localhost:54321 (databases: ramekin, ramekin_test)"; \
	fi

db-down: ## Stop postgres container
	@docker stop ramekin-db >/dev/null 2>&1 || true
	@docker rm ramekin-db >/dev/null 2>&1 || true

db-clean: db-down ## Stop postgres and remove data

seed: ## Create test user with sample recipes (requires dev server running)
	@cd cli && cargo run -q -- seed --username t --password t ../data/dev/seed.paprikarecipes

load-test: ## Run load test creating users with recipes and photos (for performance testing)
	@cd cli && cargo run -q -- load-test

install-hooks: ## Install git hooks for local development
	@cp scripts/pre-push .git/hooks/pre-push
	@chmod +x .git/hooks/pre-push
	@echo "Git hooks installed successfully"
