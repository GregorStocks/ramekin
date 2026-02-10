.PHONY: help dev dev-headless dev-down check-deps lint clean clean-api generate-schema test test-ui venv venv-clean db-up db-down db-clean seed load-test install-hooks setup-claude-web screenshots generate-test-urls pipeline pipeline-cache-stats pipeline-cache-clear ios-generate ios-build ingredient-tests-generate ingredient-tests-update ingredient-tests-generate-paprika ingredient-tests-migrate-curated ingredient-density-test ingredient-density-import

# Use bash with pipefail so piped commands propagate exit codes
SHELL := /bin/bash
.SHELLFLAGS := -o pipefail -c

# Timestamp wrapper for log output
TS := ./scripts/ts

# Source files that affect the OpenAPI spec
API_SOURCES := $(shell find server/src/api -type f -name '*.rs' 2>/dev/null) server/src/models.rs server/src/schema.rs

# Marker file for generated clients
CLIENT_MARKER := cli/generated/ramekin-client/Cargo.toml

# Default simulator target for iOS UI tests (override with IOS_UI_DESTINATION)
IOS_UI_DESTINATION ?= platform=iOS Simulator,name=iPhone 15,OS=latest

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: check-deps db-up $(CLIENT_MARKER) ## Start local dev environment (server + UI via process-compose)
	@echo "Starting dev environment (Ctrl+C to stop)..."
	@mkdir -p logs
	@process-compose up -e dev.env --port 8180

dev-headless: check-deps db-up $(CLIENT_MARKER) ## Start local dev environment without TUI
	@echo "Starting dev environment (headless)..."
	@mkdir -p logs
	@process-compose up -e dev.env -t=false --port 8180

dev-down: ## Stop dev processes (not database)
	@process-compose down 2>/dev/null || true
	@pkill -f "cargo watch" 2>/dev/null || true

# Generate OpenAPI spec from Rust source
api/openapi.json: $(API_SOURCES)
	@echo "Building server and generating OpenAPI spec..." | $(TS)
	@mkdir -p api
	@cd server && cargo build --release -q
	@server/target/release/ramekin-server --openapi > api/openapi.json
	@echo "Generated api/openapi.json" | $(TS)

# Generate clients from OpenAPI spec
$(CLIENT_MARKER): api/openapi.json
	@./scripts/generate-clients.sh
	@cd cli && cargo fmt --all -q 2>/dev/null || true

lint: venv $(CLIENT_MARKER) ## Run all linters (Rust, TypeScript, Python)
	@bash -o pipefail -c 'PATH="$(CURDIR)/.venv/bin:$$PATH" ./scripts/lint.py 2>&1 | $(TS)'

clean: ## Clean generated files and build artifacts
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/
	@rm -rf server/target/ cli/target/
	@rm -rf ramekin-ui/node_modules/
	@rm -rf tests/__pycache__/ scripts/__pycache__/
	@rm -rf .cache/ logs/

clean-api: ## Force regeneration of OpenAPI spec and clients on next build
	@rm -f api/openapi.json
	@rm -rf cli/generated/ ramekin-ui/generated-client/ tests/generated/

generate-schema: ## Regenerate schema.rs from database (requires db-up and migrations run)
	@cd server && diesel print-schema > src/schema.rs
	@echo "Schema generated at server/src/schema.rs" | $(TS)
	$(MAKE) lint

setup-claude-web: ## Setup environment for Claude Code for Web (no-op elsewhere)
	@./scripts/setup-claude-web.sh

cli/target/debug/ramekin-cli: $(CLIENT_MARKER)
	cd cli && cargo build

test: check-deps $(CLIENT_MARKER) cli/target/debug/ramekin-cli ## Run API tests
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-tests.sh

test-ui: check-deps $(CLIENT_MARKER) ## Run UI tests with Playwright (requires DATABASE_URL)
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-ui-tests.sh

.venv/.installed: requirements-test.txt
	@uv venv
	@uv pip install -r requirements-test.txt
	@touch .venv/.installed

venv: .venv/.installed ## Create Python venv with test dependencies

check-deps: venv setup-claude-web ## Check that all dependencies are installed
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/check-deps.sh

venv-clean: ## Remove Python venv
	@rm -rf .venv

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
	@# Create workspace-specific databases from dev.env and test.env if they exist
	@if [ -f dev.env ]; then \
	  DEV_DB=$$(grep '^DATABASE_URL=' dev.env | sed 's|.*/||'); \
	  if [ -n "$$DEV_DB" ]; then \
	    if docker exec ramekin-db createdb -U ramekin "$$DEV_DB" 2>/dev/null; then \
	      echo "Created database: $$DEV_DB"; \
	    fi; \
	  fi; \
	fi
	@if [ -f test.env ]; then \
	  TEST_DB=$$(grep '^DATABASE_URL=' test.env | sed 's|.*/||'); \
	  if [ -n "$$TEST_DB" ]; then \
	    if docker exec ramekin-db createdb -U ramekin "$$TEST_DB" 2>/dev/null; then \
	      echo "Created database: $$TEST_DB"; \
	    fi; \
	  fi; \
	fi

