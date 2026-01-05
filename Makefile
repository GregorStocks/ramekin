.PHONY: help dev dev-headless dev-down generate-clients check-deps lint clean generate-schema test venv venv-clean db-up db-down db-clean seed load-test install-hooks setup-claude-web devbox devbox-shell devbox-down devbox-clean

# Use bash with pipefail so piped commands propagate exit codes
SHELL := /bin/bash
.SHELLFLAGS := -o pipefail -c

# Project name for docker-compose
PROJECT := ramekin

# Timestamp wrapper for log output
TS := ./scripts/ts

# Export UID/GID for docker-compose to run containers as current user (Linux compatibility)
export UID := $(shell id -u)
export GID := $(shell id -g)

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

# =============================================================================
# Devbox (Docker-based development)
# =============================================================================

devbox: ## Start devbox environment (docker-compose up, then exec in)
	@echo "Starting devbox..."
	@docker compose -p $(PROJECT) -f docker-compose.devbox.yml up --build -d --wait
	@echo ""
	@echo "Devbox ready! Run: make devbox-shell"

devbox-shell: ## Open shell in devbox container
	@docker compose -p $(PROJECT) -f docker-compose.devbox.yml exec devbox bash

devbox-down: ## Stop devbox environment
	@docker compose -p $(PROJECT) -f docker-compose.devbox.yml down

devbox-clean: ## Stop devbox and remove volumes
	@docker compose -p $(PROJECT) -f docker-compose.devbox.yml down -v

# =============================================================================
# Local development (native, not Docker)
# =============================================================================

dev: check-deps generate-clients ## Start local dev environment (server + UI via process-compose)
	@echo "Starting dev environment (Ctrl+C to stop)..."
	@mkdir -p logs
	@process-compose up -e dev.env

dev-headless: check-deps generate-clients ## Start local dev environment without TUI
	@echo "Starting dev environment (headless)..."
	@mkdir -p logs
	@process-compose up -e dev.env -t=false

dev-down: ## Stop dev processes
	@process-compose down 2>/dev/null || true
	@pkill -f "cargo watch" 2>/dev/null || true

# =============================================================================
# Code generation and linting
# =============================================================================

generate-clients: venv ## Generate OpenAPI spec and clients
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/generate-openapi.py 2>&1 | $(TS)

generate-clients-force: ## Force regeneration of API clients (bypass cache)
	@# NOTE: You should never need to run this. If clients aren't regenerating,
	@# it means the OpenAPI spec hasn't changed. Check that the server is
	@# running with your latest code changes.
	@rm -f api/openapi-hash
	@$(MAKE) generate-clients

lint: venv ## Run all linters (Rust, TypeScript, Python)
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/lint.py 2>&1 | $(TS)

generate-schema: ## Regenerate schema.rs from database (runs migrations first)
	@diesel migration run
	@diesel print-schema > server/src/schema.rs
	@echo "Schema generated at server/src/schema.rs" | $(TS)
	$(MAKE) lint

# =============================================================================
# Testing
# =============================================================================

setup-claude-web: ## Setup environment for Claude Code for Web (no-op elsewhere)
	@./scripts/setup-claude-web.sh

test: setup-claude-web check-deps generate-clients ## Run tests
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-tests.sh

# =============================================================================
# Python venv
# =============================================================================

.venv/.installed: requirements-test.txt
	@uv venv
	@uv pip install -r requirements-test.txt
	@touch .venv/.installed

venv: .venv/.installed ## Create Python venv with test dependencies

check-deps: venv ## Check that all dependencies are installed
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/check-deps.sh

venv-clean: ## Remove Python venv
	@rm -rf .venv

# =============================================================================
# Database (for native development only)
# =============================================================================

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

# =============================================================================
# Utilities
# =============================================================================

seed: ## Create test user with sample recipes (requires dev server running)
	@cd cli && cargo run -q -- seed --username t --password t ../data/dev/seed.paprikarecipes

load-test: ## Run load test creating users with recipes and photos (for performance testing)
	@cd cli && cargo run -q -- load-test

install-hooks: ## Install git hooks for local development
	@cp scripts/pre-push .git/hooks/pre-push
	@chmod +x .git/hooks/pre-push
	@echo "Git hooks installed successfully"

clean: devbox-clean ## Stop services, clean volumes, and remove generated clients
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/
	@rm -rf server/target/ cli/target/
	@rm -rf ramekin-ui/node_modules/
	@rm -rf tests/__pycache__/ scripts/__pycache__/
	@rm -rf .cache/ logs/
