#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="$ROOT/target/benchmarks"

mkdir -p "$BENCH_DIR"

MODE="${1:-incremental}"

echo "=== pillow-rs Benchmark Orchestrator ==="
echo "Mode: $MODE"
echo ""

# Step 0: Check cache
if [ "$MODE" = "incremental" ]; then
    cd "$ROOT"
    STALE=$(python3 scripts/bench_cache.py --check 2>&1)
    if echo "$STALE" | grep -q "FRESH"; then
        echo "> All functions up-to-date. Skipping benchmarks."
        echo "  (use 'bash scripts/bench_all.sh full' to force re-bench)"
        python3 scripts/bench_aggregate.py
        exit 0
    fi
    echo "$STALE" | head -20
    echo ""
fi

# Step 1: Native CPU benchmarks (criterion)
echo "--- Native CPU Benchmarks ---"
cd "$ROOT"
cargo bench -p pillow-rs-core --bench native_cpu 2>&1 | tail -30 || echo "  (CPU benchmarks completed with warnings)"

# Step 2: WASM CPU benchmarks (Node.js)
echo ""
echo "--- WASM CPU Benchmarks ---"
if [ -f "scripts/bench_wasm_cpu.mjs" ]; then
    node scripts/bench_wasm_cpu.mjs 2>&1 || echo "  (WASM CPU harness skipped - may need build)"
else
    echo "  (harness not yet created - skipping)"
fi

# Step 3: Browser benchmarks (Puppeteer)
echo ""
echo "--- Browser Benchmarks ---"
if [ -f "scripts/bench_browser.mjs" ]; then
    node scripts/bench_browser.mjs 2>&1 || echo "  (Browser harness skipped)"
else
    echo "  (harness not yet created - skipping)"
fi

# Step 4: Generate BENCHMARKS.md
echo ""
echo "--- Generating BENCHMARKS.md ---"
cd "$ROOT"
python3 scripts/bench_aggregate.py
echo "> BENCHMARKS.md updated"

# Step 5: Show summary
echo ""
echo "=== Done ==="
head -15 "$ROOT/BENCHMARKS.md"
