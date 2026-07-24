# pillow-rs Makefile
# ===================
# Single entry point for all build, test, lint, bench, coverage, and release workflows.
# Run `make` or `make help` to see all targets.

# ── Variables ─────────────────────────────────────────────────────────────────
MATURIN      := maturin
PYTHON       := python3
IMAGE_ORACLE_PYTHON ?= $(abspath ../image-slash-star/.oracle-venv/bin/python)
NODE         := node
CARGO        := cargo
WASM_PACK    := wasm-pack
MANIFEST     := manifest.yaml
PY_SRC       := pillow-rs-py
JS_SRC       := pillow-rs-js
CORE_SRC     := pillow-rs
FONTDONE_SRC := pillow-rs-freetype
FIXTURES_DIR := tests/fixtures
FIXTURES_SUITE1_DIR := tests/fixtures_2
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
	@printf "  $(CYAN)make test-putdata$(NC)   Run Image.putdata public and fixture parity\n"
	@printf "  $(CYAN)make test-imagefont-getmask2$(NC) Run independent ImageFont.getmask2 Pillow fixtures\n"
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
	@printf "  $(CYAN)make rust-image-open-oracle$(NC) Run Image.open corpus through Rust API\n"
	@printf "  $(CYAN)make test-all$(NC)       Run core + Python + WASM tests\n"
	@printf "  $(CYAN)make image-backend-test$(NC) Run image backend migration parity\n"
	@printf "  $(CYAN)make image-backend-migration-test$(NC) Run codec/backend migration fixtures\n"
	@printf "  $(CYAN)make image-backend-parity-test$(NC) Run forced-backend Pillow parity\n"
	@printf "  $(CYAN)make image-backend-feature-test$(NC) Verify disabled codec forwarding\n"
	@printf "  $(CYAN)make parity$(NC)        Run pillow-rs imagingft + fontdone unified parity\n"
	@printf "\n$(BOLD)pillow-rs / core crate$(NC)\n"
	@printf "  $(CYAN)make pillow-rs-help$(NC) Show crate-local pillow-rs targets\n"
	@printf "  $(CYAN)make pillow-rs-test$(NC) Run all pillow-rs Rust tests\n"
	@printf "  $(CYAN)make pillow-rs-imagingft$(NC) Run imagingft matrix parity tests\n"
	@printf "  $(CYAN)make pillow-rs-fixtures$(NC) Regenerate imagingft fixture matrix\n"
	@printf "  $(CYAN)make pillow-rs-fixtures-check$(NC) Verify imagingft fixtures reproduce exactly\n"
	@printf "  $(CYAN)make pillow-rs-lint$(NC) Run pillow-rs fmt + clippy\n"
	@printf "  $(CYAN)make pillow-rs-ci$(NC)   Run pillow-rs CI sequence\n"
	@printf "\n$(BOLD)fontdone / FreeType parity$(NC)\n"
	@printf "  $(CYAN)make fontdone-help$(NC)  Show crate-local fontdone targets\n"
	@printf "  $(CYAN)make fontdone-ci$(NC)    Run fontdone docs, lint, tests, parity, FFI, bench contracts\n"
	@printf "  $(CYAN)make fontdone-test$(NC)  Run all fontdone tests\n"
	@printf "  $(CYAN)make fontdone-parity$(NC) Run the FreeType parity matrix harness\n"
	@printf "  $(CYAN)make fontdone-ffi$(NC)   Run the no-runtime-FFI guard\n"
	@printf "  $(CYAN)make fontdone-ffi-compat$(NC) Run FreeType-shaped facade tests\n"
	@printf "  $(CYAN)make fontdone-doc$(NC)   Build strict fontdone rustdoc\n"
	@printf "  $(CYAN)make fontdone-bench$(NC) Run Rust vs C FreeType benchmark report\n"
	@printf "  $(CYAN)make fontdone-fixtures$(NC) Regenerate FreeType fixture families\n"
	@printf "\n$(BOLD)Fixtures$(NC)\n"
	@printf "  $(CYAN)make fixtures$(NC)       Generate all test fixtures (requires Pillow)\n"
	@printf "  $(CYAN)make imagingft-fixtures$(NC) Generate ignored PIL imagingft fixture matrix\n"
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
	@command -v $(MATURIN) >/dev/null 2>&1 || { echo "Installing maturin..."; pip install maturin; }
	@command -v $(WASM_PACK) >/dev/null 2>&1 || { echo "Installing wasm-pack..."; cargo install wasm-pack; }
	@[ -n "$$VIRTUAL_ENV" ] || [ -n "$$CONDA_PREFIX" ] || echo "⚠️  No virtualenv detected — consider: python3 -m venv .venv && source .venv/bin/activate"
	pip install maturin coverage pillow==12.2.0 numpy pyyaml pytest pytest-timeout pytest-json-report pytest-benchmark

