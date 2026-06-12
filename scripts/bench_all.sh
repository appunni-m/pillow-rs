#!/bin/bash
# pillow-rs Benchmark Orchestrator — single command to run everything
# Usage: bash scripts/bench_all.sh
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="$ROOT/target/benchmarks"
mkdir -p "$BENCH_DIR"
echo "=== pillow-rs Benchmarks ==="
echo ""

# Step 1: Build release
echo "--- Build (release) ---"
cd "$ROOT/pillow-rs-py" && maturin develop --release 2>&1 | tail -1
cd "$ROOT"
echo ""

# Step 2: Pillow baselines (if missing)
if [ ! -f "$BENCH_DIR/pillow_baseline.json" ]; then
    echo "--- Pillow Baselines ---"
    python3 scripts/bench_pillow_baseline.py --runs 1 --warmup 1 2>&1 | tail -2
    python3 scripts/bench_baseline_add.py 2>&1 | tail -2
    echo ""
fi

# Step 3: CPU benchmarks (Python E2E)
echo "--- CPU (Python E2E) ---"
python3 scripts/bench_full.py --runs 3 --output "$BENCH_DIR/native_cpu.json" 2>&1 | tail -3
echo ""

# Step 4: WASM benchmarks (Node.js)
echo "--- WASM CPU ---"
node scripts/bench_wasm_cpu.mjs 2>&1 | tail -3
echo ""

# Step 5: Generate BENCHMARKS.md
echo "--- BENCHMARKS.md ---"
python3 scripts/bench_aggregate.py
echo ""

# Step 6: Summary
echo "=== Done ==="
head -18 "$ROOT/BENCHMARKS.md"
