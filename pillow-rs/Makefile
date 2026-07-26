# pillow-rs Makefile
# ================
# Standalone entry point for build, test, lint, fixture, and parity work.
# All workflows delegate to Rust/Cargo — no Python or JS dependencies here.
# Use the workspace root Makefile for Python/WASM test runners.

CARGO := cargo
PYTHON := python3
FONT_ORACLE_PYTHON ?= $(abspath ../.oracle-venv/bin/python)
IMAGINGFT_ORACLE_PYTHON ?= $(FONT_ORACLE_PYTHON)
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
	@printf "  make test-eval-oracle Run shared Pillow Image.eval oracle through Rust\n"
	@printf "  make test-image-open-oracle Run shared Pillow Image.open oracle through Rust\n"
	@printf "  make test-apply-transparency-oracle Run Pillow Image.apply_transparency oracle through Rust\n"
	@printf "  make test-tobytes-oracle Run shared Pillow Image.tobytes oracle through Rust\n"
	@printf "  make test-imagefont-getmask2-oracle Run shared Pillow ImageFont.getmask2 oracle through Rust\n"
	@printf "  make test-transposed-font-oracle Run shared Pillow TransposedFont oracle through Rust\n"
	@printf "  make test-fromarray-descriptor-oracle Run shared Pillow fromarray descriptor oracle through Rust\n"
	@printf "  make backend-support-matrix Emit registry-derived backend support JSON\n"
	@printf "  make font-tests        Run Font public API parity tests\n"
	@printf "  make font-tests-release Run Font public API parity tests (release)\n"
	@printf "  make font-tests-coverage Run Font public API parity tests with coverage\n"
	@printf "  make imagingft-tests   Compatibility alias for font-tests\n"
	@printf "  make test-imagingft           Run legacy ImagingFT matrix parity tests\n"
	@printf "  make test-imagingft-release   Run legacy ImagingFT matrix parity tests (release)\n"
	@printf "\nFixtures:\n"
	@printf "  make fixtures         Explain Font fixture generation policy\n"
	@printf "  make fixtures-check   Validate Font public-api raw input fixtures\n"
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
test: test-core font-tests

.PHONY: test-core
test-core:
	$(CARGO) test --lib --locked

.PHONY: test-backend-parity
test-backend-parity:
	$(CARGO) test --all-features --test backend_parity --locked -- --nocapture

.PHONY: test-paste-oracle
test-paste-oracle:
	$(CARGO) test --all-features --test backend_parity --locked paste_is_exact_on_portable_cpu_backend -- --exact --nocapture
	$(CARGO) test --all-features --test backend_parity --locked paste_validation_matches_exact_pillow_errors -- --exact --nocapture

.PHONY: test-drawing-oracle
test-drawing-oracle:
	$(CARGO) test --all-features --test backend_parity --locked drawing_is_exact_and_backend_capabilities_are_truthful -- --exact --nocapture

.PHONY: test-imagefont-getmask2-oracle
test-imagefont-getmask2-oracle:
	$(CARGO) test --all-features --test imagefont_getmask2_oracle --locked -- --nocapture

.PHONY: test-transposed-font-oracle
test-transposed-font-oracle:
	$(CARGO) test --all-features --test transposed_font_oracle --locked -- --nocapture

.PHONY: test-fromarray-descriptor-oracle
test-fromarray-descriptor-oracle:
	$(CARGO) test --all-features --test fromarray_descriptor_oracle --locked -- --nocapture

.PHONY: test-color3dlut-oracle
test-color3dlut-oracle:
	$(CARGO) test --all-features --test color3dlut_oracle --locked -- --nocapture

.PHONY: test-eval-oracle
test-eval-oracle:
	$(CARGO) test --all-features --test eval_oracle --locked -- --nocapture

.PHONY: test-image-open-oracle
test-image-open-oracle:
	$(CARGO) test --all-features --test image_open_oracle --locked -- --nocapture

.PHONY: test-tobytes-oracle
test-tobytes-oracle:
	$(CARGO) test --all-features --test tobytes_oracle --locked -- --nocapture

.PHONY: test-apply-transparency-oracle
test-apply-transparency-oracle:
	$(CARGO) test --all-features --test apply_transparency_oracle --locked -- --nocapture

.PHONY: backend-support-matrix
backend-support-matrix:
	$(CARGO) run --quiet --all-features --example backend_support_matrix --locked

.PHONY: font-tests
font-tests:
	FONT_ORACLE_PYTHON=$(FONT_ORACLE_PYTHON) \
		$(CARGO) test --all-features --test font_public_api --locked -- --nocapture

.PHONY: font-tests-release
font-tests-release:
	FONT_ORACLE_PYTHON=$(FONT_ORACLE_PYTHON) \
		$(CARGO) test --release --all-features --test font_public_api --locked -- --nocapture

.PHONY: font-tests-coverage
font-tests-coverage:
	bash ../scripts/coverage/run_font_rust_coverage.sh

.PHONY: imagingft-tests
imagingft-tests: font-tests

.PHONY: imagingft-tests-release
imagingft-tests-release: font-tests-release

.PHONY: imagingft-tests-coverage
imagingft-tests-coverage: font-tests-coverage

.PHONY: test-imagingft
test-imagingft: font-tests

.PHONY: test-imagingft-release
test-imagingft-release: font-tests-release

.PHONY: fixtures
fixtures:
	@printf "Font public-api fixtures are input-only and are not generated.\n"

.PHONY: fixtures-check
fixtures-check:
	@! rg -n '"(error|expect_error|expectation|expected|hash|oracle|output|outputs|pixels_hex|raw_path|sha256|status)"' \
		tests/fixtures/font/inputs/public-api
	@! rg -n '^[[:space:]]*(error|expect_error|expectation|expected|hash|output|outputs|pixels_hex|raw_path|sha256|status):' \
		tests/fixtures/font/font_manifest.yaml

.PHONY: fixtures-clean
fixtures-clean:
	rm -rf $(FIXTURES_DIR)/outputs/
	mkdir -p $(FIXTURES_DIR)/outputs/{jsons,images,raws}

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
