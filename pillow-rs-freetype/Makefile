# fontdone Makefile
# ===========================
# Standalone entry point for build, test, lint, parity, and benchmark work.

CARGO := cargo
PYTHON := python3
PYCACHE_DIR := target/pycache
BENCH_SAMPLES ?= 10
BENCH_PROFILE ?= default
PARITY_ARGS ?= -- --nocapture
COVERAGE_OUTPUT ?= target/coverage/unified-summary.json
CONDITION_COVERAGE_OUTPUT ?= target/coverage/unified-condition-summary.json
CONDITION_COVERAGE_LINES_OUTPUT ?= target/coverage/unified-condition-missing-lines.txt
CONDITION_COVERAGE_NORMALIZED_OUTPUT ?= target/coverage/unified-condition-summary-normalized.json
SCALER_CONDITION_COVERAGE_OUTPUT ?= target/coverage/scaler-unified-condition-summary.json
SCALER_CONDITION_COVERAGE_LINES_OUTPUT ?= target/coverage/scaler-unified-condition-missing-lines.txt
SCALER_CONDITION_COVERAGE_MCP_OUTPUT ?= target/coverage/scaler-unified-condition-summary-mcp.json
CORE_COVERAGE_IGNORE_REGEX := /(ffi-c|ffi-wasm)/src/

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf "fontdone\n\n"
	@printf "Setup:\n"
	@printf "  make setup            Fetch oracle source and regenerate fixtures\n"
	@printf "  make setup-tools      Install optional audit tooling\n"
	@printf "  make oracle-fetch     Fetch pinned FreeType C oracle source\n"
	@printf "  make font-fixture-autohint-scripts Rebuild compact autohint script fixture\n"
	@printf "  make font-fixture-cff Rebuild compact CFF/OpenType fixtures\n"
	@printf "  make font-fixture-cmap Rebuild compact cmap table fixtures\n"
	@printf "  make font-fixture-color Rebuild compact COLR/CPAL fixtures\n"
	@printf "  make font-fixture-cjk Rebuild the compact multiscript topology fixture\n"
	@printf "  make font-fixture-fvar Rebuild compact fvar table fixtures\n"
	@printf "  make font-fixture-gasp Rebuild compact gasp table fixtures\n"
	@printf "  make font-fixture-gzip Rebuild compact gzip/zlib payload fixtures\n"
	@printf "  make font-fixture-hinter Rebuild the compact TrueType program fixture\n"
	@printf "  make font-fixture-metrics Rebuild compact face metric fallback fixtures\n"
	@printf "  make font-fixture-mvar Rebuild compact MVAR variation metrics fixtures\n"
	@printf "  make font-fixture-name Rebuild compact name table fixtures\n"
	@printf "  make font-fixture-post Rebuild compact post table fixtures\n"
	@printf "  make font-fixture-render Rebuild compact render path fixtures\n"
	@printf "  make font-fixture-sbit Rebuild compact embedded bitmap/SBIT fixtures\n"
	@printf "  make font-fixture-sfnt Rebuild compact SFNT table fixtures\n"
	@printf "  make font-fixture-type1 Rebuild compact Type 1 fixtures\n"
	@printf "  make font-fixture-winfnt Rebuild compact Windows FNT fixtures\n"
	@printf "\nBuild:\n"
	@printf "  make build            Build the crate\n"
	@printf "  make doc              Build crate documentation\n"
	@printf "  make doc-test         Run rustdoc examples\n"
	@printf "\nTest:\n"
	@printf "  make test             Run full explicit unified public API parity\n"
	@printf "  make test-parity      Alias for make test\n"
	@printf "  make test-unified-fixtures Run full parity test (same as make test)\n"
	@printf "  make test-unified-fixtures-release Full parity test with Cargo --release\n"
	@printf "  make test-unified-coverage Run parity and write core Rust llvm-cov JSON\n"
	@printf "  make test-unified-condition-coverage Run parity with nightly branch/condition coverage and missing lines\n"
	@printf "  make normalize-unified-condition-coverage Cap LLVM segment hits for Coverage MCP ingestion\n"
	@printf "  make test-api-abi     Run unified API/ABI fixture and manifest gates\n"
	@printf "  make test-ffi         Run the no-runtime-FFI guard for core (<1s)\n"
	@printf "  make test-ffi-compat  Run API/ABI manifest and thin-wrapper checks\n"
	@printf "  make api-abi-audit    Generate C/Servo/fontdone API and ABI surface audit\n"
	@printf "  make api-abi-check    Check manifest/input routing and thin FFI ABI surfaces\n"
	@printf "  make route-audit      Classify public input rows by real, fallback, pending, or unsupported route\n"
	@printf "  make route-buckets    Group pending routes by operation/input hash for batch planning\n"
	@printf "  make real-parity-verify Run parity, route audit, FFI gates, fmt, and clippy\n"
	@printf "  make scaler-parity-focused Run the exact public FT_Load_Glyph vertical-flags bucket\n"
	@printf "  make scaler-condition-coverage Run full condition coverage into scaler-owned artifacts\n"
	@printf "  make scaler-condition-normalize Normalize the scaler condition report for Coverage MCP\n"
	@printf "  make scaler-route-lint-ffi-gates Run route audit, lint, and both FFI gates\n"
	@printf "\nSingle Feature Testing (<1 min per operation, use for debugging):\n"
	@printf "  make test-op OP=<op>      Run parity filtered to one operation (always refreshes oracle)\n"
	@printf "  make test-case CASE=<s>   Run parity filtered to case/subject substring (always refreshes oracle)\n"
	@printf "  make test-pending-case CASE=<exact-case-id> Promote one pending route into runtime comparison (diagnostic, may fail)\n"
	@printf "  make test-list            Print selection summary without executing (diagnostic)\n"
	@printf "  make test-list-ops        Print operation counts without executing\n"
	@printf "  See doc/TESTING_SINGLE_FEATURE.md for env var reference and cache behavior\n"
	@printf "\nLint:\n"
	@printf "  make fmt              Check rustfmt\n"
	@printf "  make fmt-fix          Apply rustfmt\n"
	@printf "  make clippy           Run clippy with warnings denied\n"
	@printf "  make lint             Run fmt + clippy\n"
	@printf "\nFixtures:\n"
	@printf "  make unified-oracle   Build the runtime C FreeType oracle helper\n"
	@printf "  make public-constants Regenerate public constant parity tables\n"
	@printf "\nBenchmark:\n"
	@printf "  make bench            Run Rust vs C FreeType benchmark report\n"
	@printf "  make bench-quick      Run short benchmark smoke comparison\n"
	@printf "  make bench-self-test  Run benchmark summarizer self-test\n"
	@printf "\nCI:\n"
	@printf "  make ci               Run the required local CI sequence\n"
	@printf "  make supply-chain     Run cargo-deny and cargo-audit\n"
	@printf "\nRelease:\n"
	@printf "  make release-dry-run  Validate crate publishing package\n"
	@printf "  make release          Publish crate to crates.io\n"
	@printf "\nClean:\n"
	@printf "  make clean            Remove local build/report artifacts\n"

