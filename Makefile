.PHONY: help dev up down restart logs logs-server logs-db generate-clients lint clean generate-schema test test-up test-run test-down test-clean test-logs seed screenshot

# Project names to keep dev and test environments isolated
DEV_PROJECT := ramekin
TEST_PROJECT := ramekin-test

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
	@mkdir -p api
	@docker run --rm \
		-v $(PWD):/app \
		-w /app/server \
		rust:latest \
		sh -c "cargo build --release -q 2>/dev/null && target/release/ramekin-server --openapi" \
		> api/openapi.json
	@echo "Generated api/openapi.json"
	@./scripts/generate-clients.sh
	@$(MAKE) lint

lint: ## Run all linters (Rust, TypeScript, Python)
	@cd server && cargo fmt --all
	@cd server && cargo clippy --all-targets --all-features -q -- -D warnings
	@cd cli && cargo fmt --all
	@cd cli && cargo clippy --all-targets --all-features -q -- -D warnings
	@npx prettier --write --log-level warn ramekin-ui/src/
	@cd ramekin-ui && npx tsc -p tsconfig.app.json --noEmit
	@uvx ruff format --quiet --exclude tests/generated tests/ scripts/
	@uvx ruff check --fix --quiet --exclude tests/generated tests/ scripts/
	@echo "Linted"

clean: test-clean ## Stop services, clean volumes, and remove generated clients
	@docker compose -p $(DEV_PROJECT) down -v 2>/dev/null
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/

generate-schema: restart ## Regenerate schema.rs from database (runs migrations first)
	@docker compose -p $(DEV_PROJECT) exec server diesel print-schema > server/src/schema.rs
	@echo "Schema generated at server/src/schema.rs"
	$(MAKE) lint

test: generate-clients test-down test-up ## Run tests (tears down on success, leaves up on failure for debugging)
	@BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml run --build --rm --quiet-pull tests 2>&1 | grep -vE "^#|^$$|Container|Network|level=warning|Built" | grep -v "^\s*$$" && docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down 2>/dev/null || (echo "Tests failed - leaving environment up for debugging" && exit 1)

test-up: ## Start test environment
	@BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml up --build -d --wait --quiet-pull postgres server 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$" || true

test-run: ## Run tests against running test environment
	@BUILDKIT_PROGRESS=plain docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml run --build --rm --quiet-pull tests 2>&1 | grep -vE "^#|Container|Network|level=warning|Built" | grep -v "^\s*$$"

test-down: ## Stop test environment
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down 2>/dev/null

test-clean: ## Stop test environment and clean volumes
	@docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml down -v 2>/dev/null

test-logs: ## Show test environment logs
	docker compose -p $(TEST_PROJECT) -f docker-compose.test.yml logs -f

seed: ## Create test user with sample recipes (requires dev server running)
	@cd cli && cargo run -q -- seed --username t --password t

screenshot: up seed ## Take a screenshot of the app (use ARGS for options)
	@uv run scripts/screenshot.py $(ARGS)
