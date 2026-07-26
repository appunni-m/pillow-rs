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

python3 - "$coverage_output" <<'PY'
import json
import sys
from pathlib import Path

MAX_SIGNED_32 = 2_147_483_647


def cap_coverage_counts(value):
    if isinstance(value, list):
        for index, item in enumerate(value):
            if isinstance(item, int) and item > MAX_SIGNED_32:
                value[index] = MAX_SIGNED_32
            else:
                cap_coverage_counts(item)
    elif isinstance(value, dict):
        for item in value.values():
            cap_coverage_counts(item)


path = Path(sys.argv[1])
document = json.loads(path.read_text())
cap_coverage_counts(document)
path.write_text(json.dumps(document, separators=(",", ":")))
PY

printf 'Rust font coverage with FreeType dependency: %s\n' "$coverage_output"
exit "$test_status"
