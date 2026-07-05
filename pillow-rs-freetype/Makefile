# pillow-rs-freetype Makefile
# ===========================
# Standalone entry point for build, test, lint, parity, and benchmark work.

CARGO := cargo
PYTHON := python3
PYCACHE_DIR := target/pycache

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf "pillow-rs-freetype\n\n"
	@printf "Setup:\n"
	@printf "  make setup            Install optional audit tooling\n"
	@printf "\nBuild:\n"
	@printf "  make build            Build the crate\n"
	@printf "  make doc              Build crate documentation\n"
	@printf "\nTest:\n"
	@printf "  make test             Run all crate tests\n"
	@printf "  make test-parity      Run the full FreeType parity matrix\n"
	@printf "  make test-ffi         Run the no-runtime-FFI guard\n"
	@printf "\nLint:\n"
	@printf "  make fmt              Check rustfmt\n"
	@printf "  make fmt-fix          Apply rustfmt\n"
	@printf "  make clippy           Run clippy with warnings denied\n"
	@printf "  make lint             Run fmt + clippy\n"
	@printf "\nBenchmark:\n"
	@printf "  make bench            Run Rust vs C FreeType benchmark report\n"
	@printf "  make bench-quick      Run short benchmark smoke comparison\n"
	@printf "  make bench-self-test  Run benchmark summarizer self-test\n"
	@printf "\nCI:\n"
	@printf "  make ci               Run the required local CI sequence\n"
	@printf "  make supply-chain     Run cargo-deny and cargo-audit\n"
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

.PHONY: test
test:
	$(CARGO) test --locked

.PHONY: test-parity
test-parity:
	$(CARGO) test --test coverage_matrix_tests --locked -- --nocapture

.PHONY: test-ffi
test-ffi:
	$(CARGO) test --test no_runtime_ffi --locked -- --nocapture

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

.PHONY: bench
bench:
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples 10 --profile default --table

.PHONY: bench-quick
bench-quick:
	$(PYTHON) scripts/bench_freetype.py --compare-c --samples 2 --profile default --table

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

.PHONY: ci
ci: fmt clippy test-parity test-ffi bench-self-test

.PHONY: clean
clean:
	rm -rf target/freetype-bench
	$(CARGO) clean
