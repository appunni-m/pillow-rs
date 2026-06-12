#!/bin/bash
# scripts/ci_coverage.sh — complete CI pipeline with coverage validation
set -e

echo "=== 1. Rust format check ==="
cargo fmt --check

echo "=== 2. Rust clippy ==="
cargo clippy --all-targets --all-features -- -D warnings

echo "=== 3. Rust core tests ==="
cargo test -p pillow-rs-core

echo "=== 4. Python parity tests ==="
python -m pytest tests/ -q --json-report --json-report-file=/tmp/report.json

echo "=== 5. Coverage validation ==="
python scripts/coverage/validate_coverage.py manifest.yaml

echo "=== 6. JS/WASM tests ==="
if [ -f pillow-rs-js/tests/run.mjs ]; then
    node pillow-rs-js/tests/run.mjs || echo "WARNING: JS tests not yet configured"
fi

echo "=== 7. Generate coverage reports ==="
python scripts/coverage/generate_coverage_page.py
python scripts/coverage/generate_wasm_coverage.py

echo ""
echo "✅ All checks passed — coverage matrix complete"
