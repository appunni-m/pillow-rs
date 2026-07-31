# pillow-rs Makefile
# ===================
# Single entry point for all build, test, lint, bench, coverage, and release workflows.
# Run `make` or `make help` to see all targets.

# ── Variables ─────────────────────────────────────────────────────────────────
PYTHON       ?= $(shell if [ -x .venv/bin/python ]; then printf '%s' .venv/bin/python; else printf '%s' python3; fi)
PIP          := $(PYTHON) -m pip
MATURIN      := $(PYTHON) -m maturin
NODE         := node
CARGO        := cargo
WASM_PACK    := wasm-pack
MANIFEST     := pillow-rs/tests/fixtures/manifest.yaml
PY_SRC       := pillow-rs-py
JS_SRC       := pillow-rs-js
CORE_SRC     := pillow-rs
FONTDONE_SRC := ../fontdone
IMAGE_SLASH_STAR_SRC := $(abspath ../image-slash-star)
IMAGE_SLASH_STAR_AVIF_LIB_DIR ?= $(shell p="$$(find "$(IMAGE_SLASH_STAR_SRC)/.oracle-venv" -name 'libavif*' -type f -print -quit 2>/dev/null)"; if [ -n "$$p" ]; then dirname "$$p"; fi)
FIXTURES_DIR := pillow-rs/tests/fixtures
REPORT       := /tmp/report.json
TIMEOUT      := 300
MIGRATION_PARITY_OUTPUT ?= build/migration-parity/parity-result.json
MIGRATION_PARITY_ARGS ?=
MIGRATION_COVERAGE_OUTPUT ?= build/migration-parity/coverage-result.json
MIGRATION_RUST_COVERAGE_OUTPUT ?= build/migration-parity/coverage-result-rust.json
MIGRATION_COVERAGE_REPORT ?= target/coverage/migration-parity-python.json
MIGRATION_BENCHMARK_OUTPUT ?= build/migration-parity/benchmark-result.json
MIGRATION_BENCHMARK_PARITY_OUTPUT ?= build/migration-parity/benchmark-parity-result.json
MIGRATION_BENCHMARK_ARGS ?=
MIGRATION_STATUS_OUTPUT ?= build/migration-parity/status-report.json

