#!/bin/bash
# Automated linting for pillow-rs. Runs clippy, fmt, and coverage check.
set -e
cd "$(dirname "$0")/.."

echo "=== rustfmt ==="
cargo fmt --check 2>&1 | tail -3

echo ""
echo "=== clippy (core) ==="
cargo clippy -p pillow-rs-core 2>&1 | tail -5

echo ""
echo "=== clippy (py bindings) ==="
cargo clippy -p pillow-rs-py 2>&1 | tail -5

echo ""
echo "=== tests ==="
python -m pytest tests/ -q --json-report --json-report-file=/tmp/report.json 2>&1 | tail -2

echo ""
echo "=== trust report ==="
python scripts/compute_coverage.py manifest.yaml /tmp/report.json 2>&1 | grep -E "TRUST|Total"
echo ""
echo "Lint complete."
