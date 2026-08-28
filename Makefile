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
FONTDONE_REPO ?= https://github.com/appunni-m/fontdone.git
FONTDONE_REF ?= 95dc33f12790c896f9a5c95571eec4360f22412e
FONTDONE_SRC ?= build/fontdone-src
IMAGE_SLASH_STAR_SRC := $(abspath ../image-slash-star)
IMAGE_SLASH_STAR_AVIF_LIB_DIR ?= $(shell p="$$(find "$(IMAGE_SLASH_STAR_SRC)/.oracle-venv" -name 'libavif*' -type f -print -quit 2>/dev/null)"; if [ -n "$$p" ]; then dirname "$$p"; fi)
FIXTURES_DIR := pillow-rs/tests/fixtures
REPORT       := /tmp/report.json
TIMEOUT      := 300
MIGRATION_PARITY_OUTPUT ?= build/migration-parity/parity-result.json
MIGRATION_PARITY_ARGS ?=
MIGRATION_PARITY_CASE_IDS ?=
MIGRATION_PARITY_CASE_ID_LIST = $(strip $(subst $(MIGRATION_COMMA),$(MIGRATION_SPACE),$(MIGRATION_PARITY_CASE_IDS)))
MIGRATION_PARITY_CASE_ARGS = $(foreach case_id,$(MIGRATION_PARITY_CASE_ID_LIST),--case-id '$(case_id)')
MIGRATION_PARITY_CASE ?= $(CASE_ID)
MIGRATION_PARITY_CASE_OUTPUT ?= build/migration-parity/parity-case-result.json
MIGRATION_COVERAGE_OUTPUT ?= build/migration-parity/coverage-result.json
MIGRATION_RUST_COVERAGE_OUTPUT ?= build/migration-parity/coverage-result-rust.json
MIGRATION_COVERAGE_REPORT ?= target/coverage/migration-parity-python.json
MIGRATION_PILLOW_COVERAGE_OUTPUT ?= build/migration-parity/pillow-oracle-coverage-result.json
MIGRATION_PILLOW_COVERAGE_REPORT ?= target/coverage/migration-parity-pillow.json
MIGRATION_PILLOW_COVERAGE_DATA ?= target/coverage/.migration-parity-pillow
MIGRATION_PILLOW_MISSING_MANIFEST ?= docs/coverage-pillow-missing-feature-manifest.json
MIGRATION_PILLOW_MISSING_MARKDOWN ?= docs/coverage-pillow-missing-feature-manifest.md
MIGRATION_COVERAGE_OPERATION ?=
MIGRATION_COVERAGE_EXCLUDE_CASE_IDS ?=
MIGRATION_COVERAGE_EXCLUDE_ARGS := $(foreach case_id,$(MIGRATION_COVERAGE_EXCLUDE_CASE_IDS),--exclude-case-id '$(case_id)')
MIGRATION_EMPTY :=
MIGRATION_SPACE := $(MIGRATION_EMPTY) $(MIGRATION_EMPTY)
MIGRATION_COMMA := ,
MIGRATION_COVERAGE_CASE_IDS ?=
MIGRATION_COVERAGE_CASE_ID_LIST := $(strip $(subst $(MIGRATION_COMMA),$(MIGRATION_SPACE),$(MIGRATION_COVERAGE_CASE_IDS)))
MIGRATION_COVERAGE_CASE_ARGS := $(foreach case_id,$(MIGRATION_COVERAGE_CASE_ID_LIST),--case-id '$(case_id)')
MIGRATION_ALL_BACKENDS_CASE_IDS ?= $(MIGRATION_PARITY_CASE_IDS)
MIGRATION_ALL_BACKENDS_CASE_ID_LIST := $(strip $(subst $(MIGRATION_COMMA),$(MIGRATION_SPACE),$(MIGRATION_ALL_BACKENDS_CASE_IDS)))
MIGRATION_ALL_BACKENDS_CASE_ARGS := $(foreach case_id,$(MIGRATION_ALL_BACKENDS_CASE_ID_LIST),--case-id '$(case_id)')
MIGRATION_TEST_CASE_IDS ?= $(MIGRATION_ALL_BACKENDS_CASE_IDS)
MIGRATION_TEST_ALL_BACKENDS_OUTPUT ?= $(if $(strip $(MIGRATION_TEST_CASE_IDS)),build/migration-parity/incremental/all-backends-test-result.json,$(MIGRATION_ALL_BACKENDS_OUTPUT))
MIGRATION_TEST_PILLOW_OUTPUT ?= $(if $(strip $(MIGRATION_TEST_CASE_IDS)),build/migration-parity/incremental/pillow-oracle-coverage-result.json,$(MIGRATION_PILLOW_COVERAGE_OUTPUT))
MIGRATION_TEST_PILLOW_REPORT ?= $(if $(strip $(MIGRATION_TEST_CASE_IDS)),build/migration-parity/incremental/migration-parity-pillow.json,$(MIGRATION_PILLOW_COVERAGE_REPORT))
MIGRATION_TEST_PILLOW_DATA ?= $(if $(strip $(MIGRATION_TEST_CASE_IDS)),build/migration-parity/incremental/.migration-parity-pillow,$(MIGRATION_PILLOW_COVERAGE_DATA))
MIGRATION_TEST_MISSING_MANIFEST ?= $(if $(strip $(MIGRATION_TEST_CASE_IDS)),build/migration-parity/incremental/coverage-pillow-missing-feature-manifest.json,$(MIGRATION_PILLOW_MISSING_MANIFEST))
MIGRATION_TEST_MISSING_MARKDOWN ?= $(if $(strip $(MIGRATION_TEST_CASE_IDS)),build/migration-parity/incremental/coverage-pillow-missing-feature-manifest.md,$(MIGRATION_PILLOW_MISSING_MARKDOWN))
MIGRATION_ALL_BACKENDS_OUTPUT ?= build/migration-parity/all-backends-test-result.json
MIGRATION_JS_PARITY_OUTPUT ?= build/migration-parity/js-wasm-parity-result.json
MIGRATION_BROWSER_PARITY_OUTPUT ?= build/migration-parity/browser-wasm-parity-result.json
MIGRATION_JS_PARITY_CHUNK_SIZE ?= 128
MIGRATION_BROWSER_PARITY_CHUNK_SIZE ?= 512
MIGRATION_JS_GAP_MANIFEST ?= build/migration-parity/js-wasm-gap-manifest.json
MIGRATION_JS_GAP_MARKDOWN ?= docs/coverage-js-wasm-gap-manifest.md
MIGRATION_JS_PARITY_CASE_IDS ?= $(MIGRATION_PARITY_CASE_IDS)
MIGRATION_JS_PARITY_CASE_ID_LIST := $(strip $(subst $(MIGRATION_COMMA),$(MIGRATION_SPACE),$(MIGRATION_JS_PARITY_CASE_IDS)))
MIGRATION_JS_PARITY_CASE_ARGS := $(foreach case_id,$(MIGRATION_JS_PARITY_CASE_ID_LIST),--case-id '$(case_id)')
MIGRATION_ALL_BACKENDS_TIMEOUT ?= 7200
MIGRATION_GPU_FULL ?= 1
MIGRATION_GPU_FULL_ARG := $(if $(filter 0 false no,$(MIGRATION_GPU_FULL)),--no-gpu-full,--gpu-full)
MIGRATION_GPU_STRICT_OUTPUT ?= build/migration-parity/gpu-strict-parity-result.json
MIGRATION_OPERATION_COVERAGE_OUTPUT ?= build/migration-parity/coverage-operation-rust.json
MIGRATION_OPERATION_COVERAGE_REPORT ?= target/coverage/migration-parity-operation-python.json
MIGRATION_OPERATION_LLVM_REPORT ?= target/coverage/migration-parity-operation-rust.json
MIGRATION_BENCHMARK_OUTPUT ?= build/migration-parity/benchmark-result.json
MIGRATION_BENCHMARK_PARITY_OUTPUT ?= build/migration-parity/benchmark-parity-result.json
MIGRATION_BENCHMARK_ARGS ?=
MIGRATION_BENCHMARK_COVERAGE_RESULT ?= $(MIGRATION_BENCHMARK_OUTPUT)
MIGRATION_BENCHMARK_COVERAGE_OUTPUT ?= build/migration-parity/pipeline-benchmark-coverage.json
MIGRATION_BENCHMARK_REPORT_OUTPUT ?= build/migration-parity/pipeline-performance-report.json
MIGRATION_BENCHMARK_BASELINE ?=
MIGRATION_BENCHMARK_ROADMAP_STATUS_OUTPUT ?= build/migration-parity/pipeline-roadmap-status.json
MIGRATION_BENCHMARK_BUDGET_OUTPUT ?= build/migration-parity/pipeline-budget-check.json
MIGRATION_BENCHMARK_BUDGET_BASELINE ?=
MIGRATION_BENCHMARK_PROFILE ?= standard
MIGRATION_PROFILE_WORKLOAD_ID ?= pipeline.quick.gaussianblur-invert.rgb-1024
MIGRATION_PROFILE_BACKEND ?= cpu
MIGRATION_PROFILE_REPEAT ?= 40
MIGRATION_PROFILE_TIMEOUT ?= 180
MIGRATION_PROFILE_OUTPUT_DIR ?= build/migration-parity/profiles
MIGRATION_CORE_BENCHMARK_ARGS ?=
MIGRATION_CORE_BENCHMARK_OUTPUT ?= build/migration-parity/pipeline-core-benchmark.json
PILLOW_RS_PY_BENCHMARK_ARGS ?=
PILLOW_RS_PY_BENCHMARK_OUTPUT ?= build/migration-parity/pillow-rs-py-binding-benchmark.json
MIGRATION_BENCHMARK_QUICK_WORKLOADS := \
	pipeline.quick.transpose-twice.rgb-1024 \
	pipeline.quick.gaussianblur-invert.rgb-1024 \
	pipeline.quick.multiply-screen.rgb-1024 \
	pipeline.quick.invert-mirror.rgb-1024
