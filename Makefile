# pillow-rs Makefile
# ===================
# Single entry point for all build, test, lint, bench, coverage, and release workflows.
# Run `make` or `make help` to see all targets.

# ── Variables ─────────────────────────────────────────────────────────────────
PYTHON       ?= $(shell if [ -x .venv/bin/python ]; then printf '%s' .venv/bin/python; else printf '%s' python3; fi)
PIP          := $(PYTHON) -m pip
MATURIN      := $(PYTHON) -m maturin
IMAGE_ORACLE_PYTHON ?= $(abspath .oracle-venv/bin/python)
NODE         := node
CARGO        := cargo
WASM_PACK    := wasm-pack
MANIFEST     := manifest.yaml
PY_SRC       := pillow-rs-py
JS_SRC       := pillow-rs-js
CORE_SRC     := pillow-rs
FONTDONE_SRC := ../fontdone
IMAGE_SLASH_STAR_SRC := $(abspath ../image-slash-star)
IMAGE_SLASH_STAR_AVIF_LIB_DIR ?= $(shell p="$$(find "$(IMAGE_SLASH_STAR_SRC)/.oracle-venv" -name 'libavif*' -type f -print -quit 2>/dev/null)"; if [ -n "$$p" ]; then dirname "$$p"; fi)
FIXTURES_DIR := tests/fixtures
FIXTURES_SUITE1_DIR := tests/fixtures_2
REPORT       := /tmp/report.json
TIMEOUT      := 300
MIGRATION_PARITY_OUTPUT ?= build/migration-parity/parity-result.json
MIGRATION_PARITY_ARGS ?=
MIGRATION_COVERAGE_OUTPUT ?= build/migration-parity/coverage-result.json
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
	@printf "  $(CYAN)make test$(NC)           Run all PIL parity tests\n"
	@printf "  $(CYAN)make test-suite0$(NC)    Run suite0 only (core functions, fast)\n"
	@printf "  $(CYAN)make test-suite1$(NC)    Run suite1 only\n"
	@printf "  $(CYAN)make test-suite2$(NC)    Run suite2 only\n"
	@printf "  $(CYAN)make test-putdata$(NC)   Run Image.putdata public and fixture parity\n"
	@printf "  $(CYAN)make test-imagefont-getmask2$(NC) Run independent ImageFont.getmask2 Pillow fixtures\n"
	@printf "  $(CYAN)make test-imagefont-facade$(NC) Run Pillow-oracle ImageFont Python facade tests\n"
	@printf "  $(CYAN)make test-point$(NC)     Run Image.point Pillow-oracle fixture parity\n"
	@printf "  $(CYAN)make test-eval$(NC)      Run Image.eval Pillow-oracle fixture parity\n"
	@printf "  $(CYAN)make test-palette-save$(NC) Run ImagePalette.save Pillow-oracle parity\n"
	@printf "  $(CYAN)make test-image-io$(NC)  Run Image open/save Pillow-oracle parity\n"
	@printf "  $(CYAN)make test-tobytes$(NC)   Run Image.tobytes Pillow-oracle parity\n"
	@printf "  $(CYAN)make test-compact-values$(NC) Run compact exact-value fixture parity\n"
	@printf "  $(CYAN)make test-core$(NC)      Run Rust core tests\n"
	@printf "  $(CYAN)make test-wasm$(NC)      Run WASM/JS tests\n"
	@printf "  $(CYAN)make js-oracle-contract$(NC) Validate the shared exact Pillow corpus in Node\n"
	@printf "  $(CYAN)make js-image-open-oracle$(NC) Run Image.open corpus through WASM\n"
	@printf "  $(CYAN)make js-tobytes-oracle$(NC) Run Image.tobytes corpus through WASM\n"
	@printf "  $(CYAN)make apply-transparency-oracle$(NC) Run exact Pillow Image.apply_transparency fixtures through all ABIs\n"
	@printf "  $(CYAN)make paste-oracle$(NC) Run exact Pillow Image.paste fixtures through all ABIs\n"
	@printf "  $(CYAN)make drawing-oracle$(NC) Run exact Pillow ImageDraw fixtures through all ABIs\n"
	@printf "  $(CYAN)make imagefont-getmask2-oracle$(NC) Run exact Pillow ImageFont.getmask2 fixtures through all ABIs\n"
	@printf "  $(CYAN)make transposed-font-oracle$(NC) Run exact Pillow TransposedFont paths through all ABIs\n"
	@printf "  $(CYAN)make fromarray-descriptor-oracle$(NC) Run exact Pillow fromarray descriptors through all ABIs\n"
	@printf "  $(CYAN)make rust-image-open-oracle$(NC) Run Image.open corpus through Rust API\n"
	@printf "  $(CYAN)make test-all$(NC)       Run core + Python + WASM tests\n"
	@printf "  $(CYAN)make image-backend-test$(NC) Run image backend migration parity\n"
	@printf "  $(CYAN)make image-backend-migration-test$(NC) Run codec/backend migration fixtures\n"
	@printf "  $(CYAN)make image-backend-parity-test$(NC) Run forced-backend Pillow parity\n"
	@printf "  $(CYAN)make image-backend-feature-test$(NC) Verify disabled codec forwarding\n"
	@printf "  $(CYAN)make migration-parity-test$(NC) Run the canonical live-oracle migration parity suite\n"
	@printf "  $(CYAN)make migration-parity-oracle-identity$(NC) Verify the pinned Pillow oracle identity\n"
	@printf "  $(CYAN)make migration-parity-target-identity$(NC) Verify the public pillow-rs target identity\n"
	@printf "  $(CYAN)make migration-parity-coverage$(NC) Run target coverage from indexed coverage plans\n"
	@printf "  $(CYAN)make migration-parity-benchmark$(NC) Run correctness-gated benchmark workloads\n"
	@printf "  $(CYAN)make migration-parity-aggregate$(NC) Join compatible parity, coverage, and benchmark evidence\n"
	@printf "  $(CYAN)make migration-parity-docs$(NC) Generate specification and evidence documentation\n"
	@printf "  $(CYAN)make migration-parity-inventory$(NC) Print the canonical selected-scope endpoint inventory\n"
	@printf "  $(CYAN)make migration-parity-inventory-check$(NC) Verify endpoint authority expansion and alias accounting\n"
	@printf "  $(CYAN)make parity$(NC)        Run pillow-rs Font + fontdone unified parity\n"
	@printf "\n$(BOLD)pillow-rs / core crate$(NC)\n"
	@printf "  $(CYAN)make pillow-rs-help$(NC) Show crate-local pillow-rs targets\n"
	@printf "  $(CYAN)make pillow-rs-test$(NC) Run all pillow-rs Rust tests\n"
	@printf "  $(CYAN)make font-tests$(NC)                  Run Font public API parity tests\n"
	@printf "  $(CYAN)make font-tests-release$(NC)          Run Font public API parity tests (release)\n"
	@printf "  $(CYAN)make font-tests-coverage-with-freetype$(NC) Run Font parity coverage including fontdone\n"
	@printf "  $(CYAN)make imagingft-tests$(NC)              Compatibility alias for font-tests\n"
	@printf "  $(CYAN)make pillow-rs-imagingft$(NC)           Run legacy ImagingFT matrix parity tests\n"
	@printf "  $(CYAN)make pillow-rs-imagingft-release$(NC)  Run legacy ImagingFT matrix parity tests (release)\n"
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
	@printf "  $(CYAN)make fixtures$(NC)       Generate all test fixtures (requires Pillow)\n"
	@printf "  $(CYAN)make migration-parity-manifest$(NC) Build the fixed project-wide manifest from frozen authority\n"
	@printf "  $(CYAN)make migration-parity-inputs$(NC) Build deterministic parity/coverage/benchmark inputs\n"
	@printf "  $(CYAN)make migration-parity-fixtures$(NC) Compatibility alias for manifest and input builds\n"
	@printf "  $(CYAN)make migration-parity-case-review$(NC) Review duplicate and nuanced case selection\n"
	@printf "  $(CYAN)make migration-parity-fixtures-check$(NC) Verify authority, manifest, and input regeneration\n"
	@printf "  $(CYAN)make migration-parity-evidence-check$(NC) Verify strict aggregate/result interfaces\n"
	@printf "  $(CYAN)make image-backend-fixtures$(NC) Generate image backend migration fixtures\n"
	@printf "  $(CYAN)make putdata-fixtures$(NC) Generate semantic Image.putdata fixtures\n"
	@printf "  $(CYAN)make imagefont-getmask2-fixtures$(NC) Generate independent ImageFont.getmask2 fixtures\n"
	@printf "  $(CYAN)make compact-value-fixtures$(NC) Regenerate compact typed sequence oracles\n"
	@printf "  $(CYAN)make fixture-coverage-check$(NC) Validate semantic fixture/manifest coverage\n"
	@printf "  $(CYAN)make fixtures-suite0$(NC) Generate suite0 fixtures only\n"
	@printf "  $(CYAN)make fixtures-suite1$(NC) Generate suite1 fixtures only\n"
	@printf "  $(CYAN)make fixtures-clean$(NC) Remove fixture outputs\n"
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
.PHONY: test test-suite0 test-suite1 test-suite2 test-putdata test-imagefont-getmask2 test-imagefont-facade test-point test-eval test-palette-save test-image-io test-tobytes test-compact-values
.PHONY: test-core test-wasm test-all rust-color3dlut-oracle rust-eval-oracle rust-image-open-oracle rust-tobytes-oracle js-oracle-contract js-color3dlut-oracle
.PHONY: js-eval-oracle js-image-open-oracle js-tobytes-oracle apply-transparency-oracle paste-oracle drawing-oracle imagefont-getmask2-oracle transposed-font-oracle fromarray-descriptor-oracle
.PHONY: backend-support-matrix

