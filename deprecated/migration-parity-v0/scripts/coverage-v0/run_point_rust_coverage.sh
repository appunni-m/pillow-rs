#!/usr/bin/env bash
# Run exact Image.point Pillow-oracle fixtures through the Python ABI while
# collecting Rust line, branch, function, and region coverage.

set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/../.." && pwd)"
coverage_dir="$repo_dir/target/coverage"
coverage_output="$coverage_dir/pillow-point-rust.json"
coverage_target_dir="$coverage_dir/point-rust-target"
python_bin="${PYTHON:-$repo_dir/.venv/bin/python}"
test_timeout="${TIMEOUT:-300}"
extension_dir="$repo_dir/pillow-rs-py/python/pillow_rs"
extension_backup_dir="$(mktemp -d)"
extension_had_backup=0

restore_extension() {
    if [[ "$extension_had_backup" -eq 1 ]]; then
        cp -p "$extension_backup_dir"/_core*.so "$extension_dir"/
        rm -f "$extension_backup_dir"/_core*.so
    else
        rm -f "$extension_dir"/_core*.so
    fi
    rmdir "$extension_backup_dir"
}

if compgen -G "$extension_dir/_core*.so" >/dev/null; then
    cp -p "$extension_dir"/_core*.so "$extension_backup_dir"/
    extension_had_backup=1
fi
trap restore_extension EXIT

if command -v maturin >/dev/null 2>&1; then
    maturin_cmd=(maturin)
elif command -v uvx >/dev/null 2>&1; then
    maturin_cmd=(uvx maturin)
else
    printf 'maturin is required; run make setup first\n' >&2
    exit 2
fi

mkdir -p "$coverage_dir"
cd "$repo_dir"

export CARGO_TARGET_DIR="$coverage_target_dir"
export RUSTUP_TOOLCHAIN=nightly
cargo +nightly llvm-cov clean --workspace
eval "$(cargo +nightly llvm-cov --branch show-env --sh)"

"${maturin_cmd[@]}" develop --manifest-path pillow-rs-py/Cargo.toml

set +e
make PYTHON="$python_bin" TIMEOUT="$test_timeout" test-point
test_status=$?
set -e

cargo +nightly llvm-cov --branch report \
    --json \
    --ignore-filename-regex='(^|/)(pillow-rs-freetype|target|\.cargo|rustc)(/|$)' \
    --output-path "$coverage_output"

printf 'Rust Image.point coverage: %s\n' "$coverage_output"
exit "$test_status"