MIGRATION_BENCHMARK_PROFILE_ARGS_standard :=
MIGRATION_BENCHMARK_PROFILE_ARGS_quick := $(foreach workload,$(MIGRATION_BENCHMARK_QUICK_WORKLOADS),--workload-id $(workload))
MIGRATION_BENCHMARK_PROFILE_ARGS_pipeline := --pipeline
MIGRATION_BENCHMARK_PROFILE_ARGS := $(MIGRATION_BENCHMARK_PROFILE_ARGS_$(MIGRATION_BENCHMARK_PROFILE))
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
	@printf "  $(CYAN)make test$(NC)           Run shared CPU/SIMD/GPU/WGSL + JS/WASM parity and reverse Pillow coverage\n"
	@printf "  $(CYAN)MIGRATION_GPU_FULL=0 make test$(NC) Keep the GPU smoke gate but mark full GPU parity not proven\n"
	@printf "  $(CYAN)make test-wasm$(NC)      Run the same public parity corpus through Node WASM and browser WASM\n"
	@printf "  $(CYAN)make migration-parity-js-gap-report$(NC)  Group actual Node/browser WASM failures; pending means summary.not_run\n"
	@printf "  $(CYAN)make migration-parity-fixtures-check$(NC) Verify the fixed manifest and indexed inputs\n"
	@printf "  $(CYAN)make migration-parity-crash-quarantine-check$(NC) Verify isolated crash inputs without executing them\n"
	@printf "  $(CYAN)make migration-parity-case-review$(NC) Verify duplicate selection and nuanced cases\n"
	@printf "  $(CYAN)make migration-parity-evidence-check$(NC) Validate strict result interfaces\n"
	@printf "  $(CYAN)make migration-parity-benchmark$(NC) Compare Pillow vs CPU, SIMD, and GPU\n"
	@printf "  $(CYAN)MIGRATION_BENCHMARK_PROFILE=quick make migration-parity-benchmark$(NC) Run the representative four-workload smoke benchmark\n"
	@printf "  $(CYAN)MIGRATION_BENCHMARK_PROFILE=pipeline make migration-parity-benchmark$(NC) Run every PipelineOp and public composition workload\n"
	@printf "  $(CYAN)make test-all$(NC)       Run the all-backend public parity campaign\n"
	@printf "  $(CYAN)make migration-parity-test$(NC) Run the canonical live-oracle migration parity suite\n"
	@printf "  $(CYAN)make migration-parity-test-gpu-strict$(NC) Audit GPU-only capability coverage (not the normal fallback lane)\n"
	@printf "  $(CYAN)make migration-parity-case CASE_ID=...$(NC) Run one public parity case for fast iteration\n"
	@printf "  $(CYAN)make migration-parity-oracle-identity$(NC) Verify the pinned Pillow oracle identity\n"
	@printf "  $(CYAN)make migration-parity-target-identity$(NC) Verify the public pillow-rs target identity\n"
	@printf "  $(CYAN)make migration-parity-coverage$(NC) Run target coverage from indexed coverage plans\n"
	@printf "  $(CYAN)make migration-parity-pillow-coverage$(NC) Run the complete public parity corpus against Pillow Python source\n"
	@printf "  $(CYAN)make migration-parity-pillow-missing-manifest$(NC) Order Pillow source gaps by missing lines/branches\n"
	@printf "  $(CYAN)make test MIGRATION_TEST_CASE_IDS=case-a,case-b$(NC) Isolate a filtered target/reverse snapshot under build/migration-parity/incremental\n"
	@printf "  $(CYAN)make migration-parity-coverage-rust$(NC) Run merged Python+Rust coverage with a temporary instrumented extension\n"
	@printf "  $(CYAN)make migration-parity-coverage-rust MIGRATION_COVERAGE_CASE_IDS=case-a,case-b$(NC) Run exact cases for incremental coverage\n"
	@printf "  $(CYAN)make migration-parity-operation-coverage MIGRATION_COVERAGE_OPERATION=PIL.Image.Image.getbbox$(NC) Run scoped operation coverage\n"
	@printf "  $(CYAN)make migration-parity-font-native-coverage$(NC) Run the font-native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imageops-native-coverage$(NC) Run the image-ops native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imagesequence-native-coverage$(NC) Run the image-sequence native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imagecore-native-coverage$(NC) Run the image-core native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imagedraw-native-coverage$(NC) Run the image-draw native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imagecolor-native-coverage$(NC) Run the image-color native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-imagepalette-native-coverage$(NC) Run the image-palette native coverage-only corpus\n"
	@printf "  $(CYAN)make migration-parity-region-coverage$(NC) Report region coverage per public operation\n"
	@printf "  $(CYAN)make migration-parity-pipeline-benchmark-coverage$(NC) Check every PipelineOp has a benchmark workload\n"
	@printf "  $(CYAN)make migration-parity-pipeline-report$(NC) Generate benchmark timing/backend/resource evidence\n"
	@printf "  $(CYAN)make migration-parity-pipeline-roadmap-status$(NC) Generate per-FIL roadmap status and denominator evidence\n"
	@printf "  $(CYAN)make migration-parity-pipeline-budget-check$(NC) Compare compatible benchmark lineages against guarded budgets\n"
	@printf "  $(CYAN)make migration-parity-profile$(NC) Capture a bounded CPU/SIMD/GPU adapter profile\n"
	@printf "  $(CYAN)make migration-parity-profile-all$(NC) Capture profiles for CPU, SIMD, and GPU\n"
	@printf "  $(CYAN)make migration-parity-pipeline-core-benchmark$(NC) Measure the direct pure-Rust pipeline boundary\n"
	@printf "  $(CYAN)make migration-parity-benchmark$(NC) Run correctness-gated benchmark workloads\n"
	@printf "  $(CYAN)make migration-parity-aggregate$(NC) Join compatible parity, coverage, and benchmark evidence\n"
	@printf "  $(CYAN)make migration-parity-docs$(NC) Generate specification and evidence documentation\n"
	@printf "  $(CYAN)make migration-parity-inventory$(NC) Print the canonical selected-scope endpoint inventory\n"
	@printf "  $(CYAN)make migration-parity-inventory-check$(NC) Verify endpoint authority expansion and alias accounting\n"
	@printf "  $(CYAN)make parity$(NC)        Run pillow-rs Font + fontdone unified parity\n"
	@printf "\n$(BOLD)pillow-rs / core crate$(NC)\n"
	@printf "  $(CYAN)make pillow-rs-help$(NC) Show crate-local pillow-rs targets\n"
	@printf "  $(CYAN)make pillow-rs-test$(NC) Run the public parity campaign for pillow-rs\n"
	@printf "  $(CYAN)make pillow-rs-lint$(NC) Run pillow-rs fmt + clippy\n"
	@printf "  $(CYAN)make pillow-rs-ci$(NC)   Run pillow-rs CI sequence\n"
	@printf "\n$(BOLD)fontdone / FreeType parity$(NC)\n"
	@printf "  $(CYAN)make fontdone-help$(NC)  Show crate-local fontdone targets (pinned Git checkout)\n"
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
	@printf "  $(CYAN)make pillow-rs-py-binding-benchmark$(NC) Release-only PyO3 GIL/concurrency benchmark\n"
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
	$(PIP) install maturin coverage pillow==12.2.0 numpy pyyaml
	cd $(JS_SRC) && npm ci