ifneq ($(strip $(IMAGE_SLASH_STAR_AVIF_LIB_DIR)),)
export IMAGE_SLASH_STAR_AVIF_LIB_DIR
endif

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
	@printf "  $(CYAN)make test$(NC)           Run the canonical live source/target parity lane\n"
	@printf "  $(CYAN)make test-core$(NC)      Run Rust core unit tests\n"
	@printf "  $(CYAN)make test-wasm$(NC)      Run WASM/JS tests\n"
	@printf "  $(CYAN)make migration-parity-fixtures-check$(NC) Verify the fixed manifest and indexed inputs\n"
	@printf "  $(CYAN)make migration-parity-case-review$(NC) Verify duplicate selection and nuanced cases\n"
	@printf "  $(CYAN)make migration-parity-evidence-check$(NC) Validate strict result interfaces\n"
	@printf "  $(CYAN)make test-all$(NC)       Run core + Python + WASM tests\n"
	@printf "  $(CYAN)make migration-parity-test$(NC) Run the canonical live-oracle migration parity suite\n"
	@printf "  $(CYAN)make migration-parity-oracle-identity$(NC) Verify the pinned Pillow oracle identity\n"
	@printf "  $(CYAN)make migration-parity-target-identity$(NC) Verify the public pillow-rs target identity\n"
	@printf "  $(CYAN)make migration-parity-coverage$(NC) Run target coverage from indexed coverage plans\n"
	@printf "  $(CYAN)make migration-parity-coverage-rust$(NC) Run merged Python+Rust coverage with a temporary instrumented extension\n"
	@printf "  $(CYAN)make migration-parity-font-native-coverage$(NC) Run the font-native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imageops-native-coverage$(NC) Run the image-ops native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imagesequence-native-coverage$(NC) Run the image-sequence native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imagecore-native-coverage$(NC) Run the image-core native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-region-coverage$(NC) Report region coverage per public operation\n"
	@printf "  $(CYAN)make migration-parity-benchmark$(NC) Run correctness-gated benchmark workloads\n"
	@printf "  $(CYAN)make migration-parity-aggregate$(NC) Join compatible parity, coverage, and benchmark evidence\n"
	@printf "  $(CYAN)make migration-parity-docs$(NC) Generate specification and evidence documentation\n"
	@printf "  $(CYAN)make migration-parity-inventory$(NC) Print the canonical selected-scope endpoint inventory\n"
	@printf "  $(CYAN)make migration-parity-inventory-check$(NC) Verify endpoint authority expansion and alias accounting\n"
	@printf "  $(CYAN)make parity$(NC)        Run pillow-rs Font + fontdone unified parity\n"
	@printf "\n$(BOLD)pillow-rs / core crate$(NC)\n"
	@printf "  $(CYAN)make pillow-rs-help$(NC) Show crate-local pillow-rs targets\n"
	@printf "  $(CYAN)make pillow-rs-test$(NC) Run all pillow-rs Rust tests\n"
	@printf "  $(CYAN)make pillow-rs-lint$(NC) Run pillow-rs fmt + clippy\n"
	@printf "  $(CYAN)make pillow-rs-ci$(NC)   Run pillow-rs CI sequence\n"
	@printf "\n$(BOLD)fontdone / FreeType parity$(NC)\n"
	@printf "  $(CYAN)make fontdone-help$(NC)  Show crate-local fontdone targets\n"
	@printf "  $(CYAN)make fontdone-ci$(NC)    Run fontdone docs, lint, tests, parity, FFI, bench contracts\n"
	@printf "  $(CYAN)make fontdone-test$(NC)  Run fontdone non-oracle tests and all-target checks\n"
	@printf "  $(CYAN)make fontdone-parity$(NC) Run the FreeType parity matrix harness\n"
	@printf "  $(CYAN)make fontdone-ffi$(NC)   Run the no-runtime-FFI guard\n"
	@printf "  $(CYAN)make fontdone-ffi-compat$(NC) Run FreeType-shaped facade tests\n"
	@printf "  $(CYAN)make fontdone-doc$(NC)   Build strict fontdone rustdoc\n"
	@printf "  $(CYAN)make fontdone-bench$(NC) Run Rust vs C FreeType benchmark report\n"
	@printf "  $(CYAN)make fontdone-fixtures$(NC) Regenerate FreeType fixture families\n"
	@printf "\n$(BOLD)image-slash-star / image backend$(NC)\n"
	@printf "  $(CYAN)make image-slash-star-check$(NC) Check image-slash-star default features\n"
	@printf "  $(CYAN)make image-slash-star-feature-test$(NC) Check image-slash-star no-default feature boundary\n"
	@printf "  $(CYAN)make image-slash-star-test$(NC) Run image-slash-star default package tests\n"
	@printf "  $(CYAN)make image-slash-star-lint$(NC) Run image-slash-star fmt + clippy\n"
	@printf "  $(CYAN)make image-slash-star-full-test$(NC) Run image-slash-star all-feature coverage matrix\n"
	@printf "\n$(BOLD)Fixtures$(NC)\n"
	@printf "  $(CYAN)make fixtures$(NC)       Build the active manifest and input specifications\n"
	@printf "  $(CYAN)make migration-parity-manifest$(NC) Build the fixed project-wide manifest from frozen authority\n"
	@printf "  $(CYAN)make migration-parity-inputs$(NC) Build deterministic parity/coverage/benchmark inputs\n"
	@printf "  $(CYAN)make migration-parity-fixtures$(NC) Compatibility alias for manifest and input builds\n"
	@printf "  $(CYAN)make migration-parity-case-review$(NC) Review duplicate and nuanced case selection\n"
	@printf "  $(CYAN)make migration-parity-fixtures-check$(NC) Verify authority, manifest, and input regeneration\n"
	@printf "  $(CYAN)make migration-parity-evidence-check$(NC) Verify strict aggregate/result interfaces\n"
	@printf "  $(CYAN)make migration-parity-inputs$(NC) Build parity, coverage, and benchmark inputs\n"
	@printf "\n$(BOLD)Lint$(NC)\n"
	@printf "  $(CYAN)make fmt$(NC)            Check Rust formatting\n"
	@printf "  $(CYAN)make fmt-fix$(NC)        Fix Rust formatting\n"
	@printf "  $(CYAN)make clippy$(NC)         Run clippy (all targets, all features)\n"
	@printf "  $(CYAN)make lint$(NC)           fmt + clippy\n"
	@printf "\n$(BOLD)Coverage$(NC)\n"
	@printf "  $(CYAN)make coverage$(NC)       Run tests + compute coverage\n"
	@printf "  $(CYAN)make coverage-python-abi-rust$(NC) Run Pillow parity through PyO3 with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-point-rust$(NC) Run Image.point Pillow parity with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-image-open-rust$(NC) Run Image.open Pillow parity with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-apply-transparency-rust$(NC) Run Image.apply_transparency Pillow parity with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-paste-rust$(NC) Run Image.paste Pillow parity with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-drawing-rust$(NC) Run ImageDraw Pillow parity with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-imagefont-getmask2-rust$(NC) Run ImageFont.getmask2 parity with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-transposed-font-rust$(NC) Run TransposedFont parity with Rust LLVM coverage\n"
	@printf "  $(CYAN)make coverage-python-wrapper$(NC) Run Pillow parity with Python wrapper branch coverage\n"
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
	@$(PYTHON) -m pip --version >/dev/null 2>&1 || { echo "Bootstrapping pip..."; $(PYTHON) -m ensurepip --upgrade; }
	@$(MATURIN) --version >/dev/null 2>&1 || { echo "Installing maturin..."; $(PIP) install maturin; }
	@command -v $(WASM_PACK) >/dev/null 2>&1 || { echo "Installing wasm-pack..."; cargo install wasm-pack; }
	@[ -n "$$VIRTUAL_ENV" ] || [ -n "$$CONDA_PREFIX" ] || [ "$(PYTHON)" = ".venv/bin/python" ] || echo "⚠️  No virtualenv detected — consider: python3 -m venv .venv && source .venv/bin/activate"
	$(PIP) install maturin coverage pillow==12.2.0 numpy pyyaml pytest pytest-timeout pytest-json-report pytest-benchmark