.PHONY: setup
setup: unified-oracle public-constants

.PHONY: setup-tools
setup-tools:
	$(CARGO) install cargo-deny cargo-audit --locked

.PHONY: build
build:
	$(CARGO) build --locked

.PHONY: doc
doc:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --no-deps --locked

.PHONY: doc-test
doc-test:
	$(CARGO) test --doc --locked

.PHONY: test
test: test-unified-fixtures test-ffi

.PHONY: test-parity
test-parity: test-unified-fixtures test-ffi

.PHONY: real-parity-verify
real-parity-verify: test-unified-fixtures route-audit test-ffi test-ffi-compat lint

.PHONY: scaler-parity-focused
scaler-parity-focused:
	$(MAKE) test-case CASE=freetype.FT_Load_Glyph.no_scale_no_hinting_and_vertical_flags
	$(MAKE) test-case CASE=freetype.FT_Load_Glyph.matrix_load.exec_code_range_and_composite

.PHONY: scaler-condition-coverage
scaler-condition-coverage: CONDITION_COVERAGE_OUTPUT=$(SCALER_CONDITION_COVERAGE_OUTPUT)
scaler-condition-coverage: CONDITION_COVERAGE_LINES_OUTPUT=$(SCALER_CONDITION_COVERAGE_LINES_OUTPUT)
scaler-condition-coverage: test-unified-condition-coverage

