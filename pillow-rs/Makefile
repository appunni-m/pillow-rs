# pillow-rs Makefile
# ================
# Standalone entry point for build, test, lint, fixture, and parity work.
# Core build and lint workflows use Rust/Cargo. The canonical parity campaign
# delegates to the workspace root, which owns the Python and JS/WASM runners.

CARGO := cargo
PYTHON := python3
MANIFEST := tests/fixtures/manifest.yaml
FIXTURES_DIR := tests/fixtures

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf "pillow-rs\n\n"
	@printf "Build:\n"
	@printf "  make build            Build the crate\n"
	@printf "  make build-release    Build with optimizations\n"
	@printf "\nTest:\n"
	@printf "  make test             Run the public migration-parity campaign\n"
	@printf "  make backend-support-matrix Emit registry-derived backend support JSON\n"
	@printf "  make migration-parity-test Run canonical live-oracle migration parity\n"
	@printf "  make migration-parity-coverage Run target coverage from indexed plans\n"
	@printf "  make migration-parity-benchmark Run correctness-gated benchmarks\n"
	@printf "\nFixtures:\n"
	@printf "  make fixtures         Delegate to project migration-parity inputs\n"
	@printf "  make fixtures-check   Validate Font public-api raw input fixtures\n"
	@printf "  make fixtures-clean   Refuse to mutate active input specifications\n"
	@printf "\nLint:\n"
	@printf "  make fmt              Check rustfmt\n"
	@printf "  make fmt-fix          Apply rustfmt\n"
	@printf "  make clippy           Run clippy with warnings denied\n"
	@printf "  make lint             Run fmt + clippy\n"
	@printf "\nBenchmark:\n"
	@printf "  make bench            Run pillow-rs operation benchmarks\n"
	@printf "\nCI:\n"
	@printf "  make ci               Run the required local CI sequence\n"
	@printf "\nClean:\n"
	@printf "  make clean            Remove local build artifacts\n"

.PHONY: build
build:
	$(CARGO) build --locked

.PHONY: build-release
build-release:
	$(CARGO) build --release --locked

.PHONY: test
test:
	$(MAKE) -C .. migration-parity-test-all-backends

.PHONY: backend-support-matrix
backend-support-matrix:
	$(CARGO) run --quiet --all-features --example backend_support_matrix --locked

.PHONY: migration-parity-test
migration-parity-test:
	$(MAKE) -C .. migration-parity-test

.PHONY: migration-parity-coverage
migration-parity-coverage:
	$(MAKE) -C .. migration-parity-coverage

.PHONY: migration-parity-benchmark
migration-parity-benchmark:
	$(MAKE) -C .. migration-parity-benchmark

.PHONY: fixtures
fixtures:
	$(MAKE) -C .. migration-parity-fixtures

.PHONY: fixtures-check
fixtures-check:
	$(MAKE) -C .. migration-parity-inputs-check

.PHONY: fixtures-clean
fixtures-clean:
	@printf "Active migration-parity inputs are immutable specifications; no outputs are cleaned.\n"

.PHONY: public-api-boundary
public-api-boundary:
	python3 ../scripts/check_public_api_boundary.py

.PHONY: fmt
fmt: public-api-boundary
	$(CARGO) fmt -- --check

.PHONY: fmt-fix
fmt-fix:
	$(CARGO) fmt
	python3 ../scripts/check_public_api_boundary.py

.PHONY: clippy
clippy: public-api-boundary
	$(CARGO) clippy --all-targets --all-features --locked -- -D warnings

.PHONY: lint
lint: fmt clippy

.PHONY: bench
bench:
	$(CARGO) bench

.PHONY: ci
ci: lint test

.PHONY: clean
clean:
	$(CARGO) clean