setup-ci: ## Install dev deps for CI
	pip install maturin coverage pillow==12.2.0 numpy pyyaml pytest pytest-timeout pytest-json-report pytest-benchmark

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
.PHONY: test test-suite0 test-suite1 test-suite2 test-putdata test-imagefont-getmask2 test-point test-eval test-palette-save test-image-io test-tobytes test-compact-values
.PHONY: test-core test-wasm test-all rust-color3dlut-oracle rust-eval-oracle rust-image-open-oracle rust-tobytes-oracle js-oracle-contract js-color3dlut-oracle
.PHONY: js-eval-oracle js-image-open-oracle js-tobytes-oracle apply-transparency-oracle paste-oracle drawing-oracle imagefont-getmask2-oracle transposed-font-oracle
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

test-core: ## Run Rust core tests (pillow-rs unit + imagingft)
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

test-all: test-core test test-wasm ## Run core + Python + WASM tests

.PHONY: parity
parity: pillow-rs-imagingft fontdone-parity ## Run pillow-rs imagingft + fontdone unified parity

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

# ── pillow-rs / core crate ──────────────────────────────────────────────────
.PHONY: pillow-rs-help pillow-rs-test pillow-rs-test-core pillow-rs-imagingft
.PHONY: image-backend-test image-backend-migration-test image-backend-parity-test image-backend-feature-test
.PHONY: pillow-rs-imagingft-release pillow-rs-fixtures pillow-rs-fixtures-check
.PHONY: pillow-rs-fixtures-clean
.PHONY: pillow-rs-fmt pillow-rs-fmt-fix pillow-rs-clippy pillow-rs-lint
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

pillow-rs-imagingft: ## Run imagingft matrix parity tests
	$(MAKE) -C $(CORE_SRC) PYTHON=$(IMAGE_ORACLE_PYTHON) test-imagingft

pillow-rs-imagingft-release: ## Run imagingft parity (release)
	$(MAKE) -C $(CORE_SRC) PYTHON=$(IMAGE_ORACLE_PYTHON) test-imagingft-release

pillow-rs-fixtures: ## Regenerate imagingft fixture matrix
	$(MAKE) -C $(CORE_SRC) PYTHON=$(IMAGE_ORACLE_PYTHON) fixtures

pillow-rs-fixtures-check: ## Verify imagingft fixtures reproduce byte-for-byte
	$(MAKE) -C $(CORE_SRC) PYTHON=$(IMAGE_ORACLE_PYTHON) fixtures-check

pillow-rs-fixtures-clean: ## Remove imagingft fixture outputs
	$(MAKE) -C $(CORE_SRC) fixtures-clean

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

fontdone-ffi-compat: ## Run FreeType-shaped facade tests
	$(MAKE) -C $(FONTDONE_SRC) test-ffi-compat

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

# ── Fixtures ──────────────────────────────────────────────────────────────────
.PHONY: fixtures imagingft-fixtures image-backend-fixtures putdata-fixtures
.PHONY: imagefont-getmask2-fixtures
.PHONY: compact-value-fixtures color3dlut-fixtures point-fixtures eval-fixtures
.PHONY: palette-save-fixtures image-io-fixtures tobytes-fixtures test-color3dlut
.PHONY: fixture-coverage-check
.PHONY: fixtures-suite0 fixtures-suite1 fixtures-clean

fixtures: fixtures-suite0 fixtures-suite1 ## Generate all test fixtures

imagingft-fixtures: ## Generate ignored PIL imagingft fixture matrix
	$(IMAGE_ORACLE_PYTHON) pillow-rs/scripts/build_imagingft_fixtures.py

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
	$(CARGO) fmt --check

fmt-fix: ## Fix Rust formatting
	$(CARGO) fmt

clippy: ## Run clippy on all targets
	$(CARGO) clippy --all-targets --all-features -- -A deprecated

clippy-core: ## Run clippy on core only
	$(CARGO) clippy -p $(CORE_SRC) -- -A deprecated

lint: fmt clippy ## Run fmt + clippy

# ── Coverage ──────────────────────────────────────────────────────────────────
.PHONY: coverage coverage-python-abi-rust coverage-python-wrapper coverage-image-backend-rust
.PHONY: coverage-point-rust coverage-image-open-rust coverage-apply-transparency-rust coverage-paste-rust coverage-drawing-rust coverage-imagefont-getmask2-rust coverage-transposed-font-rust
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

ci: repo-map-check fmt clippy pillow-rs-test-core pillow-rs-imagingft test coverage-validate ## Full CI pipeline
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
