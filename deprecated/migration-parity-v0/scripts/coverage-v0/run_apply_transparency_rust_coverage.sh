#!/usr/bin/env bash
# Run the exact shared Pillow apply-transparency corpus through the Rust API
# while collecting line, branch, function, and region coverage.

set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/../.." && pwd)"
coverage_dir="$repo_dir/target/coverage"
coverage_output="$coverage_dir/pillow-apply-transparency-rust.json"
coverage_target_dir="$coverage_dir/apply-transparency-rust-target"

mkdir -p "$coverage_dir"
cd "$repo_dir"

export CARGO_TARGET_DIR="$coverage_target_dir"
export RUSTUP_TOOLCHAIN=nightly
cargo +nightly llvm-cov clean --workspace
eval "$(cargo +nightly llvm-cov --branch show-env --sh)"

set +e
make -C pillow-rs test-apply-transparency-oracle
test_status=$?
set -e

cargo +nightly llvm-cov --branch report \
    --json \
    --ignore-filename-regex='(^|/)(pillow-rs-freetype|target|\.cargo|rustc)(/|$)' \
    --output-path "$coverage_output"

printf 'Rust Image.apply_transparency coverage: %s\n' "$coverage_output"
exit "$test_status"
