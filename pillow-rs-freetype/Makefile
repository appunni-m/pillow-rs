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
CORE_COVERAGE_IGNORE_REGEX := /(ffi-c|ffi-wasm)/src/

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf "fontdone\n\n"
	@printf "Setup:\n"
	@printf "  make setup            Fetch oracle source and regenerate fixtures\n"
	@printf "  make setup-tools      Install optional audit tooling\n"
	@printf "  make oracle-fetch     Fetch pinned FreeType C oracle source\n"
	@printf "  make font-fixture-cmap Rebuild compact cmap table fixtures\n"
	@printf "  make font-fixture-cjk Rebuild the compact multiscript topology fixture\n"
	@printf "  make font-fixture-fvar Rebuild compact fvar table fixtures\n"
	@printf "  make font-fixture-gasp Rebuild compact gasp table fixtures\n"
	@printf "  make font-fixture-hinter Rebuild the compact TrueType program fixture\n"
	@printf "  make font-fixture-name Rebuild compact name table fixtures\n"
	@printf "  make font-fixture-post Rebuild compact post table fixtures\n"
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
	@printf "  make test-api-abi     Run unified API/ABI fixture and coverage gates\n"
	@printf "  make test-ffi         Run the no-runtime-FFI guard for core (<1s)\n"
	@printf "  make test-ffi-compat  Run API/ABI coverage and thin-wrapper checks\n"
	@printf "  make api-abi-audit    Generate C/Servo/fontdone API and ABI surface audit\n"
	@printf "  make api-abi-check    Check manifest/input coverage and thin FFI ABI surfaces\n"
	@printf "  make route-audit      Classify public input rows by real, fallback, pending, or unsupported route\n"
	@printf "\nSingle Feature Testing (<1 min per operation, use for debugging):\n"
	@printf "  make test-op OP=<op>      Run parity filtered to one operation (always refreshes oracle)\n"
	@printf "  make test-case CASE=<s>   Run parity filtered to case/subject substring (always refreshes oracle)\n"
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
	RUSTFLAGS="-Zcoverage-options=condition" $(CARGO) +nightly llvm-cov \
		--test unified_fixture_parity --locked --json \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		--output-path $(CONDITION_COVERAGE_OUTPUT) -- unified_fixture_parity --nocapture
	RUSTFLAGS="-Zcoverage-options=condition" $(CARGO) +nightly llvm-cov report \
		--show-missing-lines \
		--ignore-filename-regex '$(CORE_COVERAGE_IGNORE_REGEX)' \
		> $(CONDITION_COVERAGE_LINES_OUTPUT)

.PHONY: test-api-abi
test-api-abi: test-unified-fixtures test-ffi

.PHONY: test-ffi
test-ffi:
# Core rendering code must not pull in native FreeType, bindgen, or system
# FFI libraries.  extern "C" in src/ffi/ is expected (thin facade types).
	@! rg -n 'freetype-sys|^bindgen |^cc |dlopen|libloading|pkg-config' Cargo.toml
	@for f in $$(find src -name '*.rs' ! -path 'src/ffi/*'); do \
		if rg -qn 'freetype-sys|bindgen|dlopen|libloading|pkg-config' "$$f"; then \
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

.PHONY: route-audit
route-audit: api-abi-audit
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json --route-audit

.PHONY: font-fixture-hinter
font-fixture-hinter:
	@command -v ttx >/dev/null || { echo "fonttools ttx is required" >&2; exit 1; }
	ttx -q --no-recalc-timestamp -o tests/fixtures/fonts/glyf/hinter-control-matrix.ttf \
		tests/fixtures/font-sources/hinter-control-matrix.ttx

.PHONY: font-fixture-cjk
font-fixture-cjk:
	@command -v ttx >/dev/null || { echo "fonttools ttx is required" >&2; exit 1; }
	ttx -q --no-recalc-timestamp -o tests/fixtures/fonts/autohint/cjk-coverage.ttf \
		tests/fixtures/font-sources/cjk-coverage.ttx

.PHONY: font-fixture-gasp
font-fixture-gasp: font-fixture-hinter
	$(PYTHON) scripts/build_gasp_fixtures.py

.PHONY: font-fixture-cmap
font-fixture-cmap: font-fixture-hinter
	$(PYTHON) scripts/build_cmap_fixtures.py

.PHONY: font-fixture-post
font-fixture-post: font-fixture-hinter
	$(PYTHON) scripts/build_post_fixtures.py

.PHONY: font-fixture-fvar
font-fixture-fvar:
	$(PYTHON) scripts/build_fvar_fixtures.py

.PHONY: font-fixture-name
font-fixture-name:
	$(PYTHON) scripts/build_name_fixtures.py

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