test: fixtures ## Run all PIL parity tests
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT) --strict-covers

test-suite0: fixtures-suite0 ## Run suite0 only (core functions)
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT) \
		--strict-covers -k "not suite1 and not suite2 and not suite3"

test-suite1: fixtures-suite1 ## Run suite1 only
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT) --strict-covers -k "suite1"

test-suite2: ## Run suite2 only
	$(PYTHON) -m pytest tests/ -q --tb=short --timeout=$(TIMEOUT) \
		--json-report --json-report-file=$(REPORT) --strict-covers -k "suite2"

test-putdata: putdata-fixtures ## Run Image.putdata public and fixture parity
	$(PYTHON) -m pytest tests/test_putdata_parity.py tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers -k "putdata"

test-imagefont-getmask2: imagefont-getmask2-fixtures ## Run independent ImageFont.getmask2 Pillow parity
	$(PYTHON) -m pytest tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers -k "ImageFont and getmask2"

test-imagefont-facade: ## Run Pillow-oracle ImageFont Python facade parity
	$(PYTHON) -m pytest tests/test_imagefont_facade_oracle.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers

test-point: point-fixtures ## Run exact Image.point Pillow parity
	$(PYTHON) -m pytest tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers -k "Image.point"

test-eval: eval-fixtures ## Run exact Image.eval Pillow parity
	$(PYTHON) -m pytest tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers -k "ImageModule.eval"

