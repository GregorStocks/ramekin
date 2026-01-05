.PHONY: help dev up down restart logs logs-server logs-db generate-clients lint clean generate-schema test venv venv-clean db-up db-down db-clean test-docker test-docker-shell test-docker-up test-docker-down test-docker-clean seed load-test screenshot install-hooks

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

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: generate-clients ## Start dev environment (with hot-reload)
	@{ BUILDKIT_PROGRESS=plain docker compose -p $(DEV_PROJECT) up --build -d --wait --quiet-pull 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true; echo "Dev environment ready"; } | $(TS)
	@$(MAKE) seed

up: dev ## Alias for dev

down: ## Stop all services
	@docker compose -p $(DEV_PROJECT) down 2>/dev/null

restart: generate-clients ## Force restart services
	@{ BUILDKIT_PROGRESS=plain docker compose -p $(DEV_PROJECT) up --build -d --force-recreate --wait --quiet-pull 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true; echo "Services restarted"; } | $(TS)

logs: ## Show all logs
	docker compose -p $(DEV_PROJECT) logs -f

logs-server: ## Show server logs
	docker logs ramekin-server -f

logs-db: ## Show database logs
	docker logs ramekin-postgres -f

generate-clients: ## Generate OpenAPI spec and regenerate all API clients
	@./scripts/generate-openapi.py 2>&1 | $(TS)

generate-clients-force: ## Force regeneration of API clients (bypass cache)
	@# NOTE: You should never need to run this. If clients aren't regenerating,
	@# it means the OpenAPI spec hasn't changed. Check that the server is
	@# running with your latest code changes.
	@rm -f .cache/openapi-hash
	@$(MAKE) generate-clients

lint: ## Run all linters (Rust, TypeScript, Python)
	@./scripts/lint.py 2>&1 | $(TS)

clean: test-docker-clean ## Stop services, clean volumes, and remove generated clients
	@docker compose -p $(DEV_PROJECT) down -v 2>/dev/null
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/
	@rm -rf server/target/ cli/target/
	@rm -rf ramekin-ui/node_modules/
	@rm -rf tests/__pycache__/ scripts/__pycache__/
	@rm -rf .cache/

generate-schema: restart ## Regenerate schema.rs from database (runs migrations first)
	@docker compose -p $(DEV_PROJECT) exec server diesel print-schema > server/src/schema.rs
	@echo "Schema generated at server/src/schema.rs" | $(TS)
	$(MAKE) lint

test: generate-clients venv ## Run tests natively (requires test.env, postgres, rust, python)
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/check-test-deps.sh
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-tests.sh

.venv/.installed: requirements-test.txt
	@uv venv
	@uv pip install -r requirements-test.txt
	@touch .venv/.installed

venv: .venv/.installed ## Create Python venv with test dependencies

venv-clean: ## Remove Python venv
	@rm -rf .venv

db-up: ## Start postgres container for local testing
	@if ! docker ps --format '{{.Names}}' | grep -q '^ramekin-test-db$$'; then \
	  echo "Starting postgres..."; \
	  docker run -d --name ramekin-test-db \
	    -e POSTGRES_USER=ramekin \
	    -e POSTGRES_PASSWORD=ramekin \
	    -e POSTGRES_DB=ramekin_test \
	    -p 54321:5432 \
	    postgres:16-alpine >/dev/null; \
	  echo "Waiting for postgres..."; \
	  until docker exec ramekin-test-db pg_isready -U ramekin >/dev/null 2>&1; do sleep 0.2; done; \
	  echo "Postgres ready on localhost:54321"; \
	fi

db-down: ## Stop postgres container
	@docker stop ramekin-test-db >/dev/null 2>&1 || true
	@docker rm ramekin-test-db >/dev/null 2>&1 || true

db-clean: db-down ## Stop postgres and remove data

test-docker: generate-clients test-docker-up ## Run tests in Docker
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

screenshot: up seed ## Take screenshots of the app (cookbook, recipe, edit)
	@cd cli && cargo run -q -- screenshot

install-hooks: ## Install git hooks for local development
	@cp scripts/pre-push .git/hooks/pre-push
	@chmod +x .git/hooks/pre-push
	@echo "Git hooks installed successfully"
