.PHONY: help lint fmt check test build docker-up docker-down clean

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-15s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

lint: ## Run all linters (rustfmt + clippy)
	./scripts/lint.sh

fmt: ## Format code with rustfmt
	cargo fmt --all

check: ## Check code without building
	cargo check --all-targets

test: ## Run tests
	cargo test --all

build: ## Build all packages
	cargo build --all

docker-up: ## Start docker containers
	docker compose up -d

docker-down: ## Stop docker containers
	docker compose down

docker-logs: ## Show docker logs
	docker compose logs -f

clean: ## Clean build artifacts
	cargo clean
	docker compose down -v