setup-ci: ## Install dev deps for CI
	@$(PYTHON) -m pip --version >/dev/null 2>&1 || { echo "Bootstrapping pip..."; $(PYTHON) -m ensurepip --upgrade; }
	$(PIP) install maturin coverage pillow==12.2.0 numpy pyyaml
	cd $(JS_SRC) && npm ci

# ── Build ─────────────────────────────────────────────────────────────────────
.PHONY: build build-dev build-wasm build-wasm-core build-wasm-extra build-wasm-release build-all

build: ## Build Python package (release)
	$(MATURIN) develop --manifest-path $(PY_SRC)/Cargo.toml --release

build-dev: ## Build Python package (debug, faster compile)
	$(MATURIN) develop --manifest-path $(PY_SRC)/Cargo.toml

build-wasm: build-wasm-core ## Build the default core WASM package (dev)

build-wasm-core: ## Build all-codec core WASM package (dev)
	cd $(JS_SRC) && npm run build:core

build-wasm-extra: ## Build Rust-codec extra WASM package (dev)
	cd $(JS_SRC) && npm run build:extra

build-wasm-release: ## Build WASM package (release)
	cd $(JS_SRC) && npm run build:release

build-all: build build-wasm-release ## Build Python + WASM

# ── Test ──────────────────────────────────────────────────────────────────────
.PHONY: test test-wasm test-all migration-parity-js-gap-report migration-parity-test-all-backends migration-parity-test-gpu-strict
.PHONY: backend-support-matrix

