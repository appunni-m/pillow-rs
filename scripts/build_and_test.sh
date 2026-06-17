#!/bin/bash
set -e
cd "$(dirname "$0")/.."

SUITE="${1:-}"  # optional suite arg

echo "=== Building ==="
maturin develop --manifest-path pillow-rs-py/Cargo.toml --release

echo "=== Generating fixtures ==="
chmod -R u+w tests/fixtures/outputs/ 2>/dev/null || true
rm -rf tests/fixtures/outputs/
mkdir -p tests/fixtures/outputs/{jsons,images,raws}
if [ -n "$SUITE" ]; then
    python3 scripts/generate_fixtures.py --suite "$SUITE"
else
    python3 scripts/generate_fixtures.py
fi

echo "=== Running tests ==="
find . -name "__pycache__" -exec rm -rf {} + 2>/dev/null
rm -rf .pytest_cache
if [ -n "$SUITE" ]; then
    SUITE_FILTER="suite${SUITE}"
else
    SUITE_FILTER="not suite1 and not suite2 and not suite3"  # suite0 only by default
fi
PYTHONPATH="$PWD:$PYTHONPATH" python -m pytest tests/test_parity.py -q --tb=line -k "$SUITE_FILTER" --timeout=300
