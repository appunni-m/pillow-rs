#!/bin/bash
# Automated linting for pillow-rs. Runs fmt, clippy, tests, and coverage.
# Uses workspace-level lint config from Cargo.toml — no -D warnings needed.
set -e
cd "$(dirname "$0")/.."

echo "=== rustfmt ==="
cargo fmt --check

echo ""
echo "=== clippy (workspace) ==="
cargo clippy --all-targets --all-features -- -A deprecated

echo ""
echo "=== core tests ==="
cargo test -p pillow-rs

echo ""
echo "=== Python tests ==="
python -m pytest tests/ -q --timeout=300 --json-report --json-report-file=/tmp/report.json

echo ""
echo "=== trust report ==="
python scripts/coverage/compute_coverage.py manifest.yaml /tmp/report.json 2>&1 | grep -E "TRUST|Total"

echo ""
echo "Lint complete."