test: migration-parity-fixtures-check ## Run shared target parity, JS/WASM parity, and reverse Pillow coverage
	set +e; \
	$(MAKE) migration-parity-test-all-backends \
		MIGRATION_ALL_BACKENDS_OUTPUT="$(MIGRATION_TEST_ALL_BACKENDS_OUTPUT)" \
		MIGRATION_ALL_BACKENDS_CASE_IDS="$(MIGRATION_TEST_CASE_IDS)"; \
	target_status=$$?; \
	$(MAKE) migration-parity-pillow-coverage \
		MIGRATION_PILLOW_COVERAGE_OUTPUT="$(MIGRATION_TEST_PILLOW_OUTPUT)" \
		MIGRATION_PILLOW_COVERAGE_REPORT="$(MIGRATION_TEST_PILLOW_REPORT)" \
		MIGRATION_PILLOW_COVERAGE_DATA="$(MIGRATION_TEST_PILLOW_DATA)" \
		MIGRATION_COVERAGE_CASE_IDS="$(MIGRATION_TEST_CASE_IDS)"; \
	reverse_status=$$?; \
	$(MAKE) migration-parity-pillow-missing-manifest \
		MIGRATION_PILLOW_COVERAGE_OUTPUT="$(MIGRATION_TEST_PILLOW_OUTPUT)" \
		MIGRATION_PILLOW_COVERAGE_REPORT="$(MIGRATION_TEST_PILLOW_REPORT)" \
		MIGRATION_PILLOW_MISSING_MANIFEST="$(MIGRATION_TEST_MISSING_MANIFEST)" \
		MIGRATION_PILLOW_MISSING_MARKDOWN="$(MIGRATION_TEST_MISSING_MARKDOWN)"; \
	manifest_status=$$?; \
	if [ $$target_status -ne 0 ]; then exit $$target_status; fi; \
	if [ $$reverse_status -ne 0 ]; then exit $$reverse_status; fi; \
	exit $$manifest_status

