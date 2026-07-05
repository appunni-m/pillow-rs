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
FONTDONE_SRC := pillow-rs-freetype
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
	@printf "  $(CYAN)make test$(NC)           Run all PIL parity tests\n"
	@printf "  $(CYAN)make test-suite0$(NC)    Run suite0 only (core functions, fast)\n"
	@printf "  $(CYAN)make test-suite1$(NC)    Run suite1 only\n"
	@printf "  $(CYAN)make test-suite2$(NC)    Run suite2 only\n"
	@printf "  $(CYAN)make test-core$(NC)      Run Rust core tests\n"
	@printf "  $(CYAN)make test-wasm$(NC)      Run WASM/JS tests\n"
	@printf "  $(CYAN)make test-all$(NC)       Run core + Python + WASM tests\n"
	@printf "\n$(BOLD)fontdone / FreeType parity$(NC)\n"
	@printf "  $(CYAN)make fontdone-help$(NC)  Show crate-local fontdone targets\n"
	@printf "  $(CYAN)make fontdone-ci$(NC)    Run fontdone docs, lint, tests, parity, FFI, bench contracts\n"
	@printf "  $(CYAN)make fontdone-test$(NC)  Run all fontdone tests\n"
	@printf "  $(CYAN)make fontdone-parity$(NC) Run the FreeType coverage matrix harness\n"
	@printf "  $(CYAN)make fontdone-ffi$(NC)   Run the no-runtime-FFI guard\n"
	@printf "  $(CYAN)make fontdone-doc$(NC)   Build strict fontdone rustdoc\n"
	@printf "  $(CYAN)make fontdone-bench$(NC) Run Rust vs C FreeType benchmark report\n"
	@printf "  $(CYAN)make fontdone-fixtures$(NC) Regenerate FreeType fixture families\n"
	@printf "\n$(BOLD)Fixtures$(NC)\n"
	@printf "  $(CYAN)make fixtures$(NC)       Generate all test fixtures (requires Pillow)\n"
	@printf "  $(CYAN)make imagingft-fixtures$(NC) Generate ignored PIL imagingft fixture matrix\n"
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
	@printf "\n$(BOLD)Docs$(NC)\n"
	@printf "  $(CYAN)make repo-map-check$(NC) Validate docs/REPO_MAP.md generated tree\n"
	@printf "  $(CYAN)make repo-map-update$(NC) Refresh docs/REPO_MAP.md generated tree\n"
	@printf "\n$(BOLD)Benchmark$(NC)\n"
	@printf "  $(CYAN)make bench$(NC)          Full benchmark suite (166 functions, ~20 min)\n"
	@printf "  $(CYAN)make bench-incr$(NC)     Incremental (only changed functions)\n"
	@printf "  $(CYAN)make bench-priority$(NC) Priority tier only (12 ops)\n"
	@printf "\n$(BOLD)CI$(NC)\n"
	@printf "  $(CYAN)make ci$(NC)             Full CI pipeline (fmt → clippy → test → coverage)\n"
	@printf "  $(CYAN)make verify$(NC)         Full workspace CI plus FreeType CI\n"
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

# ── fontdone / FreeType parity ───────────────────────────────────────────────
.PHONY: fontdone-help fontdone-build fontdone-doc fontdone-doc-test
.PHONY: fontdone-test fontdone-parity fontdone-ffi fontdone-lint
.PHONY: fontdone-fmt fontdone-fmt-fix fontdone-clippy
.PHONY: fontdone-bench fontdone-bench-quick fontdone-bench-self-test
.PHONY: fontdone-fixtures fontdone-ci fontdone-clean
.PHONY: freetype-help freetype-build freetype-doc freetype-doc-test
.PHONY: freetype-test freetype-parity freetype-ffi freetype-lint
.PHONY: freetype-fmt freetype-fmt-fix freetype-clippy
.PHONY: freetype-bench freetype-bench-quick freetype-bench-self-test
.PHONY: freetype-fixtures freetype-ci freetype-clean

fontdone-help: ## Show pillow-rs-freetype targets
	$(MAKE) -C $(FONTDONE_SRC) help