setup-ci: ## Install dev deps for CI
	@$(PYTHON) -m pip --version >/dev/null 2>&1 || { echo "Bootstrapping pip..."; $(PYTHON) -m ensurepip --upgrade; }
	$(PIP) install maturin coverage pillow==12.2.0 numpy pyyaml pytest pytest-timeout pytest-json-report pytest-benchmark

# ── Build ─────────────────────────────────────────────────────────────────────
.PHONY: build build-dev build-wasm build-wasm-core build-wasm-extra build-wasm-release build-all

build: ## Build Python package (release)
	$(MATURIN) develop --manifest-path $(PY_SRC)/Cargo.toml --release

build-dev: ## Build Python package (debug, faster compile)
	$(MATURIN) develop --manifest-path $(PY_SRC)/Cargo.toml

build-wasm: build-wasm-core ## Build the default core WASM package (dev)

build-wasm-core: ## Build PNG-only core WASM package (dev)
	cd $(JS_SRC) && npm run build:core

build-wasm-extra: ## Build Rust-codec extra WASM package (dev)
	cd $(JS_SRC) && npm run build:extra

build-wasm-release: ## Build WASM package (release)
	cd $(JS_SRC) && npm run build:release

build-all: build build-wasm-release ## Build Python + WASM

# ── Test ──────────────────────────────────────────────────────────────────────
.PHONY: test test-core test-wasm test-all
.PHONY: backend-support-matrix

