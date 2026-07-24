#!/usr/bin/env bash
# Run the Pillow oracle corpus through the installed PyO3 extension while
# collecting coverage from the Rust implementation and binding crates.

set -euo pipefail

repo_dir="$(cd "$(dirname "$0")/../.." && pwd)"
coverage_dir="$repo_dir/target/coverage"
coverage_output="$coverage_dir/pillow-python-abi-rust.json"
coverage_target_dir="$coverage_dir/python-abi-target"
python_bin="${PYTHON:-$repo_dir/.venv/bin/python}"
maturin_bin="${MATURIN:-$repo_dir/.venv/bin/maturin}"
test_timeout="${TIMEOUT:-30}"
pytest_report="${REPORT:-/tmp/report.json}"

if command -v "$maturin_bin" >/dev/null 2>&1; then
    maturin_cmd=("$maturin_bin")
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
make fixtures
fixture_status=$?
if [[ "$fixture_status" -eq 0 ]]; then
    "$python_bin" -m pytest tests/test_parity.py -q --tb=short \
        --timeout="$test_timeout" \
        --json-report \
        --json-report-file="$pytest_report" \
        --strict-covers
    test_status=$?
else
    test_status=$fixture_status
fi
set -e

cargo +nightly llvm-cov --branch report \
    --json \
    --ignore-filename-regex='(^|/)(pillow-rs-freetype|target|\.cargo|rustc)(/|$)' \
    --output-path "$coverage_output"

printf 'Rust-through-Python-ABI coverage: %s\n' "$coverage_output"
exit "$test_status"