.PHONY: scaler-condition-normalize
scaler-condition-normalize:
	jq 'walk(if type == "number" and . > 2147483647 then 2147483647 else . end)' \
		$(SCALER_CONDITION_COVERAGE_OUTPUT) > $(SCALER_CONDITION_COVERAGE_MCP_OUTPUT)

.PHONY: scaler-route-lint-ffi-gates
scaler-route-lint-ffi-gates: route-audit lint test-ffi test-ffi-compat

.PHONY: unified-oracle
unified-oracle: oracle-fetch
	bash scripts/build_ft.sh
	$(PYTHON) scripts/build_unified_oracle.py

.PHONY: public-constants
public-constants: oracle-fetch
	$(PYTHON) scripts/generate_public_constants.py --rust-src src/ffi/generated_constants.rs --test-lookup tests/support/generated_constant_lookup.rs

# ── Full Explicit Parity Test ────────────────────────────────────────
#
.PHONY: test-unified-fixtures
test-unified-fixtures: unified-oracle api-abi-check
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

# ── Full Parity Test with --release (~15 min total) ───────────────────
# --release mode is faster but misses debug_assertions.
#
.PHONY: test-unified-fixtures-release
test-unified-fixtures-release: unified-oracle api-abi-check
	$(CARGO) test --release --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-unified-coverage
test-unified-coverage: unified-oracle api-abi-check
	mkdir -p $(dir $(COVERAGE_OUTPUT))
	$(CARGO) llvm-cov --test unified_fixture_parity --locked --json \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		--output-path $(COVERAGE_OUTPUT) -- unified_fixture_parity --nocapture

.PHONY: test-unified-condition-coverage
test-unified-condition-coverage: unified-oracle api-abi-check
	mkdir -p $(dir $(CONDITION_COVERAGE_OUTPUT))
	FONTDONE_ENABLE_SILENT_TRACE_LOGGER=1 \
	RUSTFLAGS="-Zcoverage-options=condition" $(CARGO) +nightly llvm-cov \
		--test unified_fixture_parity --locked --json \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		--output-path $(CONDITION_COVERAGE_OUTPUT) -- unified_fixture_parity --nocapture
	RUSTFLAGS="-Zcoverage-options=condition" $(CARGO) +nightly llvm-cov report \
		--show-missing-lines \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		> $(CONDITION_COVERAGE_LINES_OUTPUT)

.PHONY: normalize-unified-condition-coverage
normalize-unified-condition-coverage:
	mkdir -p $(dir $(CONDITION_COVERAGE_NORMALIZED_OUTPUT))
	jq -c '(.data[]?.files[]?.segments[]? | select(length >= 3) | .[2]) |= if . > 2147483647 then 2147483647 else . end' \
		$(CONDITION_COVERAGE_OUTPUT) > $(CONDITION_COVERAGE_NORMALIZED_OUTPUT).tmp
	mv $(CONDITION_COVERAGE_NORMALIZED_OUTPUT).tmp $(CONDITION_COVERAGE_NORMALIZED_OUTPUT)

.PHONY: test-api-abi
test-api-abi: test-unified-fixtures test-ffi

.PHONY: test-ffi
test-ffi:
# Core rendering code must not pull in native FreeType, bindgen, or system
# FFI libraries.  extern "C" in src/ffi/ is expected (thin facade types).
	@! grep -En 'freetype-sys|^bindgen |^cc |dlopen|libloading|pkg-config' Cargo.toml
	@for f in $$(find src -name '*.rs' ! -path 'src/ffi/*'); do \
		if grep -Eqn 'freetype-sys|bindgen|dlopen|libloading|pkg-config' "$$f"; then \
			echo "$$f: forbidden FFI dependency" >&2; exit 1; \
		fi; \
	done
	@echo "no-runtime-FFI guard: clean"

.PHONY: test-ffi-compat
test-ffi-compat: api-abi-check

