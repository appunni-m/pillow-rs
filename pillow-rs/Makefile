# pillow-rs Makefile
# ================
# Standalone entry point for build, test, lint, fixture, and parity work.
# All workflows delegate to Rust/Cargo — no Python or JS dependencies here.
# Use the workspace root Makefile for Python/WASM test runners.

CARGO := cargo
PYTHON := python3
MANIFEST := ../manifest.yaml
FIXTURES_DIR := ../tests/fixtures

.DEFAULT_GOAL := help

.PHONY: help
help:
	@printf "pillow-rs\n\n"
	@printf "Build:\n"
	@printf "  make build            Build the crate\n"
	@printf "  make build-release    Build with optimizations\n"
	@printf "\nTest:\n"
	@printf "  make test             Run all Rust tests\n"
	@printf "  make test-core        Run unit tests (src/)\n"
	@printf "  make test-backend-parity Run forced-backend Pillow parity tests\n"
	@printf "  make test-color3dlut-oracle Run shared Pillow Color3DLUT oracle through Rust\n"
	@printf "  make test-imagingft   Run imagingft matrix parity tests\n"
	@printf "  make test-imagingft-release  Run imagingft parity tests (release)\n"
	@printf "\nFixtures:\n"
	@printf "  make fixtures         Regenerate imagingft fixture matrix\n"
	@printf "  make fixtures-check   Verify current fixtures reproduce byte-for-byte\n"
	@printf "  make fixtures-clean   Remove generated fixture outputs\n"
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
test: test-core test-imagingft

.PHONY: test-core
test-core:
	$(CARGO) test --lib --locked

.PHONY: test-backend-parity
test-backend-parity:
	$(CARGO) test --all-features --test backend_parity --locked -- --nocapture

.PHONY: test-color3dlut-oracle
test-color3dlut-oracle:
	$(CARGO) test --all-features --test color3dlut_oracle --locked -- --nocapture

.PHONY: test-imagingft
test-imagingft: fixtures
	$(PYTHON) scripts/build_imagingft_fixtures.py --check
	$(CARGO) test --test imagingft_matrix_tests --locked -- --nocapture

.PHONY: test-imagingft-release
test-imagingft-release: fixtures
	$(PYTHON) scripts/build_imagingft_fixtures.py --check
	$(CARGO) test --release --test imagingft_matrix_tests --locked -- --nocapture

.PHONY: fixtures
fixtures:
	$(PYTHON) scripts/build_imagingft_fixtures.py

.PHONY: fixtures-check
fixtures-check:
	$(PYTHON) scripts/build_imagingft_fixtures.py --check

.PHONY: fixtures-clean
fixtures-clean:
	rm -rf $(FIXTURES_DIR)/outputs/
	mkdir -p $(FIXTURES_DIR)/outputs/{jsons,images,raws}

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
	$(CARGO) bench

.PHONY: ci
ci: lint test

.PHONY: clean
clean:
	$(CARGO) clean
