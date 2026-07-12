.PHONY: help dev dev-headless dev-down serve serve-down check-deps check-lint-deps check-venv-deps check-lockfile lint clean clean-api generate-clients check-client-generation generate-schema test test-core test-ui ui-deps ui-unit-test pretool-hook-test venv venv-clean python-test-deps-update db-up db-down db-clean db-migrate seed load-test install-hooks setup-claude-web worktree-setup generate-test-urls refilter-test-urls pipeline pipeline-cache-stats pipeline-cache-clear pipeline-cache-capture ios-generate ios-build ios-install ios-test ios-test-ui ingredient-tests-generate ingredient-tests-update ingredient-tests-generate-paprika ingredient-tests-migrate-curated ingredient-density-test ingredient-density-import shopping-list-categorizer-test title-normalization-test description-generation-test server-release-build

# Use bash with pipefail so piped commands propagate exit codes
SHELL := /bin/bash
.SHELLFLAGS := -o pipefail -c

# Timestamp wrapper for log output
TS := ./scripts/ts

# Rust source files can expose types that affect the OpenAPI spec indirectly.
API_SOURCES := $(shell find server/src -type f -name '*.rs' 2>/dev/null)

# Marker file for generated clients
CLIENT_MARKER := cli/generated/ramekin-client/Cargo.toml
UI_DEPS_MARKER := ramekin-ui/node_modules/.package-lock.json
GENERATED_API_PATHS := api/openapi.json \
	cli/generated \
	ramekin-ui/generated-client \
	tests/generated \
	ramekin-ios/generated-client
SERVER_RELEASE_BIN := server/target/release/ramekin-server
RAMEKIN_IOS_APPLINKS_URL ?= https://ramekin.app

# Simulator destination for iOS tests. Leave empty to auto-detect the newest
# iPhone on the newest installed iOS runtime via scripts/find-ios-simulator.py.
# Override with IOS_TEST_DESTINATION / IOS_UI_DESTINATION to pin a device.
IOS_TEST_DESTINATION ?=
IOS_UI_DESTINATION ?= $(IOS_TEST_DESTINATION)

# Shell snippet that expands $$DEST to a usable -destination value. Uses the
# override if set, otherwise asks simctl. Sourcing `xcrun simctl list` also
# forces CoreSimulator to resync after macOS updates.
ios_resolve_destination = \
	if [ -n "$(1)" ]; then DEST="$(1)"; \
	else UDID=$$(xcrun simctl list devices available -j | python3 scripts/find-ios-simulator.py); \
	     DEST="platform=iOS Simulator,id=$$UDID"; fi; \
	echo "Using iOS destination: $$DEST"

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@awk 'BEGIN {FS = ":.*?## "} /^[a-zA-Z_-]+:.*?## / {printf "  %-20s %s\n", $$1, $$2}' $(MAKEFILE_LIST)

dev: check-deps db-up $(CLIENT_MARKER) ## Start local dev environment (server + UI via process-compose)
	@echo "Starting dev environment (Ctrl+C to stop)..."
	@mkdir -p logs
	@set -a && . ./dev.env && set +a && process-compose up -e dev.env --port "$${PROCESS_COMPOSE_PORT:-8180}"

dev-headless: check-deps db-up $(CLIENT_MARKER) ## Start local dev environment without TUI
	@echo "Starting dev environment (headless)..."
	@mkdir -p logs
	@set -a && . ./dev.env && set +a && process-compose up -e dev.env -t=false --port "$${PROCESS_COMPOSE_PORT:-8180}"

serve: check-deps db-up $(CLIENT_MARKER) server-release-build ## Start release-mode server with socket activation and a memory cap
	@echo "Starting release server..."
	@mkdir -p logs
	@set -a && . ./dev.env && set +a && process-compose up -e dev.env -f serve-compose.yaml -t=false --port "$${PROCESS_COMPOSE_PORT:-8180}"

dev-down: ## Stop dev processes (not database)
	@if [ -f dev.env ]; then set -a && . ./dev.env && set +a; fi; \
	port="$${PROCESS_COMPOSE_PORT:-8180}"; \
	if process-compose project state --port "$$port" >/dev/null 2>&1; then \
		process-compose down --port "$$port"; \
	else \
		echo "No process-compose instance running on port $$port"; \
	fi
	@pkill -f "cargo watch" 2>/dev/null || true

