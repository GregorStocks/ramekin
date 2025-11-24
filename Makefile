.PHONY: help dev up down restart logs logs-server logs-db generate-clients _generate-clients-run lint clean generate-schema test test-up test-run test-down test-clean test-logs

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: ## Start dev environment (with hot-reload)
	docker compose up --build -d --wait

up: dev ## Alias for dev

down: ## Stop all services
	docker compose down

restart: ## Force restart services
	docker compose up --build -d --force-recreate --wait

logs: ## Show all logs
	docker compose logs -f

logs-server: ## Show server logs
	docker logs ramekin-server -f

logs-db: ## Show database logs
	docker logs ramekin-postgres -f

generate-clients: _generate-clients-run lint ## Regenerate all API clients (Rust, TypeScript, Python)

_generate-clients-run:
	./scripts/generate-clients.sh

lint: ## Run all linters (Rust, TypeScript, Python)
	cargo fmt --all
	cargo clippy --all-targets --all-features -q -- -D warnings
	npx prettier --write --log-level warn ramekin-ui/src/
	uvx ruff format --quiet tests/
	uvx ruff check --fix --quiet tests/

clean: ## Stop services and clean volumes
	docker compose down -v

generate-schema: restart ## Regenerate schema.rs from database (runs migrations first)
	@docker compose exec server diesel print-schema > crates/ramekin-server/src/schema.rs
	@echo "Schema generated at crates/ramekin-server/src/schema.rs"

test: test-clean test-up ## Run tests (tears down on success, leaves up on failure for debugging)
	@docker compose -f docker-compose.test.yml run --build --rm tests && docker compose -f docker-compose.test.yml down -v || (echo "Tests failed - leaving environment up for debugging" && exit 1)

test-up: ## Start test environment
	docker compose -f docker-compose.test.yml up --build -d --wait postgres server

test-run: ## Run tests against running test environment
	docker compose -f docker-compose.test.yml run --build --rm tests

test-down: ## Stop test environment
	docker compose -f docker-compose.test.yml down

test-clean: ## Stop test environment and clean volumes
	docker compose -f docker-compose.test.yml down -v

test-logs: ## Show test environment logs
	docker compose -f docker-compose.test.yml logs -f