backend-support-matrix: ## Emit registry-derived CPU/SIMD/GPU support JSON
	$(MAKE) -C $(CORE_SRC) backend-support-matrix

test-wasm: build-wasm-core build-wasm-extra ## Build the declared WASM packages and run the same public corpus through Node and browser WASM
	cd $(JS_SRC) && npm run test:package
	set +e; \
	$(PYTHON) scripts/run_migration_js_parity.py \
		--host node \
		--output "$(MIGRATION_JS_PARITY_OUTPUT)" \
		--chunk-size "$(MIGRATION_JS_PARITY_CHUNK_SIZE)" \
		$(MIGRATION_JS_PARITY_CASE_ARGS); \
	node_status=$$?; \
	$(PYTHON) scripts/run_migration_js_parity.py \
		--host browser \
		--output "$(MIGRATION_BROWSER_PARITY_OUTPUT)" \
		--chunk-size "$(MIGRATION_BROWSER_PARITY_CHUNK_SIZE)" \
		$(MIGRATION_JS_PARITY_CASE_ARGS); \
	browser_status=$$?; \
	if [ $$node_status -ne 0 ]; then exit $$node_status; fi; \
	exit $$browser_status

migration-parity-js-gap-report: ## Group actual Node/browser WASM failures without loading 1+ GiB artifacts into memory
	$(PYTHON) scripts/report_migration_js_parity_gaps.py \
		--node "$(MIGRATION_JS_PARITY_OUTPUT)" \
		--browser "$(MIGRATION_BROWSER_PARITY_OUTPUT)" \
		--json-out "$(MIGRATION_JS_GAP_MANIFEST)" \
		--markdown-out "$(MIGRATION_JS_GAP_MARKDOWN)"

test-all: test ## Run the all-backend public parity campaign

.PHONY: parity
parity: font-tests fontdone-parity ## Run pillow-rs Font + fontdone unified parity

# ── fontdone / FreeType parity ───────────────────────────────────────────────
.PHONY: fontdone-source fontdone-help fontdone-build fontdone-doc fontdone-doc-test
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
.PHONY: pillow-rs-help pillow-rs-test
.PHONY: image-backend-test image-backend-migration-test image-backend-parity-test image-backend-feature-test
.PHONY: migration-parity-test migration-parity-case migration-parity-oracle-identity migration-parity-target-identity migration-parity-coverage migration-parity-pillow-coverage migration-parity-pillow-missing-manifest migration-parity-coverage-rust migration-parity-operation-coverage migration-parity-font-native-coverage migration-parity-region-coverage migration-parity-pipeline-benchmark-coverage migration-parity-pipeline-report migration-parity-pipeline-roadmap-status migration-parity-pipeline-budget-check migration-parity-profile migration-parity-profile-all migration-parity-benchmark migration-parity-pipeline-core-benchmark migration-parity-aggregate migration-parity-docs pillow-rs-py-binding-benchmark
.PHONY: font-tests font-tests-release imagingft-tests imagingft-tests-release pillow-rs-imagingft pillow-rs-imagingft-release
.PHONY: pillow-rs-fixtures-clean
.PHONY: pillow-rs-public-api-boundary pillow-rs-fmt pillow-rs-fmt-fix pillow-rs-clippy pillow-rs-lint
.PHONY: pillow-rs-build pillow-rs-build-release pillow-rs-bench
.PHONY: pillow-rs-ci pillow-rs-clean

pillow-rs-help: ## Show pillow-rs crate targets
	$(MAKE) -C $(CORE_SRC) help

pillow-rs-test: ## Run the public parity campaign for pillow-rs
	$(MAKE) migration-parity-test-all-backends

image-backend-test image-backend-migration-test image-backend-parity-test image-backend-feature-test:
	@printf "This legacy image-backend parity target is archived under deprecated/migration-parity-v0.\n"
	@printf "Use 'make migration-parity-test' with the active manifest-driven inputs.\n"
	@exit 2