test: migration-parity-fixtures-check migration-parity-test ## Run the complete live source/target parity lane

test-core: ## Run Rust core unit tests
	$(MAKE) -C $(CORE_SRC) test-core

backend-support-matrix: ## Emit registry-derived CPU/SIMD/GPU support JSON
	$(MAKE) -C $(CORE_SRC) backend-support-matrix

test-wasm: build-wasm-core build-wasm-extra ## Build the declared WASM packages and validate their package boundary
	cd $(JS_SRC) && npm run test:package

test-all: test-core test test-wasm ## Run core + live parity + WASM package checks

.PHONY: parity
parity: font-tests fontdone-parity ## Run pillow-rs Font + fontdone unified parity

# ── fontdone / FreeType parity ───────────────────────────────────────────────
.PHONY: fontdone-help fontdone-build fontdone-doc fontdone-doc-test
.PHONY: fontdone-test fontdone-parity fontdone-ffi fontdone-ffi-compat fontdone-lint
.PHONY: fontdone-fmt fontdone-fmt-fix fontdone-clippy
.PHONY: fontdone-bench fontdone-bench-quick fontdone-bench-self-test
.PHONY: fontdone-fixtures fontdone-ci fontdone-clean
.PHONY: freetype-help freetype-build freetype-doc freetype-doc-test
.PHONY: freetype-test freetype-parity freetype-ffi freetype-ffi-compat freetype-lint
.PHONY: freetype-fmt freetype-fmt-fix freetype-clippy
.PHONY: freetype-bench freetype-bench-quick freetype-bench-self-test
.PHONY: freetype-fixtures freetype-ci freetype-clean
.PHONY: image-slash-star-check image-slash-star-feature-test image-slash-star-test
.PHONY: image-slash-star-fmt image-slash-star-clippy image-slash-star-lint
.PHONY: image-slash-star-full-test image-slash-star-ci

# ── pillow-rs / core crate ──────────────────────────────────────────────────
.PHONY: pillow-rs-help pillow-rs-test pillow-rs-test-core
.PHONY: image-backend-test image-backend-migration-test image-backend-parity-test image-backend-feature-test
.PHONY: migration-parity-test migration-parity-oracle-identity migration-parity-target-identity migration-parity-coverage migration-parity-coverage-rust migration-parity-font-native-coverage migration-parity-region-coverage migration-parity-benchmark migration-parity-aggregate migration-parity-docs
.PHONY: font-tests font-tests-release imagingft-tests imagingft-tests-release pillow-rs-imagingft pillow-rs-imagingft-release
.PHONY: pillow-rs-fixtures-clean
.PHONY: pillow-rs-public-api-boundary pillow-rs-fmt pillow-rs-fmt-fix pillow-rs-clippy pillow-rs-lint
.PHONY: pillow-rs-build pillow-rs-build-release pillow-rs-bench
.PHONY: pillow-rs-ci pillow-rs-clean

pillow-rs-help: ## Show pillow-rs crate targets
	$(MAKE) -C $(CORE_SRC) help

pillow-rs-test: ## Run all pillow-rs Rust tests
	$(MAKE) -C $(CORE_SRC) test

image-backend-test image-backend-migration-test image-backend-parity-test image-backend-feature-test:
	@printf "This legacy image-backend parity target is archived under deprecated/migration-parity-v0.\n"
	@printf "Use 'make migration-parity-test' with the active manifest-driven inputs.\n"
	@exit 2

pillow-rs-test-core: ## Run pillow-rs unit tests
	$(MAKE) -C $(CORE_SRC) test-core