test-palette-save: palette-save-fixtures ## Run exact ImagePalette.save Pillow parity
	$(PYTHON) -m pytest tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers -k "ImagePalette.save"

test-image-io: image-io-fixtures ## Run exact Image open/save Pillow parity
	$(PYTHON) -m pytest tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers \
		-k "ImageModule.open or Image.open or Image.save"

test-tobytes: tobytes-fixtures ## Run exact Image.tobytes Pillow parity
	$(PYTHON) -m pytest tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers -k "Image.tobytes"

test-compact-values: compact-value-fixtures ## Run compact sequence-value parity
	$(PYTHON) -m pytest tests/test_parity.py \
		-q --tb=short --timeout=$(TIMEOUT) --strict-covers \
		-k "getdata or get_flattened_data"

test-core: ## Run Rust core tests (pillow-rs unit + Font public API)
	$(MAKE) -C $(CORE_SRC) test

rust-color3dlut-oracle: color3dlut-fixtures ## Run Color3DLUT corpus through Rust API
	$(MAKE) -C $(CORE_SRC) test-color3dlut-oracle

rust-eval-oracle: eval-fixtures ## Run Image.eval corpus through Rust API
	$(MAKE) -C $(CORE_SRC) test-eval-oracle

rust-image-open-oracle: image-io-fixtures ## Run Image.open corpus through Rust API
	$(MAKE) -C $(CORE_SRC) test-image-open-oracle

