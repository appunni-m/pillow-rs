# pillow-rs Makefile
# ===================
# Single entry point for all build, test, lint, bench, coverage, and release workflows.
# Run `make` or `make help` to see all targets.

# ── Variables ─────────────────────────────────────────────────────────────────
MATURIN      := maturin
PYTHON       := python3
NODE         := node
CARGO        := cargo
WASM_PACK    := wasm-pack
MANIFEST     := manifest.yaml
PY_SRC       := pillow-rs-py
JS_SRC       := pillow-rs-js
CORE_SRC     := pillow-rs
FIXTURES_DIR := tests/fixtures
REPORT       := /tmp/report.json
TIMEOUT      := 300

# Colors for help output
BOLD := \033[1m
CYAN := \033[36m
NC   := \033[0m

# ── Default ───────────────────────────────────────────────────────────────────
.DEFAULT_GOAL := help
.PHONY: help

# ── Help ──────────────────────────────────────────────────────────────────────
help: ## Show this help
	@printf "$(BOLD)pillow-rs Makefile$(NC)\n"
	@printf "\n$(BOLD)Setup$(NC)\n"
	@printf "  $(CYAN)make setup$(NC)          Install all dev dependencies\n"
	@printf "  $(CYAN)make setup-ci$(NC)       Install deps for CI (no virtualenv check)\n"
	@printf "\n$(BOLD)Build$(NC)\n"
	@printf "  $(CYAN)make build$(NC)          Build Python package (maturin develop --release)\n"
	@printf "  $(CYAN)make build-dev$(NC)      Build Python package (debug, faster compile)\n"
	@printf "  $(CYAN)make build-wasm$(NC)     Build WASM package (dev)\n"
	@printf "  $(CYAN)make build-wasm-release$(NC) Build WASM package (release)\n"
	@printf "  $(CYAN)make build-all$(NC)      Build Python + WASM\n"
	@printf "\n$(BOLD)Test$(NC)\n"
	@printf "  $(CYAN)make test$(NC)           Run all 1,555 PIL parity tests\n"
	@printf "  $(CYAN)make test-suite0$(NC)    Run suite0 only (core functions, fast)\n"
	@printf "  $(CYAN)make test-suite1$(NC)    Run suite1 only\n"
	@printf "  $(CYAN)make test-suite2$(NC)    Run suite2 only\n"
	@printf "  $(CYAN)make test-core$(NC)      Run Rust core tests\n"
	@printf "  $(CYAN)make test-wasm$(NC)      Run WASM/JS tests\n"
	@printf "  $(CYAN)make test-all$(NC)       Run core + Python + WASM tests\n"
	@printf "\n$(BOLD)Fixtures$(NC)\n"
	@printf "  $(CYAN)make fixtures$(NC)       Generate all test fixtures (requires Pillow)\n"
	@printf "  $(CYAN)make fixtures-suite0$(NC) Generate suite0 fixtures only\n"
	@printf "  $(CYAN)make fixtures-clean$(NC) Remove fixture outputs\n"
	@printf "\n$(BOLD)Lint$(NC)\n"
	@printf "  $(CYAN)make fmt$(NC)            Check Rust formatting\n"
	@printf "  $(CYAN)make fmt-fix$(NC)        Fix Rust formatting\n"
	@printf "  $(CYAN)make clippy$(NC)         Run clippy (all targets, all features)\n"
	@printf "  $(CYAN)make lint$(NC)           fmt + clippy\n"
	@printf "\n$(BOLD)Coverage$(NC)\n"
	@printf "  $(CYAN)make coverage$(NC)       Run tests + compute coverage\n"
	@printf "  $(CYAN)make coverage-validate$(NC) Validate coverage against manifest\n"
	@printf "  $(CYAN)make coverage-report$(NC) Generate docs/COVERAGE.md\n"
	@printf "  $(CYAN)make coverage-wasm$(NC)  Generate WASM coverage report\n"
	@printf "\n$(BOLD)Benchmark$(NC)\n"
	@printf "  $(CYAN)make bench$(NC)          Full benchmark suite (166 functions, ~20 min)\n"
	@printf "  $(CYAN)make bench-incr$(NC)     Incremental (only changed functions)\n"
	@printf "  $(CYAN)make bench-priority$(NC) Priority tier only (12 ops)\n"
	@printf "\n$(BOLD)CI$(NC)\n"
	@printf "  $(CYAN)make ci$(NC)             Full CI pipeline (fmt → clippy → test → coverage)\n"
	@printf "\n$(BOLD)Clean$(NC)\n"
	@printf "  $(CYAN)make clean$(NC)          Remove build artifacts and caches\n"
	@printf "  $(CYAN)make clean-all$(NC)      clean + cargo clean\n"
	@printf "\n$(BOLD)Release$(NC)\n"
	@printf "  $(CYAN)make release-pypi$(NC)   Build + publish to PyPI\n"
	@printf "  $(CYAN)make release-npm$(NC)    Build WASM + publish to npm\n"
	@printf "  $(CYAN)make release-crates$(NC) Publish to crates.io\n"
	@printf "\n$(BOLD)Stubs$(NC)\n"
	@printf "  $(CYAN)make stubs$(NC)          Check for missing Rust stubs vs manifest\n"

# ── Setup ─────────────────────────────────────────────────────────────────────
.PHONY: setup setup-ci

setup: ## Install all dev dependencies
	@command -v $(MATURIN) >/dev/null 2>&1 || { echo "Installing maturin..."; pip install maturin; }
	@command -v $(WASM_PACK) >/dev/null 2>&1 || { echo "Installing wasm-pack..."; cargo install wasm-pack; }
	@[ -n "$$VIRTUAL_ENV" ] || [ -n "$$CONDA_PREFIX" ] || echo "⚠️  No virtualenv detected — consider: python3 -m venv .venv && source .venv/bin/activate"
	pip install maturin pillow numpy pyyaml pytest pytest-timeout pytest-json-report pytest-benchmark

setup-ci: ## Install dev deps for CI
	pip install maturin pillow numpy pyyaml pytest pytest-timeout pytest-json-report pytest-benchmark

# ── Build ─────────────────────────────────────────────────────────────────────
.PHONY: build build-dev build-wasm build-wasm-release build-all

build: ## Build Python package (release)
	$(MATURIN) develop --manifest-path $(PY_SRC)/Cargo.toml --release

build-dev: ## Build Python package (debug, faster compile)
	$(MATURIN) develop --manifest-path $(PY_SRC)/Cargo.toml

build-wasm: ## Build WASM package (dev)
	cd $(JS_SRC) && $(WASM_PACK) build --target web --dev

build-wasm-release: ## Build WASM package (release)
	cd $(JS_SRC) && $(WASM_PACK) build --target web --release

build-all: build build-wasm-release ## Build Python + WASM

# ── Test ──────────────────────────────────────────────────────────────────────
.PHONY: test test-suite0 test-suite1 test-suite2 test-core test-wasm test-all

test: ## Run all PIL parity tests
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT)

test-suite0: ## Run suite0 only (core functions)
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT) \
		-k "not suite1 and not suite2 and not suite3"

test-suite1: ## Run suite1 only
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT) -k "suite1"

