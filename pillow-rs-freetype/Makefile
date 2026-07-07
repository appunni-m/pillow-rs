# fontdone Makefile
# ===========================
# Standalone entry point for build, test, lint, parity, and benchmark work.

CARGO := cargo
PYTHON := python3
PYCACHE_DIR := target/pycache
REF_BIN ?= /tmp/gen_refs_v4
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
	@printf "  make fixtures         Regenerate every FreeType fixture family\n"
	@printf "  make fixture-inventory Regenerate font inventory from tracked fonts\n"
	@printf "  make fixture-ref-bin  Build the C oracle helper at REF_BIN=$(REF_BIN)\n"
	@printf "  make fixture-force-autohint | fixture-native-tt-default | fixture-no-hinting\n"
	@printf "  make fixture-metrics-only | fixture-outline-cbox | fixture-render-mono | fixture-render-lcd\n"
	@printf "  make fixture-render-mode\n"
	@printf "  make unified-inputs   Regenerate unified FFI input cases from matrices\n"
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
	@printf "  make clean-fixtures   Remove generated fixture matrices and raw bytes\n"

.PHONY: setup
setup: fixtures

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
test: test-unified-fixtures

.PHONY: test-parity
test-parity: test-unified-fixtures

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
test-api-abi: test-unified-fixtures

.PHONY: test-ffi
test-ffi:
	@! rg -n 'freetype-sys|bindgen|cc::|extern "C"|dlopen|libloading|pkg-config' src Cargo.toml

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

.PHONY: fixture-ref-bin
fixture-ref-bin: oracle-fetch
	bash scripts/build_ft.sh
	mkdir -p $(dir $(REF_BIN))
	cc -I freetype/include scripts/gen_ft_refs.c -L freetype/build -lfreetype -lm -o $(REF_BIN)

.PHONY: fixture-inventory
fixture-inventory: fixture-ref-bin
	FT_REF_BIN=$(REF_BIN) $(PYTHON) scripts/build_fixtures.py --inventory

.PHONY: fixture-force-autohint
fixture-force-autohint: fixture-ref-bin fixture-inventory
	FT_REF_BIN=$(REF_BIN) $(PYTHON) scripts/build_fixtures.py

.PHONY: fixture-native-tt-default
fixture-native-tt-default: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family native_tt_default --ref-bin $(REF_BIN)

.PHONY: fixture-no-hinting
fixture-no-hinting: fixture-ref-bin fixture-inventory
	$(PYTHON) scripts/build_ft_fixture.py --family no_hinting --ref-bin $(REF_BIN)

.PHONY: fixture-metrics-only
fixture-metrics-only: fixture-ref-bin fixture-inventory
	$(PYTHON) scripts/build_ft_fixture.py --family metrics_only --ref-bin $(REF_BIN)

.PHONY: fixture-outline-cbox
fixture-outline-cbox: fixture-ref-bin fixture-inventory
	$(PYTHON) scripts/build_ft_fixture.py --family outline_cbox --ref-bin $(REF_BIN)

.PHONY: fixture-render-mono
fixture-render-mono: fixture-ref-bin fixture-inventory
	$(PYTHON) scripts/build_ft_fixture.py --family render_mono --ref-bin $(REF_BIN)

.PHONY: fixture-render-lcd
fixture-render-lcd: fixture-ref-bin fixture-inventory
	$(PYTHON) scripts/build_ft_fixture.py --family render_lcd --ref-bin $(REF_BIN)

.PHONY: fixture-render-mode
fixture-render-mode: fixture-ref-bin
	FT_REF_BIN=$(REF_BIN) $(PYTHON) scripts/build_render_mode_fixture.py

.PHONY: fixture-fstype
fixture-fstype:
	$(PYTHON) scripts/build_fstype_fixtures.py

.PHONY: fixture-kerning
fixture-kerning:
	$(PYTHON) scripts/build_kerning_fixtures.py

.PHONY: fixture-face-properties
fixture-face-properties: fixture-kerning
	$(PYTHON) scripts/build_face_property_fixtures.py

.PHONY: fixture-variable
fixture-variable:
	$(PYTHON) scripts/build_variable_fixtures.py

.PHONY: fixture-vertical
fixture-vertical:
	$(PYTHON) scripts/build_vertical_fixtures.py

.PHONY: fixtures
fixtures: fixture-inventory fixture-force-autohint fixture-native-tt-default fixture-no-hinting
fixtures: fixture-metrics-only fixture-outline-cbox fixture-render-mono
fixtures: fixture-render-lcd fixture-render-mode fixture-fstype fixture-kerning fixture-face-properties
fixtures: fixture-variable fixture-vertical

.PHONY: bench
bench: fixture-ref-bin
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples $(BENCH_SAMPLES) --profile $(BENCH_PROFILE) --table

.PHONY: bench-quick
bench-quick: fixture-ref-bin
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
	$(CARGO) clean

.PHONY: clean-fixtures
clean-fixtures:
	rm -f tests/fixtures/*.json
	rm -rf tests/fixtures/outputs