migration-parity-test: ## Run canonical input-only parity against live Pillow
	set +e; \
	$(PYTHON) scripts/run_migration_parity.py --output $(MIGRATION_PARITY_OUTPUT) $(MIGRATION_PARITY_ARGS) $(MIGRATION_PARITY_CASE_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py parity $(MIGRATION_PARITY_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-test-gpu-strict: build-dev ## Audit GPU-only capability coverage without CPU fallback
	$(MAKE) migration-parity-test \
		MIGRATION_TARGET_BACKEND=gpu \
		MIGRATION_STRICT_TARGET_BACKEND=1 \
		MIGRATION_PARITY_OUTPUT="$(MIGRATION_GPU_STRICT_OUTPUT)"

migration-parity-test-all-backends: build-dev ## Build once, then run CPU, SIMD, bounded full GPU, Python, Node WASM, and browser WASM together
	set +e; \
	$(PYTHON) scripts/run_all_backend_tests.py \
		--output "$(MIGRATION_ALL_BACKENDS_OUTPUT)" \
		--timeout "$(MIGRATION_ALL_BACKENDS_TIMEOUT)" \
		$(MIGRATION_ALL_BACKENDS_CASE_ARGS) \
		$(MIGRATION_GPU_FULL_ARG); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py all_backends "$(MIGRATION_ALL_BACKENDS_OUTPUT)"; \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-case: ## Run one public parity case without replacing the full-suite artifact
	@test -n "$(MIGRATION_PARITY_CASE)" || { \
		printf "Set MIGRATION_PARITY_CASE to an active case_id.\n" >&2; \
		exit 2; \
	}
	set +e; \
	$(PYTHON) scripts/run_migration_parity.py \
		--output $(MIGRATION_PARITY_CASE_OUTPUT) \
		--case-id "$(MIGRATION_PARITY_CASE)" \
		$(MIGRATION_PARITY_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py parity $(MIGRATION_PARITY_CASE_OUTPUT); \
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
		--coverage-report $(MIGRATION_COVERAGE_REPORT) \
		$(MIGRATION_COVERAGE_EXCLUDE_ARGS) \
		$(MIGRATION_COVERAGE_CASE_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py coverage $(MIGRATION_COVERAGE_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-pillow-coverage: ## Run the indexed coverage plans against Pillow Python source
	$(PYTHON) scripts/run_migration_pillow_coverage.py \
		--output $(MIGRATION_PILLOW_COVERAGE_OUTPUT) \
		--coverage-report $(MIGRATION_PILLOW_COVERAGE_REPORT) \
		--coverage-data $(MIGRATION_PILLOW_COVERAGE_DATA) \
		--same-parity-corpus \
		$(if $(MIGRATION_COVERAGE_OPERATION),--operation '$(MIGRATION_COVERAGE_OPERATION)',) \
		$(MIGRATION_COVERAGE_EXCLUDE_ARGS) \
		$(MIGRATION_COVERAGE_CASE_ARGS)

migration-parity-pillow-missing-manifest: ## Order Pillow source gaps by missing lines and branches
	$(PYTHON) scripts/report_migration_pillow_missing.py \
		--manifest "$(MANIFEST)" \
		--report "$(MIGRATION_PILLOW_COVERAGE_REPORT)" \
		--receipt "$(MIGRATION_PILLOW_COVERAGE_OUTPUT)" \
		--output "$(MIGRATION_PILLOW_MISSING_MANIFEST)" \
		--markdown "$(MIGRATION_PILLOW_MISSING_MARKDOWN)"

migration-parity-coverage-rust: ## Run merged Python+Rust coverage with a temporary instrumented extension
	set +e; \
	$(PYTHON) scripts/run_migration_rust_coverage.py \
		--output $(MIGRATION_RUST_COVERAGE_OUTPUT) \
		$(MIGRATION_COVERAGE_EXCLUDE_ARGS) \
		$(MIGRATION_COVERAGE_CASE_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py coverage $(MIGRATION_RUST_COVERAGE_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-operation-coverage: ## Run merged coverage for one manifest public operation
	@test -n "$(MIGRATION_COVERAGE_OPERATION)" || { \
		printf "Set MIGRATION_COVERAGE_OPERATION to a manifest operation path.\n" >&2; \
		exit 2; \
	}
	set +e; \
	$(PYTHON) scripts/run_migration_rust_coverage.py \
		--operation "$(MIGRATION_COVERAGE_OPERATION)" \
		--output $(MIGRATION_OPERATION_COVERAGE_OUTPUT) \
		--python-report $(MIGRATION_OPERATION_COVERAGE_REPORT) \
		--llvm-report $(MIGRATION_OPERATION_LLVM_REPORT) \
		$(MIGRATION_COVERAGE_EXCLUDE_ARGS) \
		$(MIGRATION_COVERAGE_CASE_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py coverage $(MIGRATION_OPERATION_COVERAGE_OUTPUT); \
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

migration-parity-imagedraw-native-coverage: ## Run the image-draw native coverage-only corpus
	$(PYTHON) scripts/run_migration_imagedraw_native_cases.py

migration-parity-imagecolor-native-coverage: ## Run the image-color native coverage-only corpus
	$(PYTHON) scripts/run_migration_imagecolor_native_cases.py

migration-parity-imagepalette-native-coverage: ## Run the image-palette native coverage-only corpus
	$(PYTHON) scripts/run_migration_imagepalette_native_cases.py

migration-parity-region-coverage: ## Report region coverage per public operation
	$(PYTHON) scripts/report_migration_parity_region_coverage.py

migration-parity-pipeline-benchmark-coverage: ## Check the complete PipelineOp benchmark matrix
	$(PYTHON) scripts/report_pipeline_benchmark_coverage.py \
		--result "$(MIGRATION_BENCHMARK_COVERAGE_RESULT)" \
		--output "$(MIGRATION_BENCHMARK_COVERAGE_OUTPUT)"

migration-parity-pipeline-report: ## Generate benchmark timing/backend/resource evidence
	$(PYTHON) scripts/report_pipeline_performance.py \
		--result "$(MIGRATION_BENCHMARK_OUTPUT)" \
		--output "$(MIGRATION_BENCHMARK_REPORT_OUTPUT)" \
		$(if $(strip $(MIGRATION_BENCHMARK_BASELINE)),--baseline "$(MIGRATION_BENCHMARK_BASELINE)",)

migration-parity-pipeline-roadmap-status: ## Generate per-FIL roadmap status and denominator evidence
	$(PYTHON) scripts/report_pipeline_roadmap_status.py \
		--result "$(MIGRATION_BENCHMARK_OUTPUT)" \
		--output "$(MIGRATION_BENCHMARK_ROADMAP_STATUS_OUTPUT)" \
		--check

migration-parity-pipeline-budget-check: ## Compare compatible benchmark lineages against guarded budgets
	$(PYTHON) scripts/check_pipeline_benchmark_budgets.py \
		--current "$(MIGRATION_BENCHMARK_OUTPUT)" \
		--baseline "$(MIGRATION_BENCHMARK_BUDGET_BASELINE)" \
		--output "$(MIGRATION_BENCHMARK_BUDGET_OUTPUT)" \
		--check

migration-parity-profile: build ## Capture one bounded adapter profile without running unit tests
	$(PYTHON) scripts/profile_migration_benchmark.py \
		--workload-id "$(MIGRATION_PROFILE_WORKLOAD_ID)" \
		--backend "$(MIGRATION_PROFILE_BACKEND)" \
		--repeat "$(MIGRATION_PROFILE_REPEAT)" \
		--timeout "$(MIGRATION_PROFILE_TIMEOUT)" \
		--output-dir "$(MIGRATION_PROFILE_OUTPUT_DIR)"

migration-parity-profile-all: ## Capture bounded CPU, SIMD, and GPU adapter profiles
	$(MAKE) migration-parity-profile MIGRATION_PROFILE_BACKEND=cpu
	$(MAKE) migration-parity-profile MIGRATION_PROFILE_BACKEND=simd
	$(MAKE) migration-parity-profile MIGRATION_PROFILE_BACKEND=gpu

migration-parity-benchmark: build ## Build release, then run correctness-gated benchmark workloads
	@test "$(MIGRATION_BENCHMARK_PROFILE)" = standard -o "$(MIGRATION_BENCHMARK_PROFILE)" = quick -o "$(MIGRATION_BENCHMARK_PROFILE)" = pipeline || { \
		printf "MIGRATION_BENCHMARK_PROFILE must be 'standard', 'quick', or 'pipeline'.\n" >&2; \
		exit 2; \
	}
	set +e; \
	$(PYTHON) scripts/run_migration_benchmark.py \
		--output $(MIGRATION_BENCHMARK_OUTPUT) \
		--parity-output $(MIGRATION_BENCHMARK_PARITY_OUTPUT) \
		$(MIGRATION_BENCHMARK_PROFILE_ARGS) \
		$(MIGRATION_BENCHMARK_ARGS); \
	status=$$?; \
	$(PYTHON) scripts/validate_migration_parity_result.py benchmark $(MIGRATION_BENCHMARK_OUTPUT); \
	validator=$$?; \
	if [ $$status -ne 0 ]; then exit $$status; fi; \
	exit $$validator

migration-parity-pipeline-core-benchmark: ## Run the direct pure-Rust pipeline boundary benchmark
	@mkdir -p "$(dir $(MIGRATION_CORE_BENCHMARK_OUTPUT))"
	$(CARGO) run --manifest-path $(CORE_SRC)/Cargo.toml --release --locked --example pipeline_layers -- $(MIGRATION_CORE_BENCHMARK_ARGS) > "$(MIGRATION_CORE_BENCHMARK_OUTPUT)"

pillow-rs-py-binding-benchmark: build ## Run the release-only PyO3 boundary benchmark
	@mkdir -p "$(dir $(PILLOW_RS_PY_BENCHMARK_OUTPUT))"
	$(PYTHON) pillow-rs-py/bench/release_benchmark.py $(PILLOW_RS_PY_BENCHMARK_ARGS) > "$(PILLOW_RS_PY_BENCHMARK_OUTPUT)"

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
fontdone-source: ## Ensure the pinned GitHub fontdone checkout exists
	@if test -d "$(FONTDONE_SRC)/.git"; then \
		actual="$$(git -C "$(FONTDONE_SRC)" rev-parse HEAD 2>/dev/null || true)"; \
		if test "$$actual" != "$(FONTDONE_REF)"; then \
			printf "fontdone checkout at %s is %s, expected %s; use another FONTDONE_SRC or update it explicitly.\n" "$(FONTDONE_SRC)" "$$actual" "$(FONTDONE_REF)" >&2; \
			exit 2; \
		fi; \
	elif test -e "$(FONTDONE_SRC)"; then \
		printf "fontdone source path exists but is not a Git checkout: %s\n" "$(FONTDONE_SRC)" >&2; \
		exit 2; \
	else \
		mkdir -p "$$(dirname "$(FONTDONE_SRC)")"; \
		git clone --filter=blob:none --no-checkout "$(FONTDONE_REPO)" "$(FONTDONE_SRC)"; \
		git -C "$(FONTDONE_SRC)" checkout --detach "$(FONTDONE_REF)"; \
	fi

fontdone-help: ## Show fontdone targets
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) help

fontdone-build: ## Build fontdone
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) build

fontdone-doc: ## Build strict fontdone rustdoc
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) doc

fontdone-doc-test: ## Run fontdone doctests
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) doc-test

fontdone-test: ## Run fontdone non-oracle tests and all-target checks
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) test-fast

fontdone-parity: ## Run FreeType parity matrix tests
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) test-parity

fontdone-ffi: ## Run no-runtime-FFI guard
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) test-ffi

fontdone-ffi-compat: ## Run FreeType-shaped API/ABI facade audit
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) api-abi-check