test-suite2: ## Run suite2 only
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT) -k "suite2"

test-core: ## Run Rust core tests
	$(CARGO) test -p $(CORE_SRC)

test-wasm: ## Run WASM/JS tests
	@[ -f "$(JS_SRC)/tests/run_wasm_test.mjs" ] || { echo "No WASM test runner found"; exit 1; }
	$(NODE) $(JS_SRC)/tests/run_wasm_test.mjs

test-all: test-core test test-wasm ## Run core + Python + WASM tests

# ── Fixtures ──────────────────────────────────────────────────────────────────
.PHONY: fixtures fixtures-suite0 fixtures-suite1 fixtures-clean

fixtures: ## Generate all test fixtures
	$(PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_DIR)

fixtures-suite0: ## Generate suite0 fixtures
	$(PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_DIR) --suite 0

fixtures-suite1: ## Generate suite1 fixtures
	$(PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_DIR) --suite 1

fixtures-clean: ## Remove fixture outputs
	chmod -R u+w $(FIXTURES_DIR)/outputs/ 2>/dev/null || true
	rm -rf $(FIXTURES_DIR)/outputs/
	mkdir -p $(FIXTURES_DIR)/outputs/{jsons,images,raws}

# ── Lint ──────────────────────────────────────────────────────────────────────
.PHONY: fmt fmt-fix clippy clippy-core lint

