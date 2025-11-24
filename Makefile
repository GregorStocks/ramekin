.PHONY: help dev up down restart logs logs-server logs-db generate-clients generate-rust-client generate-ts-client lint clean generate-schema

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: ## Start dev environment (with hot-reload)
	docker compose up --build -d

up: dev ## Alias for dev

down: ## Stop all services
	docker compose down

restart: ## Force restart services
	docker compose up --build -d --force-recreate

logs: ## Show all logs
	docker compose logs -f

logs-server: ## Show server logs
	docker logs ramekin-server -f

logs-db: ## Show database logs
	docker logs ramekin-postgres -f

generate-clients: generate-rust-client generate-ts-client ## Regenerate both API clients

generate-rust-client: ## Regenerate Rust CLI client
	cd crates/ramekin-cli && ./generate-client.sh

generate-ts-client: ## Regenerate TypeScript UI client
	cd ramekin-ui && ./generate-client.sh

lint: ## Run clippy linter
	cargo fmt --all
	cargo clippy --all-targets --all-features -- -D warnings

clean: ## Stop services and clean volumes
	docker compose down -v

generate-schema: restart ## Regenerate schema.rs from database (runs migrations first)
	@echo "Waiting for server to run migrations..."
	@sleep 30
	@docker compose exec server diesel print-schema > crates/ramekin-server/src/schema.rs
	@echo "Schema generated at crates/ramekin-server/src/schema.rs"
