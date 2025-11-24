.PHONY: help dev up down restart logs logs-server logs-db generate-clients generate-rust-client generate-ts-client lint clean

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: ## Start dev environment (with hot-reload)
	docker compose up

up: dev ## Alias for dev

down: ## Stop all services
	docker compose down

restart: ## Restart services
	docker compose restart

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