fmt: ## Check Rust formatting
	$(CARGO) fmt --check

fmt-fix: ## Fix Rust formatting
	$(CARGO) fmt

clippy: ## Run clippy on all targets
	$(CARGO) clippy --all-targets --all-features -- -D warnings

clippy-core: ## Run clippy on core only
	$(CARGO) clippy -p $(CORE_SRC) -- -D warnings

lint: fmt clippy ## Run fmt + clippy

# ── Coverage ──────────────────────────────────────────────────────────────────
.PHONY: coverage coverage-validate coverage-report coverage-wasm

coverage: ## Run tests + compute coverage
	@[ -f "$(REPORT)" ] || { echo "No test report found. Run: make test"; exit 1; }
	$(PYTHON) scripts/coverage/compute_coverage.py $(MANIFEST) $(REPORT)

coverage-validate: ## Validate coverage against manifest (exit 1 on gaps)
	$(PYTHON) scripts/coverage/validate_coverage.py $(MANIFEST)

coverage-report: ## Generate docs/COVERAGE.md
	$(PYTHON) scripts/coverage/generate_multi_backend_coverage.py

coverage-wasm: ## Generate WASM coverage report
	$(PYTHON) scripts/coverage/generate_wasm_coverage.py

# ── Benchmark ─────────────────────────────────────────────────────────────────
.PHONY: bench bench-incr bench-priority

bench: ## Full benchmark suite
	bash scripts/bench/bench_all.sh full

bench-incr: ## Incremental benchmark (only changed functions)
	bash scripts/bench/bench_all.sh incremental

bench-priority: ## Priority tier benchmark (12 ops)
	bash scripts/bench/bench_all.sh --group priority

# ── CI ────────────────────────────────────────────────────────────────────────
.PHONY: ci

ci: ## Full CI pipeline (exit 1 on any failure)
	@echo "=== fmt ===" && $(CARGO) fmt --check
	@echo "=== clippy ===" && $(CARGO) clippy --all-targets --all-features -- -D warnings
	@echo "=== core tests ===" && $(CARGO) test -p $(CORE_SRC)
	@echo "=== fixtures ===" && $(MAKE) fixtures-suite0
	@echo "=== python tests ===" && $(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) --json-report --json-report-file=$(REPORT)
	@echo "=== coverage ===" && $(PYTHON) scripts/coverage/validate_coverage.py $(MANIFEST)
	@echo "=== done ==="

# ── Clean ─────────────────────────────────────────────────────────────────────
.PHONY: clean clean-all

clean: ## Remove build artifacts and caches
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	rm -rf .pytest_cache
	rm -f $(REPORT)
	chmod -R u+w $(FIXTURES_DIR)/outputs/ 2>/dev/null || true
	rm -rf $(FIXTURES_DIR)/outputs/

clean-all: clean ## clean + cargo clean
	$(CARGO) clean

# ── Stubs ─────────────────────────────────────────────────────────────────────
.PHONY: stubs

stubs: ## Check for missing Rust stubs vs manifest
	$(PYTHON) scripts/generate_stubs.py $(MANIFEST)

# ── Release ───────────────────────────────────────────────────────────────────
.PHONY: release-pypi release-npm release-crates

release-pypi: build ## Build + publish to PyPI
	cd $(PY_SRC) && $(MATURIN) publish

release-npm: build-wasm-release ## Build WASM + publish to npm
	cd $(JS_SRC)/pkg && npm publish

release-crates: ## Publish to crates.io
	$(CARGO) publish -p $(CORE_SRC)