# ── Single Feature Testing ────────────────────────────────────────────
# These targets run the unified parity test filtered to a single operation
# or case. They always refresh the oracle cache (FONTDONE_UNIFIED_ORACLE_REFRESH=1)
# to avoid cache-key confusion when switching between filters.
#
# Usage:
#   make test-op OP=ftadvanc.get_advance
#   make test-case CASE=load_glyph
#   make test-list
#   make test-list-ops
#
# The full-suite cache is never affected — different filter selections produce
# different cache keys, so filtered runs and full-suite runs use separate cache
# files under tests/fixtures/outputs/unified_oracle_cache/.
#
# Env vars (all optional):
#   FONTDONE_UNIFIED_OPERATION_FILTER  – substring match on operation name
#   FONTDONE_UNIFIED_CASE_FILTER       – substring match on case_id/subject/case
#   FONTDONE_UNIFIED_CASE_LIMIT        – max number of selected concrete cases
#   FONTDONE_UNIFIED_ORACLE_REFRESH    – force skip cache, re-run C oracle
#   FONTDONE_UNIFIED_SELECTION_ONLY    – print selection, don't execute
#   FONTDONE_UNIFIED_PROFILE           – print timing profiles

.PHONY: test-op
test-op: unified-oracle api-abi-check
	@if [ -z "$(OP)" ]; then \
		echo "Usage: make test-op OP=<operation>" >&2; \
		echo "  Example operations: ftadvanc.get_advance, load_glyph, render_glyph, freetype.inspect_glyph_metrics" >&2; \
		echo "  For diagnostics: make test-list-ops" >&2; \
		exit 1; \
	fi
	FONTDONE_UNIFIED_OPERATION_FILTER="$(OP)" \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-case
test-case: unified-oracle api-abi-check
	@if [ -z "$(CASE)" ]; then \
		echo "Usage: make test-case CASE=<substring>" >&2; \
		echo "  Matches against case_id, subject, or case fields" >&2; \
		exit 1; \
	fi
	FONTDONE_UNIFIED_CASE_FILTER="$(CASE)" \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-pending-case
test-pending-case: unified-oracle api-abi-check
	@if [ -z "$(CASE)" ]; then \
		echo "Usage: make test-pending-case CASE=<exact-case-id>" >&2; \
		exit 2; \
	fi
	FONTDONE_UNIFIED_CASE_FILTER="$(CASE)" \
	FONTDONE_UNIFIED_INCLUDE_PENDING_CASE="$(CASE)" \
	FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-filter-guard
test-filter-guard: unified-oracle api-abi-check
	@output="$$(mktemp)"; \
	trap 'rm -f "$$output"' EXIT; \
	if FONTDONE_UNIFIED_CASE_FILTER="__fontdone_no_matching_fixture__" \
		FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
		$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture \
		>"$$output" 2>&1; then \
		cat "$$output"; \
		echo "expected an unmatched explicit fixture filter to fail" >&2; \
		exit 1; \
	fi; \
	if ! grep -Fq "explicit runtime filter matched no fixture cases" "$$output"; then \
		cat "$$output"; \
		echo "unmatched fixture filter failed for an unexpected reason" >&2; \
		exit 1; \
	fi

.PHONY: test-error-comparison-guard
test-error-comparison-guard:
	$(CARGO) test --test unified_fixture_parity --locked \
		unified_fixture_parity_exact_error_guard_rejects_non_error_results -- --nocapture

.PHONY: test-list
test-list: unified-oracle api-abi-check
	FONTDONE_UNIFIED_SELECTION_ONLY=1 \
	FONTDONE_UNIFIED_CASE_LIMIT=5 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-list-ops
test-list-ops: unified-oracle api-abi-check
	FONTDONE_UNIFIED_SELECTION_ONLY=1 \
	FONTDONE_UNIFIED_CASE_LIMIT=0 \
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: api-abi-audit
api-abi-audit: oracle-fetch
	$(PYTHON) scripts/audit_api_abi.py

.PHONY: api-abi-check
api-abi-check: api-abi-audit
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json --route-audit

.PHONY: route-audit
route-audit: api-abi-check

.PHONY: route-buckets
route-buckets: route-audit
	$(PYTHON) scripts/report_pending_route_buckets.py

.PHONY: font-fixture-hinter
font-fixture-hinter:
	$(PYTHON) -m fontTools.ttx -q --no-recalc-timestamp -o tests/fixtures/fonts/glyf/hinter-control-matrix.ttf \
		tests/fixtures/font-sources/hinter-control-matrix.ttx
	$(PYTHON) scripts/build_hinter_edge_fixtures.py