migration-parity-test: ## Run canonical input-only parity against live Pillow
	set +e; \
	$(PYTHON) scripts/run_migration_parity.py --output $(MIGRATION_PARITY_OUTPUT) $(MIGRATION_PARITY_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py parity $(MIGRATION_PARITY_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-oracle-identity: ## Verify the pinned Pillow oracle identity
	$(PYTHON) scripts/run_migration_parity.py --identity source

migration-parity-target-identity: ## Verify the public pillow-rs target identity
	PYTHONPATH=$(abspath $(PY_SRC)/python):$$PYTHONPATH \
		$(PYTHON) scripts/run_migration_parity.py --identity target

migration-parity-coverage: ## Run target coverage from indexed coverage plans
	set +e; \
	$(PYTHON) scripts/run_migration_coverage.py \
		--output $(MIGRATION_COVERAGE_OUTPUT) \
		--coverage-report $(MIGRATION_COVERAGE_REPORT); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py coverage $(MIGRATION_COVERAGE_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-coverage-rust: ## Run merged Python+Rust coverage with a temporary instrumented extension
	set +e; \
	$(PYTHON) scripts/run_migration_rust_coverage.py \
		--output $(MIGRATION_RUST_COVERAGE_OUTPUT); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py coverage $(MIGRATION_RUST_COVERAGE_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-font-native-coverage: ## Run the font-native coverage-only corpus
	$(PYTHON) scripts/run_migration_font_native_cases.py

migration-parity-imageops-native-coverage: ## Run the image-ops native coverage-only corpus
	$(PYTHON) scripts/run_migration_imageops_native_cases.py

migration-parity-imagesequence-native-coverage: ## Run the image-sequence native coverage-only corpus
	$(PYTHON) scripts/run_migration_imagesequence_native_cases.py

migration-parity-imagecore-native-coverage: ## Run the image-core native coverage-only corpus
	$(PYTHON) scripts/run_migration_imagecore_native_cases.py

migration-parity-region-coverage: ## Report region coverage per public operation
	$(PYTHON) scripts/report_migration_parity_region_coverage.py

migration-parity-benchmark: ## Run correctness-gated benchmark workloads
	set +e; \
	$(PYTHON) scripts/run_migration_benchmark.py \
		--output $(MIGRATION_BENCHMARK_OUTPUT) \
		--parity-output $(MIGRATION_BENCHMARK_PARITY_OUTPUT) \
		$(MIGRATION_BENCHMARK_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py benchmark $(MIGRATION_BENCHMARK_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-aggregate: ## Join compatible parity, coverage, and benchmark evidence
	@coverage_evidence="$(MIGRATION_COVERAGE_OUTPUT)"; \
	if [ -f "$(MIGRATION_RUST_COVERAGE_OUTPUT)" ]; then coverage_evidence="$(MIGRATION_RUST_COVERAGE_OUTPUT)"; fi; \
	set +e; \
	$(PYTHON) scripts/aggregate_migration_parity.py \
		--parity $(MIGRATION_PARITY_OUTPUT) \
		--coverage "$$coverage_evidence" \
		--benchmark $(MIGRATION_BENCHMARK_OUTPUT) \
		--output $(MIGRATION_STATUS_OUTPUT); \
	status=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	$(PYTHON) scripts/validate_migration_parity_result.py status $(MIGRATION_STATUS_OUTPUT)

migration-parity-docs: migration-parity-aggregate ## Generate specification and evidence documentation
	$(PYTHON) scripts/generate_migration_parity_docs.py \
		--status $(MIGRATION_STATUS_OUTPUT)

font-tests font-tests-release imagingft-tests imagingft-tests-release:
	$(MAKE) migration-parity-test

pillow-rs-imagingft pillow-rs-imagingft-release:
	@printf "The legacy ImagingFT matrix is archived under deprecated/migration-parity-v0.\n"
	@printf "Use 'make migration-parity-test' with the active manifest-driven inputs.\n"
	@exit 2

pillow-rs-fixtures-clean: ## Remove imagingft fixture outputs
	$(MAKE) -C $(CORE_SRC) fixtures-clean

pillow-rs-public-api-boundary: ## Enforce pillow-rs explicit root public API boundary
	$(MAKE) -C $(CORE_SRC) public-api-boundary

pillow-rs-fmt: ## Check pillow-rs formatting
	$(MAKE) -C $(CORE_SRC) fmt

pillow-rs-fmt-fix: ## Fix pillow-rs formatting
	$(MAKE) -C $(CORE_SRC) fmt-fix

pillow-rs-clippy: ## Run strict pillow-rs clippy
	$(MAKE) -C $(CORE_SRC) clippy

pillow-rs-lint: ## Run pillow-rs fmt + clippy
	$(MAKE) -C $(CORE_SRC) lint

pillow-rs-build: ## Build pillow-rs
	$(MAKE) -C $(CORE_SRC) build

pillow-rs-build-release: ## Build pillow-rs (release)
	$(MAKE) -C $(CORE_SRC) build-release

pillow-rs-bench: ## Run pillow-rs benchmarks
	$(MAKE) -C $(CORE_SRC) bench

pillow-rs-ci: ## Run pillow-rs CI sequence
	$(MAKE) -C $(CORE_SRC) ci

pillow-rs-clean: ## Clean pillow-rs artifacts
	$(MAKE) -C $(CORE_SRC) clean

# ── fontdone / FreeType parity ───────────────────────────────────────────────
fontdone-help: ## Show fontdone targets
	$(MAKE) -C $(FONTDONE_SRC) help

fontdone-build: ## Build fontdone
	$(MAKE) -C $(FONTDONE_SRC) build

fontdone-doc: ## Build strict fontdone rustdoc
	$(MAKE) -C $(FONTDONE_SRC) doc

fontdone-doc-test: ## Run fontdone doctests
	$(MAKE) -C $(FONTDONE_SRC) doc-test

fontdone-test: ## Run fontdone non-oracle tests and all-target checks
	$(MAKE) -C $(FONTDONE_SRC) test-fast

fontdone-parity: ## Run FreeType parity matrix tests
	$(MAKE) -C $(FONTDONE_SRC) test-parity

fontdone-ffi: ## Run no-runtime-FFI guard
	$(MAKE) -C $(FONTDONE_SRC) test-ffi

fontdone-ffi-compat: ## Run FreeType-shaped API/ABI facade audit
	$(MAKE) -C $(FONTDONE_SRC) api-abi-check

fontdone-fmt: ## Check fontdone formatting
	$(MAKE) -C $(FONTDONE_SRC) fmt

fontdone-fmt-fix: ## Apply fontdone formatting
	$(MAKE) -C $(FONTDONE_SRC) fmt

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
	$(MAKE) -C $(FONTDONE_SRC) font-fixtures

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
freetype-ffi-compat: fontdone-ffi-compat
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

# ── image-slash-star / image backend ──────────────────────────────────────────
image-slash-star-check: ## Check image-slash-star default features
	$(CARGO) check --manifest-path $(IMAGE_SLASH_STAR_SRC)/Cargo.toml --all-targets --locked

image-slash-star-feature-test: ## Check image-slash-star no-default feature boundary
	$(CARGO) check --manifest-path $(IMAGE_SLASH_STAR_SRC)/Cargo.toml --no-default-features --locked

image-slash-star-test: ## Run image-slash-star default package tests
	$(CARGO) test --manifest-path $(IMAGE_SLASH_STAR_SRC)/Cargo.toml --locked

image-slash-star-fmt: ## Check image-slash-star formatting
	$(CARGO) fmt --manifest-path $(IMAGE_SLASH_STAR_SRC)/Cargo.toml --all -- --check

image-slash-star-clippy: ## Run strict image-slash-star clippy on default features
	$(CARGO) clippy --manifest-path $(IMAGE_SLASH_STAR_SRC)/Cargo.toml \
		--workspace --all-targets --locked -- -D warnings

image-slash-star-lint: image-slash-star-fmt image-slash-star-clippy ## Run image-slash-star fmt + clippy

image-slash-star-full-test: ## Run image-slash-star all-feature coverage matrix
	$(CARGO) test --manifest-path $(IMAGE_SLASH_STAR_SRC)/Cargo.toml \
		--all-features --test coverage_matrix_tests --locked

image-slash-star-ci: image-slash-star-check image-slash-star-feature-test image-slash-star-test image-slash-star-lint ## Run maintained image-slash-star integration gates

# ── Fixtures ──────────────────────────────────────────────────────────────────
.PHONY: fixtures migration-parity-inventory migration-parity-inventory-check migration-parity-manifest migration-parity-inputs migration-parity-fixtures migration-parity-case-review migration-parity-fixtures-check migration-parity-evidence-check image-backend-fixtures putdata-fixtures
.PHONY: imagefont-getmask2-fixtures
.PHONY: compact-value-fixtures color3dlut-fixtures point-fixtures eval-fixtures
.PHONY: palette-save-fixtures image-io-fixtures tobytes-fixtures test-color3dlut
.PHONY: fixture-coverage-check
.PHONY: fixtures-suite0 fixtures-suite1 fixtures-clean

fixtures: migration-parity-fixtures ## Build the active manifest and input specifications

migration-parity-inventory: ## Print canonical selected-scope endpoint inventory
	$(PYTHON) scripts/migration_parity_inventory.py

migration-parity-inventory-check: ## Verify endpoint authority expansion and alias accounting
	$(PYTHON) -m unittest tests.test_migration_parity_inventory

migration-parity-manifest: ## Build fixed project-wide manifest from frozen authority
	$(PYTHON) scripts/build_migration_parity_manifest.py

migration-parity-inputs: ## Build deterministic parity, coverage, and benchmark inputs
	$(PYTHON) scripts/build_migration_parity_inputs.py

migration-parity-fixtures: migration-parity-manifest migration-parity-inputs ## Compatibility alias during migration

migration-parity-case-review: migration-parity-inputs ## Review duplicate and nuanced active case selection
	$(PYTHON) scripts/review_migration_parity_cases.py

migration-parity-fixtures-check: migration-parity-inventory-check ## Verify authority and manifest regeneration
	@tmp="$$(mktemp -d)"; \
	trap 'rm -rf "$$tmp"' EXIT; \
	$(PYTHON) scripts/build_migration_parity_manifest.py --output "$$tmp/manifest.yaml"; \
	diff -u pillow-rs/tests/fixtures/manifest.yaml "$$tmp/manifest.yaml"; \
	$(MAKE) migration-parity-inputs-check
	$(MAKE) migration-parity-evidence-check

.PHONY: migration-parity-inputs-check
migration-parity-inputs-check: ## Verify deterministic input regeneration
	$(PYTHON) scripts/check_migration_parity_inputs.py
	$(PYTHON) -m unittest tests.test_migration_parity_cases tests.test_migration_parity_contract

migration-parity-evidence-check: ## Verify strict aggregate/result interfaces
	$(PYTHON) -m unittest tests.test_migration_parity_evidence

image-backend-fixtures putdata-fixtures imagefont-getmask2-fixtures \
	compact-value-fixtures color3dlut-fixtures point-fixtures eval-fixtures \
	palette-save-fixtures image-io-fixtures tobytes-fixtures test-color3dlut \
	fixture-coverage-check fixtures-suite0 fixtures-suite1 fixtures-clean:
	@printf "This legacy fixture target is archived under deprecated/migration-parity-v0.\n"
	@printf "Use 'make migration-parity-fixtures' and the live migration lanes.\n"
	@exit 2

# ── Lint ──────────────────────────────────────────────────────────────────────
.PHONY: fmt fmt-fix clippy clippy-core lint

fmt: ## Check Rust formatting
	python3 scripts/check_public_api_boundary.py
	$(CARGO) fmt --check

fmt-fix: ## Fix Rust formatting
	$(CARGO) fmt
	python3 scripts/check_public_api_boundary.py

clippy: ## Run clippy on all targets
	python3 scripts/check_public_api_boundary.py
	$(CARGO) clippy --all-targets --all-features -- -A deprecated

clippy-core: ## Run clippy on core only
	python3 scripts/check_public_api_boundary.py
	$(CARGO) clippy -p $(CORE_SRC) -- -A deprecated

lint: fmt clippy ## Run fmt + clippy

# ── Coverage ──────────────────────────────────────────────────────────────────
.PHONY: coverage coverage-validate coverage-report coverage-wasm

coverage: migration-parity-coverage ## Run the canonical target coverage lane

coverage-validate: migration-parity-coverage ## Validate the canonical coverage result

coverage-report: migration-parity-docs ## Regenerate the migration-parity evidence documentation

coverage-wasm:
	@printf "No WASM coverage profile is declared in the active manifest.\n"
	@printf "Add a reviewed target profile and coverage plan before enabling this lane.\n"
	@exit 2

coverage-python-abi-rust coverage-python-wrapper coverage-image-backend-rust \
	coverage-point-rust coverage-image-open-rust coverage-apply-transparency-rust \
	coverage-paste-rust coverage-drawing-rust coverage-imagefont-getmask2-rust \
	coverage-transposed-font-rust coverage-font-rust coverage-font-rust-with-freetype \
	coverage-imagingft-rust font-tests-coverage font-tests-coverage-with-freetype \
	imagingft-tests-coverage:
	@printf "This legacy coverage target is archived under deprecated/migration-parity-v0.\n"
	@printf "Use 'make migration-parity-coverage' with indexed coverage plans.\n"
	@exit 2

# ── Benchmark ─────────────────────────────────────────────────────────────────
.PHONY: bench bench-incr bench-priority

bench bench-incr bench-priority: migration-parity-benchmark ## Run the fixed correctness-gated benchmark lane

# ── Documentation ─────────────────────────────────────────────────────────────
.PHONY: repo-map-check repo-map-update

repo-map-check: ## Validate docs/REPO_MAP.md generated tree
	$(PYTHON) scripts/check_repo_map.py

repo-map-update: ## Refresh docs/REPO_MAP.md generated tree
	$(PYTHON) scripts/check_repo_map.py --write

# ── CI ────────────────────────────────────────────────────────────────────────
.PHONY: ci verify

ci: repo-map-check fmt clippy pillow-rs-test-core migration-parity-fixtures-check migration-parity-test migration-parity-coverage ## Full CI pipeline
	@echo "=== done ==="

verify: ci fontdone-parity ## Full workspace CI plus FreeType parity
	@echo "=== all verification done ==="

# ── Clean ─────────────────────────────────────────────────────────────────────
.PHONY: clean clean-all

clean: ## Remove build artifacts and caches
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	rm -rf .pytest_cache
	rm -f $(REPORT)

clean-all: clean ## clean + cargo clean
	$(CARGO) clean

# ── Stubs ─────────────────────────────────────────────────────────────────────
.PHONY: stubs

stubs:
	@printf "The old stub generator is archived under deprecated/migration-parity-v0.\n"
	@printf "Use the fixed manifest inventory and implementation checks instead.\n"
	@exit 2

# ── Release ───────────────────────────────────────────────────────────────────
.PHONY: release-pypi release-npm release-crates

release-pypi: build ## Build + publish to PyPI
	cd $(PY_SRC) && $(MATURIN) publish

release-npm: build-wasm-release ## Build WASM + publish to npm
	cd $(JS_SRC)/pkg && npm publish

release-crates: ## Publish to crates.io
	$(CARGO) publish -p $(CORE_SRC)