serve-down: ## Stop release-mode serve processes
	@if [ -f dev.env ]; then set -a && . ./dev.env && set +a; fi; \
	port="$${PROCESS_COMPOSE_PORT:-8180}"; \
	if process-compose project state --port "$$port" >/dev/null 2>&1; then \
		process-compose down --port "$$port"; \
	else \
		echo "No process-compose instance running on port $$port"; \
	fi
	@pkill -f "systemfd --no-pid" 2>/dev/null || true

# Generate OpenAPI spec from Rust source
api/openapi.json: $(API_SOURCES)
	@echo "Building server and generating OpenAPI spec..." | $(TS)
	@mkdir -p api
	@$(MAKE) server-release-build
	@$(SERVER_RELEASE_BIN) --openapi > api/openapi.json
	@echo "Generated api/openapi.json" | $(TS)

server-release-build:
	@cd server && cargo build --release -q

# Install the lockfile-pinned UI toolchain used to compile the generated client
$(UI_DEPS_MARKER): ramekin-ui/package.json ramekin-ui/package-lock.json
	@cd ramekin-ui && npx --yes -p npm@latest npm ci --silent

ui-deps: $(UI_DEPS_MARKER) ## Install lockfile-pinned UI dependencies

# Generate clients from OpenAPI spec
generate-clients: api/openapi.json $(UI_DEPS_MARKER) ## Regenerate all API clients with pinned tools
	@./scripts/generate-clients.sh
	@cd cli && cargo fmt --all -q

$(CLIENT_MARKER): api/openapi.json $(UI_DEPS_MARKER)
	@$(MAKE) generate-clients

check-client-generation: $(UI_DEPS_MARKER) ## Regenerate API clients and fail if committed output drifts
	@git diff --quiet -- $(GENERATED_API_PATHS) || \
		{ echo "Generated API artifacts must be clean before checking" >&2; exit 1; }
	@git diff --cached --quiet -- $(GENERATED_API_PATHS) || \
		{ echo "Generated API artifacts must be clean before checking" >&2; exit 1; }
	@$(MAKE) -B api/openapi.json
	@$(MAKE) generate-clients
	@git diff --exit-code -- $(GENERATED_API_PATHS)

check-lint-deps: ## Check that the tools needed by make lint are installed
	@./scripts/check-deps.sh --lint

lint: check-lint-deps venv $(CLIENT_MARKER) ## Run all linters (Rust, TypeScript, Python)
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

generate-schema: db-migrate ## Regenerate schema.rs from the migrated dev database
	@cd server && DATABASE_URL=$$(grep '^DATABASE_URL=' ../dev.env | cut -d= -f2-) \
	    diesel print-schema --no-generate-missing-sql-type-definitions > src/schema.rs.tmp \
	    && mv src/schema.rs.tmp src/schema.rs \
	    || { rm -f src/schema.rs.tmp; echo "diesel print-schema failed; schema.rs left unchanged" >&2; exit 1; }
	@echo "Schema generated at server/src/schema.rs" | $(TS)
	$(MAKE) lint

setup-claude-web: ## Setup environment for Claude Code for Web (no-op elsewhere)
	@./scripts/setup-claude-web.sh

worktree-setup: ## Generate dev.env and test.env for this worktree
	@./scripts/worktree-setup.py \
		$(if $(WORKSPACE_NAME),--workspace-name $(WORKSPACE_NAME),) \
		$(if $(BASE_PORT),--base-port $(BASE_PORT),) \
		$(if $(FORCE),--force,)

cli/target/debug/ramekin-cli: $(CLIENT_MARKER)
	cd cli && cargo build

test: check-deps $(CLIENT_MARKER) cli/target/debug/ramekin-cli server-release-build ## Run API tests
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-tests.sh

test-core: ## Run ramekin-core unit and fixture tests (no dev environment required)
	@cargo test -q --manifest-path ramekin-core/Cargo.toml

test-ui: check-deps $(CLIENT_MARKER) ## Run UI tests with Playwright (requires DATABASE_URL)
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/run-ui-tests.sh

ui-unit-test: $(CLIENT_MARKER) ## Run web unit tests (Vitest)
	@cd ramekin-ui && if [ ! -x node_modules/.bin/vitest ]; then npx --yes -p npm@latest npm ci --silent; fi && npx vitest run