rust-tobytes-oracle: tobytes-fixtures ## Run Image.tobytes corpus through Rust API
	$(MAKE) -C $(CORE_SRC) test-tobytes-oracle

backend-support-matrix: ## Emit registry-derived CPU/SIMD/GPU support JSON
	$(MAKE) -C $(CORE_SRC) backend-support-matrix

test-wasm: ## Run WASM/JS tests
	cd $(JS_SRC) && npm run test:codecs
	cd $(JS_SRC) && npm run test:package

js-oracle-contract: fixtures ## Validate shared Pillow input/output pairs from Node
	cd $(JS_SRC) && npm run test:oracle-contract

js-color3dlut-oracle: color3dlut-fixtures build-wasm-core ## Run Color3DLUT corpus through WASM
	cd $(JS_SRC) && npm run test:color3dlut-oracle

js-eval-oracle: eval-fixtures build-wasm-core ## Run Image.eval corpus through WASM
	cd $(JS_SRC) && npm run test:eval-oracle

js-image-open-oracle: image-io-fixtures build-wasm-extra ## Run Image.open corpus through WASM
	cd $(JS_SRC) && npm run test:image-open-oracle

js-tobytes-oracle: tobytes-fixtures build-wasm-core ## Run Image.tobytes corpus through WASM
	cd $(JS_SRC) && npm run test:tobytes-oracle

apply-transparency-oracle: build-wasm-extra ## Run exact Pillow Image.apply_transparency fixtures through Rust/Python/WASM
	$(MAKE) -C $(CORE_SRC) test-apply-transparency-oracle
	$(PYTHON) -m pytest tests/test_apply_transparency_oracle.py -q
	cd $(JS_SRC) && npm run test:apply-transparency-oracle

paste-oracle: build-wasm-extra ## Run exact Pillow Image.paste fixtures through Rust/Python/WASM
	$(MAKE) -C $(CORE_SRC) test-paste-oracle
	$(PYTHON) -m pytest tests/test_paste_oracle.py -q
	cd $(JS_SRC) && npm run test:paste-oracle

drawing-oracle: build-wasm-extra ## Run exact Pillow ImageDraw fixtures through Rust/Python/WASM
	$(MAKE) -C $(CORE_SRC) test-drawing-oracle
	$(PYTHON) -m pytest tests/test_drawing_oracle.py -q
	cd $(JS_SRC) && npm run test:drawing-oracle

imagefont-getmask2-oracle: imagefont-getmask2-fixtures build-wasm-extra ## Run exact Pillow ImageFont.getmask2 fixtures through Rust/Python/WASM
	$(MAKE) -C $(CORE_SRC) test-imagefont-getmask2-oracle
	$(PYTHON) -m pytest tests/test_parity.py tests/test_imagefont_oracle.py -q -k "ImageFont and getmask2 or default_font_name"
	cd $(JS_SRC) && npm run test:imagefont-getmask2-oracle

