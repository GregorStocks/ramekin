.PHONY: help dev dev-headless dev-docker dev-down up down restart logs logs-server logs-db check-deps lint clean generate-schema test venv venv-clean db-up db-down db-clean test-docker test-docker-shell test-docker-up test-docker-down test-docker-clean seed load-test install-hooks setup-claude-web

# Use bash with pipefail so piped commands propagate exit codes
SHELL := /bin/bash
.SHELLFLAGS := -o pipefail -c

# Project names to keep dev and test environments isolated
DEV_PROJECT := ramekin
TEST_PROJECT := ramekin-test

# Timestamp wrapper for log output
TS := ./scripts/ts

# Export UID/GID for docker-compose to run containers as current user (Linux compatibility)
export UID := $(shell id -u)
export GID := $(shell id -g)

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

dev-docker: check-deps $(CLIENT_MARKER) ## Start Docker dev environment
	@{ BUILDKIT_PROGRESS=plain docker compose -p $(DEV_PROJECT) up --build -d --wait --quiet-pull 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true; echo "Dev environment ready"; } | $(TS)
	@$(MAKE) seed

dev-docker-down: ## Stop dev processes (not database)
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

lint: venv ## Run all linters (Rust, TypeScript, Python)
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/lint.py 2>&1 | $(TS)

clean: test-docker-clean ## Stop services, clean volumes, and remove generated clients
	@docker compose -p $(DEV_PROJECT) down -v 2>/dev/null
	@rm -f api/openapi.json
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/
	@rm -rf server/target/ cli/target/
	@rm -rf ramekin-ui/node_modules/
	@rm -rf tests/__pycache__/ scripts/__pycache__/
	@rm -rf .cache/ logs/

generate-schema: restart ## Regenerate schema.rs from database (runs migrations first)
	@docker compose -p $(DEV_PROJECT) exec server diesel print-schema > server/src/schema.rs
	@echo "Schema generated at server/src/schema.rs" | $(TS)
	$(MAKE) lint

setup-claude-web: ## Setup environment for Claude Code for Web (no-op elsewhere)
	@./scripts/setup-claude-web.sh

test: setup-claude-web check-deps $(CLIENT_MARKER) ## Run tests natively
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-tests.sh

.venv/.installed: requirements-test.txt
	@uv venv
	@uv pip install -r requirements-test.txt
	@touch .venv/.installed

venv: .venv/.installed ## Create Python venv with test dependencies

check-deps: venv ## Check that all dependencies are installed
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

test-docker: check-deps $(CLIENT_MARKER) test-docker-up ## Run tests in Docker
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml exec tests /usr/local/bin/run-tests.sh

test-docker-shell: test-docker-up ## Start interactive shell in Docker test container
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml exec tests bash

test-docker-up: ## Start Docker test environment (postgres + test container)
	@if ! docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml ps --status running tests 2>/dev/null | grep -q tests; then \
	  echo "Starting test environment..."; \
	  BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml up --build -d --wait --quiet-pull 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true; \
	  echo "Test environment ready"; \
	fi

test-docker-down: ## Stop Docker test environment
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down 2>/dev/null

test-docker-clean: ## Stop and remove Docker test environment with volumes
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down -v 2>/dev/null || true

seed: ## Create test user with sample recipes (requires dev server running)
	@cd cli && cargo run -q -- seed --username t --password t ../data/dev/seed.paprikarecipes

load-test: ## Run load test creating users with recipes and photos (for performance testing)
	@cd cli && cargo run -q -- load-test

install-hooks: ## Install git hooks for local development
	@cp scripts/pre-push .git/hooks/pre-push
	@chmod +x .git/hooks/pre-push
	@echo "Git hooks installed successfully"