pretool-hook-test: venv $(CLIENT_MARKER) ## Run repo PreToolUse hook policy tests
	@PYTHONPATH="$(CURDIR)/tests:$(CURDIR)/tests/generated" \
	    PATH="$(CURDIR)/.venv/bin:$(PATH)" \
	    python3 -m pytest tests/test_pretool_hook_config.py

check-venv-deps:
	@./scripts/check-deps.sh --venv

# Gates consuming the lock, not producing it, so python-test-deps-update stays
# runnable when the lock is what's stale.
check-lockfile:
	@./scripts/check-deps.sh --lockfile

# Depends on the lockfile alone: `uv pip sync` reads only the lock, and a rule
# building the lock from requirements-test.txt would let a git checkout's mtimes
# trigger an implicit `--upgrade` during an ordinary `make test`.
.venv/.installed: requirements-test.lock | check-venv-deps check-lockfile
	@test -d .venv || uv venv
	@uv pip sync requirements-test.lock
	@touch .venv/.installed

venv: .venv/.installed ## Create Python venv with test dependencies

python-test-deps-update: check-venv-deps ## Refresh pinned Python test dependencies
	@uv pip compile --universal --upgrade requirements-test.txt \
	    --output-file requirements-test.lock \
	    --custom-compile-command "make python-test-deps-update"

check-deps: venv setup-claude-web ## Check that all dependencies are installed
	@PATH="$(CURDIR)/.venv/bin:$(PATH)" ./scripts/check-deps.sh

venv-clean: ## Remove Python venv
	@rm -rf .venv

db-up: ## Start postgres container with dev and test databases
	@if docker ps --format '{{.Names}}' | grep -q '^ramekin-db$$'; then \
	  echo "Postgres already running."; \
	elif docker ps -a --format '{{.Names}}' | grep -q '^ramekin-db$$'; then \
	  echo "Starting existing postgres container..."; \
	  docker start ramekin-db >/dev/null; \
	  echo "Waiting for postgres..."; \
	  until docker exec ramekin-db pg_isready -U ramekin >/dev/null 2>&1; do sleep 0.2; done; \
	  echo "Postgres ready on localhost:54321"; \
	else \
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

db-migrate: db-up ## Apply pending diesel migrations against the dev database
	@cd server && DATABASE_URL=$$(grep '^DATABASE_URL=' ../dev.env | cut -d= -f2-) \
	    diesel migration run

seed: ## Create test user with sample recipes (requires dev server running)
	@cd cli && cargo run -q -- seed --username t --password t ../data/dev/seed.paprikarecipes

load-test: ## Run load test creating users with recipes and photos (for performance testing)
	@cd cli && cargo run -q -- load-test

install-hooks: ## Install git hooks for local development
	@cp scripts/pre-push .git/hooks/pre-push
	@chmod +x .git/hooks/pre-push
	@echo "Git hooks installed successfully"

generate-test-urls: ## Generate test URL list from top recipe sites
	@cargo run -q --manifest-path cli/Cargo.toml -- generate-test-urls -o data/test-urls.json \
		$(if $(SITE),--site $(SITE),) \
		$(if $(MIN_YEAR),--min-year $(MIN_YEAR),) \
		$(if $(NO_LIMIT),--no-limit,)

refilter-test-urls: ## Refilter existing test URLs through current filter logic
	@cargo run -q --manifest-path cli/Cargo.toml -- generate-test-urls --refilter

pipeline: ## Run the full pipeline over every URL in test-urls.json
	@./scripts/run-pipeline.sh \
		$(if $(DELAY),--delay-ms $(DELAY),) \
		$(if $(FORCE_REFETCH),--force-refetch,) \
		$(if $(ON_FETCH_FAIL),--on-fetch-fail $(ON_FETCH_FAIL),) \
		$(if $(CONCURRENCY),--concurrency $(CONCURRENCY),)

pipeline-cache-stats: ## Show HTML cache statistics
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- pipeline-cache-stats

pipeline-cache-clear: ## Clear HTML cache
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- pipeline-cache-clear

pipeline-cache-capture: ## Run a localhost server + bookmarklet to manually save bot-walled URLs into the pipeline cache
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- pipeline-cache-capture \
		$(if $(HOST),--host $(HOST),) \
		$(if $(PORT),--port $(PORT),) \
		$(if $(URL),--url $(URL),)

