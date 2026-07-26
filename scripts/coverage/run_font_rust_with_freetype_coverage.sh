#!/usr/bin/env bash
# Run the Font public API parity corpus through the Rust API with coverage,
# including the pure-Rust FreeType-compatible dependency used by Font.

set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/../.." && pwd)"
coverage_dir="$repo_dir/target/coverage"
coverage_output="$coverage_dir/font-rust-with-freetype.json"
coverage_target_dir="$coverage_dir/font-rust-with-freetype-target"

mkdir -p "$coverage_dir"
cd "$repo_dir"

export CARGO_TARGET_DIR="$coverage_target_dir"
export RUSTUP_TOOLCHAIN=nightly
cargo +nightly llvm-cov clean --workspace
eval "$(cargo +nightly llvm-cov --branch show-env --sh)"

set +e
make -C pillow-rs font-tests
test_status=$?
set -e

cargo +nightly llvm-cov --branch report \
    --json \
    --ignore-filename-regex='(^|/)(target|\.cargo|rustc)(/|$)' \
    --output-path "$coverage_output"

printf 'Rust font coverage with FreeType dependency: %s\n' "$coverage_output"
exit "$test_status"