fontdone-fmt: ## Check fontdone formatting
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) fmt

fontdone-fmt-fix: ## Apply fontdone formatting
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) fmt

fontdone-clippy: ## Run strict fontdone clippy
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) clippy

fontdone-lint: ## Run fontdone fmt + clippy
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) lint

fontdone-bench: ## Run Rust vs C FreeType benchmark report
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) bench

fontdone-bench-quick: ## Run short FreeType benchmark smoke comparison
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) bench-quick

fontdone-bench-self-test: ## Run benchmark tooling self-test
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) bench-self-test

fontdone-fixtures: ## Regenerate all FreeType fixture families
	@$(MAKE) fontdone-source
	$(MAKE) -C $(FONTDONE_SRC) font-fixtures

fontdone-ci: ## Run required fontdone local CI sequence
	@$(MAKE) fontdone-source
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
.PHONY: fixtures migration-parity-inventory migration-parity-inventory-check migration-parity-manifest migration-parity-inputs migration-parity-fixtures migration-parity-case-review migration-parity-fixtures-check migration-parity-inputs-check migration-parity-crash-quarantine-check migration-parity-evidence-check image-backend-fixtures putdata-fixtures
.PHONY: imagefont-getmask2-fixtures
.PHONY: compact-value-fixtures color3dlut-fixtures point-fixtures eval-fixtures
.PHONY: palette-save-fixtures image-io-fixtures tobytes-fixtures test-color3dlut
.PHONY: fixture-coverage-check
.PHONY: fixtures-suite0 fixtures-suite1 fixtures-clean