transposed-font-oracle: build-wasm-extra ## Run exact Pillow TransposedFont paths through Rust/Python/WASM
	$(PYTHON) scripts/generate_transposed_font_oracle.py
	$(MAKE) -C $(CORE_SRC) test-transposed-font-oracle
	$(PYTHON) -m pytest tests/test_transposed_font_oracle.py -q --strict-covers
	cd $(JS_SRC) && npm run test:transposed-font-oracle

fromarray-descriptor-oracle: build-wasm-core ## Run exact Pillow fromarray descriptors through Rust/Python/WASM
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fromarray_descriptor_oracle.py
	$(MAKE) -C $(CORE_SRC) test-fromarray-descriptor-oracle
	$(PYTHON) -m pytest tests/test_fromarray_descriptor_oracle.py -q --strict-covers
	cd $(JS_SRC) && npm run test:fromarray-descriptor-oracle

test-all: test-core test test-wasm ## Run core + Python + WASM tests

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
.PHONY: migration-parity-test migration-parity-oracle-identity migration-parity-target-identity migration-parity-coverage migration-parity-benchmark migration-parity-aggregate migration-parity-docs
.PHONY: font-tests font-tests-release imagingft-tests imagingft-tests-release pillow-rs-imagingft pillow-rs-imagingft-release
.PHONY: pillow-rs-fixtures-clean
.PHONY: pillow-rs-public-api-boundary pillow-rs-fmt pillow-rs-fmt-fix pillow-rs-clippy pillow-rs-lint
.PHONY: pillow-rs-build pillow-rs-build-release pillow-rs-bench
.PHONY: pillow-rs-ci pillow-rs-clean

pillow-rs-help: ## Show pillow-rs crate targets
	$(MAKE) -C $(CORE_SRC) help

pillow-rs-test: ## Run all pillow-rs Rust tests
	$(MAKE) -C $(CORE_SRC) test

image-backend-test: ## Run all image backend migration and forced-backend parity tests
	$(CARGO) test -p pillow-rs --all-features \
		--test image_backend_migration --test backend_parity --locked

image-backend-migration-test: ## Run image codec/backend migration fixtures
	$(CARGO) test -p pillow-rs --all-features \
		--test image_backend_migration --locked

image-backend-parity-test: ## Run Pillow-exact paste/drawing/transparency backend parity
	$(CARGO) test -p pillow-rs --all-features --test backend_parity --locked -- --nocapture

image-backend-feature-test: ## Verify disabled image codec feature forwarding
	$(CARGO) test -p pillow-rs --no-default-features --test image_feature_gates --locked

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
	set +e; \
	$(PYTHON) scripts/aggregate_migration_parity.py \
		--parity $(MIGRATION_PARITY_OUTPUT) \
		--coverage $(MIGRATION_COVERAGE_OUTPUT) \
		--benchmark $(MIGRATION_BENCHMARK_OUTPUT) \
		--output $(MIGRATION_STATUS_OUTPUT); \
	status=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	$(PYTHON) scripts/validate_migration_parity_result.py status $(MIGRATION_STATUS_OUTPUT)

migration-parity-docs: migration-parity-aggregate ## Generate specification and evidence documentation
	$(PYTHON) scripts/generate_migration_parity_docs.py \
		--status $(MIGRATION_STATUS_OUTPUT)

font-tests: ## Run Font public API parity tests
	$(MAKE) migration-parity-test

font-tests-release: ## Run Font public API parity tests (release)
	$(MAKE) -C $(CORE_SRC) font-tests-release

imagingft-tests: ## Compatibility alias for Font public API parity tests
	$(MAKE) -C $(CORE_SRC) imagingft-tests

imagingft-tests-release: ## Compatibility alias for Font public API parity tests (release)
	$(MAKE) -C $(CORE_SRC) imagingft-tests-release

