.PHONY: help dev up down restart logs logs-server logs-db generate-clients lint clean generate-schema test test-up test-run test-down test-clean test-logs seed load-test screenshot

# Project names to keep dev and test environments isolated
DEV_PROJECT := ramekin
TEST_PROJECT := ramekin-test

# Export UID/GID for docker-compose to run containers as current user (Linux compatibility)
export UID := $(shell id -u)
export GID := $(shell id -g)

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: generate-clients ## Start dev environment (with hot-reload)
	@BUILDKIT_PROGRESS=plain docker compose -p $(DEV_PROJECT) up --build -d --wait --quiet-pull 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true
	@echo "Dev environment ready"
	@$(MAKE) seed

up: dev ## Alias for dev

down: ## Stop all services
	@docker compose -p $(DEV_PROJECT) down 2>/dev/null

restart: generate-clients ## Force restart services
	@BUILDKIT_PROGRESS=plain docker compose -p $(DEV_PROJECT) up --build -d --force-recreate --wait --quiet-pull 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true
	@echo "Services restarted"

logs: ## Show all logs
	docker compose -p $(DEV_PROJECT) logs -f

logs-server: ## Show server logs
	docker logs ramekin-server -f

logs-db: ## Show database logs
	docker logs ramekin-postgres -f

generate-clients: ## Generate OpenAPI spec and regenerate all API clients
	@./scripts/generate-openapi.py

generate-clients-force: ## Force regeneration of API clients (bypass cache)
	@rm -f .cache/openapi-hash
	@$(MAKE) generate-clients

lint: ## Run all linters (Rust, TypeScript, Python)
	@./scripts/lint.py

clean: test-clean ## Stop services, clean volumes, and remove generated clients
	@docker compose -p $(DEV_PROJECT) down -v 2>/dev/null
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/
	@rm -rf server/target/ cli/target/
	@rm -rf ramekin-ui/node_modules/
	@rm -rf tests/__pycache__/ scripts/__pycache__/

generate-schema: restart ## Regenerate schema.rs from database (runs migrations first)
	@docker compose -p $(DEV_PROJECT) exec server diesel print-schema > server/src/schema.rs
	@echo "Schema generated at server/src/schema.rs"
	$(MAKE) lint

test: generate-clients test-up ## Run tests (reuses running containers if available)
	@BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml run --build --rm --quiet-pull tests 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$"

test-up: ## Start test environment
	@if ! docker ps --filter "name=ramekin-test-server" --filter "status=running" -q | grep -q .; then \
		echo "Test environment not running, starting..."; \
		@BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml up --build -d --wait --quiet-pull postgres server 2>&1 | grep -vE "^#|^$$|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true
	fi

test-down: ## Stop test environment
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down 2>/dev/null

test-clean: ## Stop test environment and clean volumes
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down -v 2>/dev/null

test-logs: ## Show test environment logs
	docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml logs -f

test-rebuild: test-clean test-up ## Force rebuild test environment from scratch

seed: ## Create test user with sample recipes (requires dev server running)
	@cd cli && cargo run -q -- seed --username t --password t ../data/dev/seed.paprikarecipes

load-test: ## Run load test creating users with recipes and photos (for performance testing)
	@cd cli && cargo run -q -- load-test

screenshot: up seed ## Take screenshots of the app (cookbook, recipe, edit)
	@cd cli && cargo run -q -- screenshot
