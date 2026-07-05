# pillow-rs-freetype Makefile
# ===========================
# Standalone entry point for build, test, lint, parity, and benchmark work.

CARGO := cargo
PYTHON := python3
PYCACHE_DIR := target/pycache
REF_BIN ?= /tmp/gen_refs_v4
BENCH_SAMPLES ?= 10
BENCH_PROFILE ?= default

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf "pillow-rs-freetype\n\n"
	@printf "Setup:\n"
	@printf "  make setup            Install optional audit tooling\n"
	@printf "\nBuild:\n"
	@printf "  make build            Build the crate\n"
	@printf "  make doc              Build crate documentation\n"
	@printf "  make doc-test         Run rustdoc examples\n"
	@printf "\nTest:\n"
	@printf "  make test             Run all crate tests\n"
	@printf "  make test-parity      Run the full FreeType parity matrix\n"
	@printf "  make test-ffi         Run the no-runtime-FFI guard\n"
	@printf "  make test-harness     Run harness contract tests\n"
	@printf "  make test-generator   Run generator contract tests\n"
	@printf "  make test-render-mode Run render-mode matrix tests\n"
	@printf "  make test-fixed       Run fixed arithmetic parity tests\n"
	@printf "  make test-interface   Run public interface coverage report\n"
	@printf "  make test-perf        Run benchmark contract tests\n"
	@printf "\nLint:\n"
	@printf "  make fmt              Check rustfmt\n"
	@printf "  make fmt-fix          Apply rustfmt\n"
	@printf "  make clippy           Run clippy with warnings denied\n"
	@printf "  make lint             Run fmt + clippy\n"
	@printf "\nFixtures:\n"
	@printf "  make fixtures         Regenerate every FreeType fixture family\n"
	@printf "  make fixture-ref-bin  Build the C oracle helper at REF_BIN=$(REF_BIN)\n"
	@printf "  make fixture-force-autohint | fixture-native-tt-default | fixture-no-hinting\n"
	@printf "  make fixture-metrics-only | fixture-outline-cbox | fixture-render-mono | fixture-render-lcd\n"
	@printf "  make fixture-render-mode\n"
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
setup:
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
test:
	$(CARGO) test --locked

.PHONY: test-parity
test-parity:
	$(CARGO) test --test coverage_matrix_tests --locked -- --nocapture

.PHONY: test-ffi
test-ffi:
	$(CARGO) test --test no_runtime_ffi --locked -- --nocapture

.PHONY: test-harness
test-harness:
	$(CARGO) test --test harness_contract --locked

.PHONY: test-generator
test-generator:
	$(CARGO) test --test generator_contract --locked

.PHONY: test-render-mode
test-render-mode:
	$(CARGO) test --test render_mode_matrix --locked

.PHONY: test-fixed
test-fixed:
	$(CARGO) test --test fixed_parity --locked

.PHONY: test-interface
test-interface:
	$(CARGO) test --test interface_coverage --locked -- --nocapture

.PHONY: test-perf
test-perf:
	$(CARGO) test --test perf_benchmark_contract --locked

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

.PHONY: fixture-ref-bin
fixture-ref-bin:
	bash scripts/build_ft.sh
	mkdir -p $(dir $(REF_BIN))
	cc -I freetype/include scripts/gen_ft_refs.c -L freetype/build -lfreetype -lm -o $(REF_BIN)

.PHONY: fixture-force-autohint
fixture-force-autohint: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family force_autohint --ref-bin $(REF_BIN)

.PHONY: fixture-native-tt-default
fixture-native-tt-default: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family native_tt_default --ref-bin $(REF_BIN)

.PHONY: fixture-no-hinting
fixture-no-hinting: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family no_hinting --ref-bin $(REF_BIN)

.PHONY: fixture-metrics-only
fixture-metrics-only: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family metrics_only --ref-bin $(REF_BIN)

.PHONY: fixture-outline-cbox
fixture-outline-cbox: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family outline_cbox --ref-bin $(REF_BIN)

.PHONY: fixture-render-mono
fixture-render-mono: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family render_mono --ref-bin $(REF_BIN)

.PHONY: fixture-render-lcd
fixture-render-lcd: fixture-ref-bin
	$(PYTHON) scripts/build_ft_fixture.py --family render_lcd --ref-bin $(REF_BIN)

.PHONY: fixture-render-mode
fixture-render-mode: fixture-ref-bin
	FT_REF_BIN=$(REF_BIN) $(PYTHON) scripts/build_render_mode_fixture.py

.PHONY: fixtures
fixtures: fixture-force-autohint fixture-native-tt-default fixture-no-hinting
fixtures: fixture-metrics-only fixture-outline-cbox fixture-render-mono
fixtures: fixture-render-lcd fixture-render-mode

.PHONY: bench
bench:
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples $(BENCH_SAMPLES) --profile $(BENCH_PROFILE) --table

.PHONY: bench-quick
bench-quick:
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
ci: fmt clippy doc doc-test test bench-self-test

.PHONY: clean
clean:
	rm -rf target/freetype-bench
	$(CARGO) clean