.PHONY: font-fixture-render
font-fixture-render:
	$(PYTHON) scripts/build_render_fixtures.py

.PHONY: font-fixture-cjk
font-fixture-cjk:
	$(PYTHON) -m fontTools.ttx -q --no-recalc-timestamp -o tests/fixtures/fonts/autohint/cjk-coverage.ttf \
		tests/fixtures/font-sources/cjk-coverage.ttx
	$(PYTHON) -m fontTools.ttx -q --no-recalc-timestamp -o tests/fixtures/fonts/autohint/cjk-width-order.ttf \
		tests/fixtures/font-sources/cjk-width-order.ttx

.PHONY: font-fixture-autohint-scripts
font-fixture-autohint-scripts:
	$(PYTHON) scripts/build_autohint_script_fixtures.py

.PHONY: font-fixture-cff
font-fixture-cff:
	$(PYTHON) scripts/build_cff_fixtures.py

.PHONY: font-fixture-type1
font-fixture-type1:
	$(PYTHON) scripts/build_type1_fixtures.py

.PHONY: font-fixture-winfnt
font-fixture-winfnt:
	$(PYTHON) scripts/generate_winfnt_fixtures.py

.PHONY: font-fixture-gasp
font-fixture-gasp: font-fixture-hinter
	$(PYTHON) scripts/build_gasp_fixtures.py

.PHONY: font-fixture-gzip
font-fixture-gzip:
	$(PYTHON) scripts/build_gzip_fixtures.py

.PHONY: font-fixture-metrics
font-fixture-metrics: font-fixture-hinter
	$(PYTHON) scripts/build_metric_fixtures.py

.PHONY: font-fixture-mvar
font-fixture-mvar:
	$(PYTHON) scripts/build_mvar_fixtures.py

.PHONY: font-fixture-cmap
font-fixture-cmap: font-fixture-hinter
	$(PYTHON) scripts/build_cmap_fixtures.py

.PHONY: font-fixture-color
font-fixture-color:
	$(PYTHON) scripts/build_cpal_palette_fixtures.py

.PHONY: font-fixture-post
font-fixture-post: font-fixture-hinter
	$(PYTHON) scripts/build_post_fixtures.py

.PHONY: font-fixture-fvar
font-fixture-fvar:
	$(PYTHON) scripts/build_fvar_fixtures.py

.PHONY: font-fixture-name
font-fixture-name:
	$(PYTHON) scripts/build_name_fixtures.py

.PHONY: font-fixture-sfnt
font-fixture-sfnt: font-fixture-hinter
	$(PYTHON) scripts/build_sfnt_fixtures.py

.PHONY: font-fixture-sbit
font-fixture-sbit: font-fixture-hinter
	$(PYTHON) scripts/build_sbit_fixtures.py

.PHONY: fmt
fmt:
	$(CARGO) fmt -- --check

.PHONY: fmt-fix
fmt-fix:
	$(CARGO) fmt

.PHONY: clippy
clippy:
	$(CARGO) clippy --all-targets --all-features --locked -- -D warnings

.PHONY: lint
lint: fmt clippy

.PHONY: oracle-fetch
oracle-fetch:
	bash scripts/fetch_ft.sh

.PHONY: bench
bench: unified-oracle
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples $(BENCH_SAMPLES) --profile $(BENCH_PROFILE) --table

.PHONY: bench-quick
bench-quick: unified-oracle
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples 2 --profile $(BENCH_PROFILE) --table

.PHONY: bench-self-test
bench-self-test:
	PYTHONPYCACHEPREFIX=$(PYCACHE_DIR) $(PYTHON) -m py_compile scripts/bench_freetype.py
	$(PYTHON) scripts/bench_freetype.py --self-test

.PHONY: supply-chain
supply-chain:
	$(CARGO) deny check advisories
	$(CARGO) deny check bans
	$(CARGO) deny check licenses
	$(CARGO) deny check sources
	$(CARGO) audit

.PHONY: release-dry-run
release-dry-run:
	$(CARGO) publish --dry-run --locked

.PHONY: release
release:
	$(CARGO) publish --locked

.PHONY: ci
ci: setup fmt clippy doc doc-test test bench-self-test

.PHONY: clean
clean:
	rm -rf target/fontdone-bench
	rm -rf tests/fixtures/outputs
	$(CARGO) clean