fontdone-build: ## Build fontdone
	$(MAKE) -C $(FONTDONE_SRC) build

fontdone-doc: ## Build strict fontdone rustdoc
	$(MAKE) -C $(FONTDONE_SRC) doc

fontdone-doc-test: ## Run fontdone doctests
	$(MAKE) -C $(FONTDONE_SRC) doc-test

fontdone-test: ## Run all fontdone tests
	$(MAKE) -C $(FONTDONE_SRC) test

fontdone-parity: ## Run FreeType parity matrix tests
	$(MAKE) -C $(FONTDONE_SRC) test-parity

fontdone-ffi: ## Run no-runtime-FFI guard
	$(MAKE) -C $(FONTDONE_SRC) test-ffi

fontdone-fmt: ## Check fontdone formatting
	$(MAKE) -C $(FONTDONE_SRC) fmt

fontdone-fmt-fix: ## Apply fontdone formatting
	$(MAKE) -C $(FONTDONE_SRC) fmt-fix

fontdone-clippy: ## Run strict fontdone clippy
	$(MAKE) -C $(FONTDONE_SRC) clippy

fontdone-lint: ## Run fontdone fmt + clippy
	$(MAKE) -C $(FONTDONE_SRC) lint

fontdone-bench: ## Run Rust vs C FreeType benchmark report
	$(MAKE) -C $(FONTDONE_SRC) bench

fontdone-bench-quick: ## Run short FreeType benchmark smoke comparison
	$(MAKE) -C $(FONTDONE_SRC) bench-quick

fontdone-bench-self-test: ## Run benchmark tooling self-test
	$(MAKE) -C $(FONTDONE_SRC) bench-self-test

fontdone-fixtures: ## Regenerate all FreeType fixture families
	$(MAKE) -C $(FONTDONE_SRC) fixtures

fontdone-ci: ## Run required fontdone local CI sequence
	$(MAKE) -C $(FONTDONE_SRC) ci

fontdone-clean: ## Clean fontdone artifacts
	$(MAKE) -C $(FONTDONE_SRC) clean

freetype-help: fontdone-help
freetype-build: fontdone-build
freetype-doc: fontdone-doc
freetype-doc-test: fontdone-doc-test
freetype-test: fontdone-test
freetype-parity: fontdone-parity
freetype-ffi: fontdone-ffi
freetype-fmt: fontdone-fmt
freetype-fmt-fix: fontdone-fmt-fix
freetype-clippy: fontdone-clippy
freetype-lint: fontdone-lint
freetype-bench: fontdone-bench
freetype-bench-quick: fontdone-bench-quick
freetype-bench-self-test: fontdone-bench-self-test
freetype-fixtures: fontdone-fixtures
freetype-ci: fontdone-ci
freetype-clean: fontdone-clean

# ── Fixtures ──────────────────────────────────────────────────────────────────
.PHONY: fixtures imagingft-fixtures fixtures-suite0 fixtures-suite1 fixtures-clean

fixtures: ## Generate all test fixtures
	$(PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_DIR)

imagingft-fixtures: ## Generate ignored PIL imagingft fixture matrix
	$(PYTHON) pillow-rs/scripts/build_imagingft_fixtures.py

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
	$(CARGO) clippy --all-targets --all-features -- -A deprecated

clippy-core: ## Run clippy on core only
	$(CARGO) clippy -p $(CORE_SRC) -- -A deprecated

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

# ── Documentation ─────────────────────────────────────────────────────────────
.PHONY: repo-map-check repo-map-update

repo-map-check: ## Validate docs/REPO_MAP.md generated tree
	$(PYTHON) scripts/check_repo_map.py

repo-map-update: ## Refresh docs/REPO_MAP.md generated tree
	$(PYTHON) scripts/check_repo_map.py --write

# ── CI ────────────────────────────────────────────────────────────────────────
.PHONY: ci verify

ci: repo-map-check fmt clippy test-core fixtures-suite0 test coverage-validate ## Full CI pipeline
	@echo "=== done ==="

verify: ci fontdone-ci ## Full workspace CI plus FreeType CI
	@echo "=== all verification done ==="

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