fixtures: migration-parity-fixtures ## Build the active manifest and input specifications

migration-parity-inventory: ## Print canonical selected-scope endpoint inventory
	$(PYTHON) scripts/migration_parity_inventory.py

migration-parity-inventory-check: ## Verify endpoint authority expansion and alias accounting
	$(PYTHON) scripts/migration_parity_inventory.py --format check

migration-parity-manifest: ## Build fixed project-wide manifest from frozen authority
	$(PYTHON) scripts/build_migration_parity_manifest.py

migration-parity-inputs: ## Build deterministic parity, coverage, and benchmark inputs
	$(PYTHON) scripts/build_migration_parity_inputs.py

migration-parity-fixtures: migration-parity-manifest migration-parity-inputs ## Compatibility alias during migration

migration-parity-case-review: migration-parity-inputs ## Review duplicate and nuanced active case selection
	$(PYTHON) scripts/review_migration_parity_cases.py

migration-parity-fixtures-check: migration-parity-inventory-check ## Verify authority and manifest regeneration
	@set -e; \
	tmp="$$(mktemp -d)"; \
	trap 'rm -rf "$$tmp"' EXIT; \
	$(PYTHON) scripts/build_migration_parity_manifest.py --output "$$tmp/manifest.yaml"; \
	diff -u pillow-rs/tests/fixtures/manifest.yaml "$$tmp/manifest.yaml"; \
	$(MAKE) migration-parity-inputs-check

.PHONY: migration-parity-inputs-check
migration-parity-inputs-check: ## Verify deterministic input regeneration
	$(PYTHON) scripts/check_migration_parity_inputs.py
	$(PYTHON) scripts/validate_migration_parity_contract.py --manifest "$(MANIFEST)"

migration-parity-crash-quarantine-check: ## Verify isolated crash inputs without executing them
	$(PYTHON) scripts/check_migration_parity_inputs.py --quarantine-only

migration-parity-evidence-check: ## Verify strict aggregate/result interfaces
	$(PYTHON) scripts/validate_migration_parity_contract.py --manifest "$(MANIFEST)"
	@if test -f "$(MIGRATION_PARITY_OUTPUT)"; then \
		$(PYTHON) scripts/validate_migration_parity_result.py parity "$(MIGRATION_PARITY_OUTPUT)"; \
	fi
	@if test -f "$(MIGRATION_COVERAGE_OUTPUT)"; then \
		$(PYTHON) scripts/validate_migration_parity_result.py coverage "$(MIGRATION_COVERAGE_OUTPUT)"; \
	fi
	@if test -f "$(MIGRATION_ALL_BACKENDS_OUTPUT)"; then \
		$(PYTHON) scripts/validate_migration_parity_result.py all_backends "$(MIGRATION_ALL_BACKENDS_OUTPUT)"; \
	fi

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

ci: repo-map-check fmt clippy migration-parity-fixtures-check migration-parity-test-all-backends migration-parity-coverage ## Full CI pipeline
	@echo "=== done ==="

verify: ci fontdone-parity ## Full workspace CI plus FreeType parity
	@echo "=== all verification done ==="

# ── Clean ─────────────────────────────────────────────────────────────────────
.PHONY: clean clean-all

clean: ## Remove build artifacts and caches
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
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