pillow-rs-imagingft: ## Run legacy ImagingFT matrix parity tests
	$(MAKE) -C $(CORE_SRC) test-imagingft

pillow-rs-imagingft-release: ## Run legacy ImagingFT matrix parity tests (release)
	$(MAKE) -C $(CORE_SRC) test-imagingft-release

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

fixtures: fixtures-suite0 fixtures-suite1 ## Generate all test fixtures

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
	$(PYTHON) -m unittest tests.test_migration_parity_cases

migration-parity-evidence-check: ## Verify strict aggregate/result interfaces
	$(PYTHON) -m unittest tests.test_migration_parity_evidence

image-backend-fixtures: ## Generate exact Pillow image backend migration fixtures
	$(IMAGE_ORACLE_PYTHON) scripts/generate_image_backend_operation_fixtures.py

putdata-fixtures: ## Generate semantic Image.putdata inputs and exact Pillow oracles
	$(PYTHON) scripts/generate_putdata_fixture_inputs.py
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_DIR) --suite 0 --fixture Image.putdata
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1 --fixture Image.putdata

imagefont-getmask2-fixtures: ## Generate independent ImageFont.getmask2 inputs and exact Pillow oracles
	$(IMAGE_ORACLE_PYTHON) scripts/generate_imagefont_getmask2_fixture_inputs.py
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_DIR) --suite 0 --fixture ImageFont.getmask2
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1 --fixture ImageFont.getmask2

compact-value-fixtures: ## Regenerate compact exact getdata/flattened-data oracles
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0 \
		--fixture Image.getdata --fixture Image.get_flattened_data
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1 \
		--fixture Image.getdata --fixture Image.get_flattened_data

color3dlut-fixtures: ## Regenerate independent-path Color3DLUT Pillow oracles
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0 \
		--fixture ImageFilter.Color3DLUT
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1 \
		--fixture ImageFilter.Color3DLUT

point-fixtures: ## Regenerate Image.point Pillow oracles
	$(PYTHON) scripts/generate_point_fixture_inputs.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0
	$(PYTHON) scripts/generate_point_fixture_inputs.py \
		--fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0 --fixture Image.point
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1 --fixture Image.point

eval-fixtures: ## Regenerate Image.eval Pillow oracles
	$(IMAGE_ORACLE_PYTHON) scripts/generate_eval_error_oracle.py
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0 --fixture ImageModule.eval
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1 --fixture ImageModule.eval

palette-save-fixtures: ## Regenerate independent ImagePalette.save inputs and oracles
	$(PYTHON) scripts/generate_palette_save_fixture_inputs.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0
	$(PYTHON) scripts/generate_palette_save_fixture_inputs.py \
		--fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0 --fixture ImagePalette.save
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1 --fixture ImagePalette.save

image-io-fixtures: ## Regenerate independent suite0 Image open/save oracles
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0 \
		--fixture ImageModule.open --fixture Image.save

tobytes-fixtures: ## Regenerate independent suite0 Image.tobytes oracles
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py \
		--fixtures-dir $(FIXTURES_DIR) --suite 0 --fixture Image.tobytes

test-color3dlut: color3dlut-fixtures ## Run exact Color3DLUT Pillow parity
	$(PYTHON) -m pytest tests/test_parity.py -q -k Color3DLUT

fixture-coverage-check: fixtures ## Validate semantic fixture/manifest coverage
	PYTHONPATH=tests $(IMAGE_ORACLE_PYTHON) tests/fixture_coverage.py

fixtures-suite0: ## Generate suite0 fixtures
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_DIR) --suite 0

fixtures-suite1: ## Generate suite1 fixtures
	$(IMAGE_ORACLE_PYTHON) scripts/generate_fixtures.py --fixtures-dir $(FIXTURES_SUITE1_DIR) --suite 1