ios-generate: ## Generate Xcode project for iOS app (requires xcodegen: brew install xcodegen)
	@APPLINKS_URL="$(RAMEKIN_IOS_APPLINKS_URL)" && \
	RAMEKIN_EXTERNAL_HOST=$$(echo "$$APPLINKS_URL" | sed -E 's|^[a-z]+://||; s|[:/].*$$||') && \
	if [ -z "$$RAMEKIN_EXTERNAL_HOST" ]; then \
	  echo "RAMEKIN_IOS_APPLINKS_URL must include a host" >&2; \
	  exit 1; \
	fi && \
	export RAMEKIN_EXTERNAL_HOST && \
	echo "Using applinks host: $$RAMEKIN_EXTERNAL_HOST" && \
	cd ramekin-ios && xcodegen generate
	@echo "Xcode project generated at ramekin-ios/Ramekin.xcodeproj"
	@echo "Open with: open ramekin-ios/Ramekin.xcodeproj"

ios-build: ios-generate ## Build iOS app for simulator
	@cd ramekin-ios && xcodebuild -project Ramekin.xcodeproj -scheme Ramekin -destination 'generic/platform=iOS Simulator' build

ios-install: ios-generate ## Build and install iOS app on connected device
	@cd ramekin-ios && xcodebuild -project Ramekin.xcodeproj -scheme Ramekin \
		-destination 'generic/platform=iOS' \
		-derivedDataPath build \
		build
	@APP_PATH=$$(find ramekin-ios/build/Build/Products -name "Ramekin.app" -path "*/Debug-iphoneos/*" | head -1) && \
		DEVICE_ID=$$(xcrun devicectl list devices 2>/dev/null | grep -oE '[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}' | head -1) && \
		if [ -z "$$DEVICE_ID" ]; then echo "No connected device found" && exit 1; fi && \
		echo "Installing to device $$DEVICE_ID..." && \
		xcrun devicectl device install app --device "$$DEVICE_ID" "$$APP_PATH"

# Simulator test builds must stay (ad-hoc) code signed: without a signature
# the keychain access-group entitlement is missing, every keychain operation
# fails, and UI-test login never completes. Never pass CODE_SIGNING_ALLOWED=NO.
ios-test: ios-generate ## Run iOS unit tests
	@mkdir -p logs
	@rm -rf logs/ios-tests.xcresult
	@$(call ios_resolve_destination,$(IOS_TEST_DESTINATION)); \
	cd ramekin-ios && xcodebuild test \
		-project Ramekin.xcodeproj \
		-scheme Ramekin \
		-destination "$$DEST" \
		-only-testing:RamekinTests \
		-resultBundlePath ../logs/ios-tests.xcresult
	@echo "Unit test results at logs/ios-tests.xcresult"

ios-test-ui: ios-generate ## Run iOS UI tests (requires dev server running)
	@mkdir -p logs
	@rm -rf logs/ios-ui-tests.xcresult
	@$(call ios_resolve_destination,$(IOS_UI_DESTINATION)); \
	cd ramekin-ios && xcodebuild test \
		-project Ramekin.xcodeproj \
		-scheme Ramekin \
		-destination "$$DEST" \
		-only-testing:RamekinUITests \
		-resultBundlePath ../logs/ios-ui-tests.xcresult
	@echo "UI test results at logs/ios-ui-tests.xcresult"

ingredient-tests-generate: ## Generate ingredient parsing test fixtures from latest pipeline run
	@cargo run -q --release --manifest-path cli/Cargo.toml -- ingredient-tests-generate

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

shopping-list-categorizer-test: ## Score the categorizer against the prod shopping-list corpus (reports mismatches + 'Other' rate)
	@cargo test -q --manifest-path ramekin-core/Cargo.toml --test shopping_list_categorizer_tests -- --nocapture

title-normalization-test: ## Normalize recipe titles from seed.paprikarecipes via the LLM (cached; free on rerun)
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- title-normalization-test \
		$(if $(FILE),--file $(FILE),) \
		$(if $(LIMIT),--limit $(LIMIT),)

description-generation-test: ## Generate menu-style descriptions from seed.paprikarecipes via the LLM (cached; free on rerun)
	@set -a && [ -f cli.env ] && . ./cli.env; set +a && \
	cargo run -q --manifest-path cli/Cargo.toml -- description-generation-test \
		$(if $(FILE),--file $(FILE),) \
		$(if $(LIMIT),--limit $(LIMIT),)
