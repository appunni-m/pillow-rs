#!/bin/bash
set -e
cd "$(dirname "$0")/.."

SUITE="${1:-}"  # optional: 0, 1, 2, all

echo "=== Building ==="
maturin develop --manifest-path pillow-rs-py/Cargo.toml --release

echo "=== Generating fixtures ==="
chmod -R u+w tests/fixtures/outputs/ 2>/dev/null || true
rm -rf tests/fixtures/outputs/
mkdir -p tests/fixtures/outputs/{jsons,images,raws}
if [ "$SUITE" = "all" ]; then
    python3 scripts/generate_fixtures.py
elif [ -n "$SUITE" ]; then
    python3 scripts/generate_fixtures.py --suite "$SUITE"
else
    python3 scripts/generate_fixtures.py
fi

echo "=== Running tests ==="
find . -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
rm -rf .pytest_cache 2>/dev/null || true
if [ "$SUITE" = "all" ]; then
    PYTHONPATH="$PWD:$PYTHONPATH" python -m pytest tests/test_parity.py -q --tb=line --timeout=300
elif [ -n "$SUITE" ]; then
    PYTHONPATH="$PWD:$PYTHONPATH" python -m pytest tests/test_parity.py -q --tb=line -k "suite${SUITE}" --timeout=300
else
    PYTHONPATH="$PWD:$PYTHONPATH" python -m pytest tests/test_parity.py -q --tb=line -k "not suite1 and not suite2 and not suite3" --timeout=300
fi
