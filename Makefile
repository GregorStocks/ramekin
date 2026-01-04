.PHONY: help dev up down restart logs logs-server logs-db generate-clients lint clean generate-schema test test-docker test-docker-up test-docker-down test-docker-clean test-docker-logs test-docker-rebuild seed load-test screenshot install-hooks check-deps install-deps

# Project names to keep dev and test environments isolated
DEV_PROJECT := ramekin
TEST_PROJECT := ramekin-test

# Timestamp wrapper for log output
TS := ./scripts/ts

# OpenAPI Generator configuration
OPENAPI_GENERATOR_VERSION := 7.10.0
OPENAPI_GENERATOR_JAR := .cache/openapi-generator-cli-$(OPENAPI_GENERATOR_VERSION).jar

# Export UID/GID for docker-compose to run containers as current user (Linux compatibility)
export UID := $(shell id -u)
export GID := $(shell id -g)

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

check-deps: ## Check that all development dependencies are installed
	@missing=""; \
	command -v cargo >/dev/null 2>&1 || missing="$$missing cargo"; \
	command -v diesel >/dev/null 2>&1 || missing="$$missing diesel"; \
	command -v java >/dev/null 2>&1 || missing="$$missing java"; \
	command -v uv >/dev/null 2>&1 || missing="$$missing uv"; \
	command -v npx >/dev/null 2>&1 || missing="$$missing npx"; \
	command -v psql >/dev/null 2>&1 || missing="$$missing psql"; \
	if [ ! -f "$(OPENAPI_GENERATOR_JAR)" ]; then missing="$$missing openapi-generator-cli.jar"; fi; \
	if [ -n "$$missing" ]; then \
		echo "ERROR: Missing dependencies:$$missing" >&2; \
		echo "Run 'make install-deps' to install them" >&2; \
		exit 1; \
	fi; \
	echo "All dependencies installed"

install-deps: ## Install development dependencies (best effort)
	@echo "Installing development dependencies..."
	@echo ""
	@echo "=== Rust toolchain (cargo) ==="
	@command -v cargo >/dev/null 2>&1 && echo "cargo: already installed" || echo "Install from: https://rustup.rs/"
	@echo ""
	@echo "=== Diesel CLI ==="
	@command -v diesel >/dev/null 2>&1 && echo "diesel: already installed" || \
		(echo "Installing diesel_cli..." && cargo install diesel_cli --no-default-features --features postgres)
	@echo ""
	@echo "=== Java (for OpenAPI Generator) ==="
	@command -v java >/dev/null 2>&1 && echo "java: already installed" || echo "Install OpenJDK 11+ from your package manager"
	@echo ""
	@echo "=== OpenAPI Generator CLI ==="
	@if [ -f "$(OPENAPI_GENERATOR_JAR)" ]; then \
		echo "openapi-generator-cli: already installed"; \
	else \
		echo "Downloading OpenAPI Generator CLI..."; \
		mkdir -p .cache; \
		curl -L -o "$(OPENAPI_GENERATOR_JAR)" \
			"https://repo1.maven.org/maven2/org/openapitools/openapi-generator-cli/$(OPENAPI_GENERATOR_VERSION)/openapi-generator-cli-$(OPENAPI_GENERATOR_VERSION).jar"; \
	fi
	@echo ""
	@echo "=== uv (Python package manager) ==="
	@command -v uv >/dev/null 2>&1 && echo "uv: already installed" || \
		(echo "Installing uv..." && curl -LsSf https://astral.sh/uv/install.sh | sh)
	@echo ""
	@echo "=== Node.js/npx ==="
	@command -v npx >/dev/null 2>&1 && echo "npx: already installed" || echo "Install Node.js from: https://nodejs.org/"
	@echo ""
	@echo "=== PostgreSQL client ==="
	@command -v psql >/dev/null 2>&1 && echo "psql: already installed" || echo "Install postgresql-client from your package manager"
	@echo ""
	@echo "Run 'make check-deps' to verify all dependencies are installed"

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

generate-clients: check-deps ## Generate OpenAPI spec and regenerate all API clients
	@./scripts/generate-openapi.py 2>&1 | $(TS)

generate-clients-force: ## Force regeneration of API clients (bypass cache)
	@# NOTE: You should never need to run this. If clients aren't regenerating,
	@# it means the OpenAPI spec hasn't changed. Check that the server is
	@# running with your latest code changes.
	@rm -f .cache/openapi-hash
	@$(MAKE) generate-clients

lint: check-deps ## Run all linters (Rust, TypeScript, Python)
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

test: check-deps generate-clients ## Run tests locally (without Docker)
	@./scripts/run-tests.py 2>&1 | $(TS)

test-docker: generate-clients test-docker-up ## Run tests in Docker (legacy)
	@BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml run --build --rm --quiet-pull tests 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$"

test-docker-up: ## Start Docker test environment
	@if ! docker ps --filter "name=ramekin-test-server" --filter "status=running" -q | grep -q .; then \
	  { echo "Test environment not running, starting..."; \
	  BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml up --build -d --wait --quiet-pull postgres server 2>&1 | grep -vE "^#|^$$|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true; } | $(TS); \
	fi

test-docker-down: ## Stop Docker test environment
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down 2>/dev/null

test-docker-clean: ## Stop Docker test environment and clean volumes
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down -v 2>/dev/null

test-docker-logs: ## Show Docker test environment logs
	docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml logs -f

test-docker-rebuild: test-docker-clean test-docker-up ## Force rebuild Docker test environment from scratch

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