db-down: ## Stop postgres container
	@docker stop ramekin-db >/dev/null 2>&1 || true

db-clean: db-down ## Stop postgres and remove data
	@docker rm ramekin-db >/dev/null 2>&1 || true

seed: ## Create test user with sample recipes (requires dev server running)
	@cd cli && cargo run -q -- seed --username t --password t ../data/dev/seed.paprikarecipes

load-test: ## Run load test creating users with recipes and photos (for performance testing)
	@cd cli && cargo run -q -- load-test

install-hooks: ## Install git hooks for local development
	@cp scripts/pre-push .git/hooks/pre-push
	@chmod +x .git/hooks/pre-push
	@echo "Git hooks installed successfully"

screenshots: check-deps $(CLIENT_MARKER) ## Take screenshots for visual testing
	@PC_EXIT_ON_END=true SERVER_CMD="./target/release/ramekin-server" SERVER_RESTART=exit_on_failure process-compose up -e dev.env -t=false --port 8180 || true
	@test -f logs/cookbook.png || (echo "Screenshots not found" && exit 1)

generate-test-urls: ## Generate test URL list from top recipe sites
	@cargo run -q --manifest-path cli/Cargo.toml -- generate-test-urls -o data/test-urls.json \
		--merge \
		$(if $(SITE),--site $(SITE),) \
		$(if $(MIN_YEAR),--min-year $(MIN_YEAR),) \
		$(if $(NO_LIMIT),--no-limit,)

pipeline: ## Run pipeline for test URLs and generate reports (offline by default, use OFFLINE=false to enable network)
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- pipeline \
		$(if $(LIMIT),--limit $(LIMIT),) \
		$(if $(SITE),--site $(SITE),) \
		$(if $(DELAY),--delay-ms $(DELAY),) \
		$(if $(OFFLINE),--offline=$(OFFLINE),) \
		$(if $(FORCE_REFETCH),--force-refetch,) \
		$(if $(ON_FETCH_FAIL),--on-fetch-fail $(ON_FETCH_FAIL),) \
		$(if $(TAGS_FILE),--tags-file $(TAGS_FILE),) \
		$(if $(CONCURRENCY),--concurrency $(CONCURRENCY),)
	@$(MAKE) ingredient-tests-generate

pipeline-cache-stats: ## Show HTML cache statistics
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- pipeline-cache-stats

pipeline-cache-clear: ## Clear HTML cache
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- pipeline-cache-clear

ios-generate: ## Generate Xcode project for iOS app (requires xcodegen: brew install xcodegen)
	@cd ramekin-ios && xcodegen generate
	@echo "Xcode project generated at ramekin-ios/Ramekin.xcodeproj"
	@echo "Open with: open ramekin-ios/Ramekin.xcodeproj"

ios-build: ## Build iOS app for simulator
	@cd ramekin-ios && xcodebuild -project Ramekin.xcodeproj -scheme Ramekin -destination 'generic/platform=iOS Simulator' build

ios-test-ui: ios-generate ## Run iOS UI tests (requires dev server running)
	@rm -rf logs/ios-ui-tests.xcresult
	@cd ramekin-ios && xcodebuild test \
		-project Ramekin.xcodeproj \
		-scheme Ramekin \
		-destination '$(IOS_UI_DESTINATION)' \
		-only-testing:RamekinUITests \
		-resultBundlePath ../logs/ios-ui-tests.xcresult \
		CODE_SIGNING_ALLOWED=NO
	@echo "UI test results at logs/ios-ui-tests.xcresult"

ingredient-tests-generate: ## Generate ingredient parsing test fixtures from latest pipeline run
	@cargo run -q --manifest-path cli/Cargo.toml -- ingredient-tests-generate

ingredient-tests-update: ## Update ingredient parsing test fixtures to match current parser output
	@cargo run -q --manifest-path cli/Cargo.toml -- ingredient-tests-update

ingredient-tests-generate-paprika: ## Generate ingredient parsing test fixtures from paprikarecipes file
	@cargo run -q --manifest-path cli/Cargo.toml -- ingredient-tests-generate-paprika

ingredient-tests-migrate-curated: ## Migrate curated fixtures from individual files to category files
	@cargo run -q --manifest-path cli/Cargo.toml -- ingredient-tests-migrate-curated

ingredient-density-test: ## Run ingredient-density crate tests
	@cd ingredient-density && cargo test

ingredient-density-import: ## Regenerate USDA data from downloaded CSV (requires USDA data download)
	@cd ingredient-density && cargo run --bin import_usda
