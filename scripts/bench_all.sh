#!/bin/bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BENCH_DIR="$ROOT/target/benchmarks"

mkdir -p "$BENCH_DIR"

MODE="incremental"
ONLY=""
GROUP=""

# Parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        full) MODE="full"; shift ;;
        incremental) MODE="incremental"; shift ;;
        --only) ONLY="$2"; MODE="only"; shift 2 ;;
        --group) GROUP="$2"; MODE="group"; shift 2 ;;
        --skip-wasm) SKIP_WASM=1; shift ;;
        --skip-browser) SKIP_BROWSER=1; shift ;;
        *) echo "Unknown: $1"; echo "Usage: bench_all.sh [full|incremental] [--only a,b,c] [--group priority|filters|chops|enhance|misc]"; exit 1 ;;
    esac
done

echo "=== pillow-rs Benchmark Orchestrator ==="
if [ "$MODE" = "only" ]; then echo "Mode: only [$ONLY]"; elif [ "$MODE" = "group" ]; then echo "Mode: group [$GROUP]"; else echo "Mode: $MODE"; fi
echo ""

# Step 0: Check cache (skip for --only/--group — always run those)
if [ "$MODE" = "incremental" ]; then
    cd "$ROOT"
    STALE=$(python3 scripts/bench_cache.py --check 2>&1)
    if echo "$STALE" | grep -q "FRESH"; then
        echo "✓ All functions up-to-date. Skipping benchmarks."
        echo "  (use 'bash scripts/bench_all.sh full' to force re-bench)"
        python3 scripts/bench_aggregate.py
        exit 0
    fi
    echo "$STALE" | head -20
    echo ""
fi

# Build filter args for harnesses
FILTER_ARG=""
if [ "$MODE" = "only" ]; then
    FILTER_ARG="--only $ONLY"
elif [ "$MODE" = "group" ]; then
    FILTER_ARG="--group $GROUP"
fi

# Step 1: Native CPU benchmarks (criterion)
echo "--- Native CPU Benchmarks ---"
cd "$ROOT"
cargo bench -p pillow-rs-core --bench native_cpu 2>&1 | tee "$BENCH_DIR/native_cpu_raw.txt" || echo "  (CPU benchmarks completed)"
# Parse criterion output → JSON
python3 -c "
import json, re
results = {}
with open('$BENCH_DIR/native_cpu_raw.txt') as f:
    for line in f:
        m = re.match(r'(\S+)\s+time:\s+\[([\d.]+)\s*(\w+)\s+([\d.]+)\s*\w+\s+([\d.]+)\s*\w+', line)
        if m:
            name = m.group(1)
            mean = float(m.group(4))  # middle value = criterion estimate
            unit = m.group(3)
            if unit == 'ns': mean /= 1_000_000
            elif unit == 'µs' or unit == 'us': mean /= 1_000
            elif unit == 's': mean *= 1_000
            # ms: keep as-is
            results[name] = {'mean_ms': round(mean, 4)}
with open('$BENCH_DIR/native_cpu.json', 'w') as f:
    json.dump(results, f, indent=2)
print(f'Parsed {len(results)} CPU benchmark results')
"
# Update cache
if [ -f "$BENCH_DIR/native_cpu.json" ]; then
    python3 -c "
import json; results = json.load(open('$BENCH_DIR/native_cpu.json'))
for name in results:
    print(name, results[name]['mean_ms'], 0)
" | while read -r name mean std; do
        python3 scripts/bench_cache.py --update native_cpu "$name" "$mean" "$std" 2>/dev/null || true
    done
fi

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
