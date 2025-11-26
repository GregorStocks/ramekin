.PHONY: help dev up down restart logs logs-server logs-db generate-clients lint clean generate-schema test test-up test-run test-down test-clean test-logs

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: generate-clients ## Start dev environment (with hot-reload)
	docker compose up --build -d --wait

up: dev ## Alias for dev

down: ## Stop all services
	docker compose down

restart: generate-clients ## Force restart services
	docker compose up --build -d --force-recreate --wait

logs: ## Show all logs
	docker compose logs -f

logs-server: ## Show server logs
	docker logs ramekin-server -f

logs-db: ## Show database logs
	docker logs ramekin-postgres -f

generate-clients: ## Generate OpenAPI spec and regenerate all API clients
	mkdir -p api
	docker run --rm \
		-v $(PWD):/app \
		-w /app/server \
		rust:latest \
		sh -c "cargo build --release -q 2>/dev/null && target/release/ramekin-server --openapi" \
		> api/openapi.json
	echo "Generated api/openapi.json"
	./scripts/generate-clients.sh
	$(MAKE) lint

lint: ## Run all linters (Rust, TypeScript, Python)
	cd server && cargo fmt --all
	cd server && cargo clippy --all-targets --all-features -q -- -D warnings
	npx prettier --write --log-level warn ramekin-ui/src/
	cd ramekin-ui && npx tsc -p tsconfig.app.json --noEmit
	uvx ruff format --quiet --exclude tests/generated tests/
	uvx ruff check --fix --quiet --exclude tests/generated tests/

clean: test-clean ## Stop services, clean volumes, and remove generated clients
	docker compose down -v
	rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/

generate-schema: restart ## Regenerate schema.rs from database (runs migrations first)
	@docker compose exec server diesel print-schema > server/src/schema.rs
	@echo "Schema generated at server/src/schema.rs"
	$(MAKE) lint

test: generate-clients test-down test-up ## Run tests (tears down on success, leaves up on failure for debugging)
	@docker compose -f docker-compose.test.yml run --build --rm tests && docker compose -f docker-compose.test.yml down || (echo "Tests failed - leaving environment up for debugging" && exit 1)

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
