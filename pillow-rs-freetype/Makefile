# fontdone Makefile
# ===========================
# Standalone entry point for build, test, lint, parity, and benchmark work.

CARGO := cargo
PYTHON := python3
PYCACHE_DIR := target/pycache
BENCH_SAMPLES ?= 10
BENCH_PROFILE ?= default
PARITY_ARGS ?= -- --nocapture

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf "fontdone\n\n"
	@printf "Setup:\n"
	@printf "  make setup            Fetch oracle source and regenerate fixtures\n"
	@printf "  make setup-tools      Install optional audit tooling\n"
	@printf "  make oracle-fetch     Fetch pinned FreeType C oracle source\n"
	@printf "\nBuild:\n"
	@printf "  make build            Build the crate\n"
	@printf "  make doc              Build crate documentation\n"
	@printf "  make doc-test         Run rustdoc examples\n"
	@printf "\nTest:\n"
	@printf "  make test             Run manifest-driven unified public API parity\n"
	@printf "  make test-parity      Run manifest-driven unified public API parity\n"
	@printf "  make test-unified-fixtures Run manifest-linked input fixtures against runtime C oracle\n"
	@printf "  make test-unified-fixtures-release Run unified fixture parity with Cargo --release\n"
	@printf "  make test-unified-coverage Alias for the unified parity test\n"
	@printf "  make test-api-abi     Run unified API/ABI fixture and coverage gates\n"
	@printf "  make test-ffi         Run the no-runtime-FFI guard for core\n"
	@printf "  make test-ffi-compat  Run API/ABI coverage and thin-wrapper checks\n"
	@printf "  make api-abi-audit    Generate C/Servo/fontdone API and ABI surface audit\n"
	@printf "  make api-abi-check    Check manifest/input coverage and thin FFI ABI surfaces\n"
	@printf "\nLint:\n"
	@printf "  make fmt              Check rustfmt\n"
	@printf "  make fmt-fix          Apply rustfmt\n"
	@printf "  make clippy           Run clippy with warnings denied\n"
	@printf "  make lint             Run fmt + clippy\n"
	@printf "\nFixtures:\n"
	@printf "  make unified-inputs   Regenerate unified FFI input cases\n"
	@printf "  make unified-inputs-check Check generated unified FFI input cases are current\n"
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
setup: unified-oracle unified-inputs public-constants

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

.PHONY: unified-inputs
unified-inputs:
	$(PYTHON) scripts/build_unified_fixture_inputs.py

.PHONY: unified-inputs-check
unified-inputs-check:
	$(PYTHON) scripts/build_unified_fixture_inputs.py --check

.PHONY: test-unified-fixtures
test-unified-fixtures: unified-oracle unified-inputs-check api-abi-check
	$(CARGO) test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-unified-fixtures-release
test-unified-fixtures-release: unified-oracle unified-inputs-check api-abi-check
	$(CARGO) test --release --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture

.PHONY: test-unified-coverage
test-unified-coverage: test-unified-fixtures

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

.PHONY: api-abi-audit
api-abi-audit: oracle-fetch
	$(PYTHON) scripts/audit_api_abi.py

.PHONY: api-abi-check
api-abi-check: api-abi-audit
	$(PYTHON) scripts/check_public_api_inputs.py --audit-json target/api-abi-audit/api_abi_audit.json

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