fixtures-clean: ## Remove fixture outputs
	chmod -R u+w $(FIXTURES_DIR)/outputs/ 2>/dev/null || true
	rm -rf $(FIXTURES_DIR)/outputs/
	mkdir -p $(FIXTURES_DIR)/outputs/{jsons,images,raws}

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
.PHONY: coverage coverage-python-abi-rust coverage-python-wrapper coverage-image-backend-rust
.PHONY: coverage-point-rust coverage-image-open-rust coverage-apply-transparency-rust coverage-paste-rust coverage-drawing-rust coverage-imagefont-getmask2-rust coverage-transposed-font-rust
.PHONY: coverage-font-rust coverage-font-rust-with-freetype coverage-imagingft-rust font-tests-coverage font-tests-coverage-with-freetype imagingft-tests-coverage
.PHONY: coverage-validate coverage-report coverage-wasm

coverage: ## Run tests + compute coverage
	@[ -f "$(REPORT)" ] || { echo "No test report found. Run: make test"; exit 1; }
	$(PYTHON) scripts/coverage/compute_coverage.py $(MANIFEST) $(REPORT)

coverage-python-abi-rust: ## Run Pillow parity through PyO3 and export Rust LLVM coverage
	PYTHON=$(PYTHON) MATURIN=$(MATURIN) TIMEOUT=$(TIMEOUT) REPORT=$(REPORT) \
		bash scripts/coverage/run_python_abi_rust_coverage.sh

coverage-point-rust: ## Run Image.point Pillow parity and export Rust LLVM branch coverage
	PYTHON=$(PYTHON) TIMEOUT=$(TIMEOUT) \
		bash scripts/coverage/run_point_rust_coverage.sh

coverage-image-open-rust: ## Run Image.open Pillow parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_image_open_rust_coverage.sh

coverage-apply-transparency-rust: ## Run Image.apply_transparency parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_apply_transparency_rust_coverage.sh

coverage-paste-rust: ## Run Image.paste parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_paste_rust_coverage.sh

coverage-drawing-rust: ## Run ImageDraw parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_drawing_rust_coverage.sh

coverage-imagefont-getmask2-rust: ## Run ImageFont.getmask2 parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_imagefont_getmask2_rust_coverage.sh

coverage-font-rust: ## Run Font public API parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_font_rust_coverage.sh

coverage-font-rust-with-freetype: ## Run Font public API parity and export Rust LLVM branch coverage including fontdone
	bash scripts/coverage/run_font_rust_with_freetype_coverage.sh

coverage-imagingft-rust: ## Compatibility alias for Font public API Rust coverage
	bash scripts/coverage/run_imagingft_rust_coverage.sh

font-tests-coverage: ## Run Font public API parity via coverage and export Rust LLVM branch coverage
	bash scripts/coverage/run_font_rust_coverage.sh

font-tests-coverage-with-freetype: ## Run Font public API parity coverage including fontdone
	bash scripts/coverage/run_font_rust_with_freetype_coverage.sh

imagingft-tests-coverage: ## Compatibility alias for Font public API Rust coverage
	bash scripts/coverage/run_imagingft_rust_coverage.sh

coverage-transposed-font-rust: ## Run TransposedFont parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_transposed_font_rust_coverage.sh

coverage-image-backend-rust: ## Run exact backend parity and export Rust LLVM branch coverage
	bash scripts/coverage/run_image_backend_rust_coverage.sh

coverage-python-wrapper: ## Run Pillow parity and export Python wrapper branch coverage
	PYTHON=$(PYTHON) TIMEOUT=$(TIMEOUT) REPORT=$(REPORT) \
		bash scripts/coverage/run_python_wrapper_coverage.sh

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

ci: repo-map-check fmt clippy pillow-rs-test-core font-tests test coverage-validate ## Full CI pipeline
	@echo "=== done ==="

verify: ci fontdone-parity ## Full workspace CI plus FreeType parity
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
