#!/usr/bin/env bash
# Run the exact shared Pillow ImageFont.getmask2 corpus through Rust with coverage.

set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/../.." && pwd)"
coverage_dir="$repo_dir/target/coverage"
coverage_output="$coverage_dir/pillow-imagefont-getmask2-rust.json"
coverage_target_dir="$coverage_dir/imagefont-getmask2-rust-target"

mkdir -p "$coverage_dir"
cd "$repo_dir"

export CARGO_TARGET_DIR="$coverage_target_dir"
export RUSTUP_TOOLCHAIN=nightly
cargo +nightly llvm-cov clean --workspace
eval "$(cargo +nightly llvm-cov --branch show-env --sh)"

set +e
make -C pillow-rs test-imagefont-getmask2-oracle
test_status=$?
set -e

cargo +nightly llvm-cov --branch report \
    --json \
    --ignore-filename-regex='(^|/)(pillow-rs-freetype/fixtures|target|\.cargo|rustc)(/|$)' \
    --output-path "$coverage_output"

printf 'Rust ImageFont.getmask2 coverage: %s\n' "$coverage_output"
exit "$test_status"
