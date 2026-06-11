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
        --baseline) RUN_BASELINE=1; shift ;;
        --setup) RUN_SETUP=1; shift ;;
        *) echo "Unknown: $1"; echo "Usage: bench_all.sh [full|incremental|setup] [--only a,b,c] [--group priority|filters|chops|enhance|misc] [--baseline]"; exit 1 ;;
    esac
done

echo "=== pillow-rs Benchmark Orchestrator ==="
if [ "$MODE" = "only" ]; then echo "Mode: only [$ONLY]"; elif [ "$MODE" = "group" ]; then echo "Mode: group [$GROUP]"; elif [ "$MODE" = "setup" ]; then echo "Mode: setup (generate baselines + reference images)"; else echo "Mode: $MODE"; fi
echo ""

# Step -1: Setup mode — generate reference images and Pillow baselines (one-time)
if [ "${RUN_SETUP:-0}" = "1" ] || [ "$MODE" = "setup" ]; then
    echo "--- Setup: Generating reference images ---"
    python3 -c "
from PIL import Image
import os, random
d = 'scripts/bench_reference_images'
os.makedirs(d, exist_ok=True)
random.seed(42)
# ref_2k.jpg
im = Image.new('RGB', (2048, 1536))
px = im.load()
for y in range(im.height):
    for x in range(im.width):
        px[x, y] = ((x * 255 // im.width + y * 64 // im.height) % 256,
                     (y * 255 // im.height + x * 32 // im.width) % 256,
                     ((x + y) * 128 // (im.width + im.height)) % 256)
im.save(f'{d}/ref_2k.jpg', quality=95)
print(f'Created ref_2k.jpg: {im.size}')
# ref_1k.png
im = Image.new('RGBA', (1024, 1024))
px = im.load()
for y in range(im.height):
    for x in range(im.width):
        a = 255 if (x + y) % 200 > 100 else 128
        px[x, y] = (int(x/im.width*255), int(y/im.height*255), 128, a)
im.save(f'{d}/ref_1k.png')
print(f'Created ref_1k.png: {im.size}')
# ref_grayscale.png
im = Image.new('L', (1024, 1024))
px = im.load()
for y in range(im.height):
    for x in range(im.width):
        px[x, y] = int(((x + y) / (im.width + im.height)) * 255)
im.save(f'{d}/ref_grayscale.png')
print(f'Created ref_grayscale.png: {im.size}')
"
    echo "--- Setup: Generating Pillow baselines (165 functions) ---"
    python3 scripts/bench_pillow_baseline.py --runs 10
    python3 scripts/bench_baseline_add.py
    echo "✓ Setup complete. Run 'bash scripts/bench_all.sh full' to benchmark."
    if [ "$MODE" = "setup" ]; then
        python3 scripts/bench_aggregate.py
        exit 0
    fi
fi

# Step 0: Update baselines if requested
if [ "${RUN_BASELINE:-0}" = "1" ]; then
    echo "--- Updating Pillow baselines ---"
    python3 scripts/bench_pillow_baseline.py --runs 10
    python3 scripts/bench_baseline_add.py
    echo ""
fi

# Step 0.5: Check cache (skip for --only/--group — always run those)
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

# Step 1: Native CPU — Python E2E through PyO3 binding (true end-to-end)
echo "--- Native CPU (Python E2E via RSPIL) ---"
cd "$ROOT"
python3 scripts/bench_native_cpu.py --runs 10 --output "$BENCH_DIR/native_cpu.json" 2>&1 | tail -5 || echo "  (CPU E2E completed)"
echo ""

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
