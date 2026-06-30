#!/usr/bin/env bash
# Coverage comparison script for pillow-rs-freetype fixture-driven tests.
#
# Usage:
#   scripts/compare_font_coverage.sh [BASELINE_DIR] [COMPARE_DIR]
#
# If BASELINE_DIR is given, it should contain a full coverage run's
# profdata file.  If COMPARE_DIR is given, only the comparison step
# runs (skipping profile collection).
#
# Without arguments: runs full 29-font baseline, then 5-font subset,
# and compares.
#
# Requires: Rust toolchain with llvm-tools-preview component.
#           python3 for fixture filtering.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
CACHE_DIR="${COVERAGE_CACHE_DIR:-/tmp/pillow-rs-freetype-coverage}"

FIXTURE_ORIG="$REPO_DIR/pillow-rs-freetype/tests/fixtures/coverage_matrix_ft.json"
FIXTURE_BACKUP="${FIXTURE_ORIG}.backup"

# ── LLVM tools from Rust toolchain ──────────────────────────────────────
RUST_SYSROOT="$(rustc --print sysroot)"
LLVM_BIN="$RUST_SYSROOT/lib/rustlib/$(rustc -vV | grep 'host:' | awk '{print $2}')/bin"
LLVM_PROFDATA="$LLVM_BIN/llvm-profdata"
LLVM_COV="$LLVM_BIN/llvm-cov"

# ── Default font sets ───────────────────────────────────────────────────
FULL_FONTS_29="DejaVuMathTeXGyre DejaVuSans-ExtraLight DejaVuSans-Oblique DejaVuSansMono DejaVuSansMono-Oblique DejaVuSerif-Bold DejaVuSerif-Italic DejaVuSerifCondensed-Bold DejaVuSerifCondensed-Italic LiberationMono-Italic LiberationMono-Regular LiberationSans-BoldItalic LiberationSans-Regular LiberationSansNarrow-Bold LiberationSansNarrow-BoldItalic LiberationSerif-Bold LiberationSerif-BoldItalic NotoMono-Regular NotoSans-Bold NotoSans-BoldItalic NotoSansMath-Regular NotoSerif-Italic NotoSerif-Regular NotoSerifDisplay-Bold NotoSerifDisplay-BoldItalic Ubuntu-Italic[wdth,wght] UbuntuMono-Italic[wght] UbuntuMono[wght] UbuntuSans[wdth,wght]"

# 5-font minimal set (from font-reduction-research.md)
FONTS_5="DejaVuSans-Oblique LiberationSans-Regular DejaVuSerif-Bold DejaVuSansMono DejaVuSerifCondensed-Bold"

# 8-font conservative set (includes Extralight for future coverage, Noto family, narrow variation)
FONTS_8="DejaVuSans-ExtraLight DejaVuSans-Oblique LiberationSans-Regular NotoSans-Bold DejaVuSerif-Italic DejaVuSerif-Bold DejaVuSansMono LiberationSansNarrow-Bold"

# ── Helper functions ────────────────────────────────────────────────────

filter_fixture() {
    local fonts="$1"
    local output="$2"
    python3 - "$fonts" "$FIXTURE_ORIG" "$output" << 'PYEOF'
import json, sys

fonts = set(sys.argv[1].split())
input_path = sys.argv[2]
output_path = sys.argv[3]

with open(input_path) as f:
    data = json.load(f)

original = len(data["rows"])
data["rows"] = [r for r in data["rows"] if r["font"] in fonts]
filtered = len(data["rows"])

if "summary" in data:
    data["summary"]["total_rows"] = filtered
    data["summary"]["active_rows"] = filtered
    data["summary"]["fonts"] = len(fonts)

with open(output_path, "w") as f:
    json.dump(data, f)

print(f"Filtered fixture: {original} → {filtered} rows ({len(fonts)} fonts)")
PYEOF
}

run_coverage() {
    local label="$1"
    local fixture_path="$2"
    local out_dir="$3"

    mkdir -p "$out_dir"
    rm -f "$out_dir"/*.profraw

    # Swap fixture (only if different path from original)
    local fixture_real
    fixture_real="$(realpath "$fixture_path" 2>/dev/null || readlink -f "$fixture_path")"
    local orig_real
    orig_real="$(realpath "$FIXTURE_ORIG" 2>/dev/null || readlink -f "$FIXTURE_ORIG")"
    if [ "$fixture_real" != "$orig_real" ]; then
        if [ ! -f "$FIXTURE_BACKUP" ]; then
            cp "$FIXTURE_ORIG" "$FIXTURE_BACKUP"
        fi
        cp "$fixture_path" "$FIXTURE_ORIG"
    fi

    echo "=== Running $label coverage ==="
    RUSTFLAGS="-C instrument-coverage" \
    LLVM_PROFILE_FILE="$out_dir/${label}-%m-%p.profraw" \
    cargo test -p pillow-rs-freetype test_font_coverage_matrix_freetype -- --nocapture 2>&1 | grep "font matrix" || true

    # Merge profiles
    local profdata="$out_dir/${label}.profdata"
    "$LLVM_PROFDATA" merge -sparse "$out_dir"/${label}-*.profraw -o "$profdata" 2>/dev/null

    # Restore fixture
    if [ -f "$FIXTURE_BACKUP" ]; then
        mv "$FIXTURE_BACKUP" "$FIXTURE_ORIG"
    fi

    echo "$profdata"
}

show_coverage() {
    local profdata="$1"
    local label="$2"
    local test_bin
    test_bin=$(find target/debug/deps -name "coverage_matrix_tests-*" -not -name "*.d" | head -1)

    echo ""
    echo "=== $label Coverage Report ==="
    "$LLVM_COV" report \
        --instr-profile="$profdata" \
        --object="$test_bin" \
        --ignore-filename-regex="rustc/|\.cargo/" \
        --show-region-summary=false 2>&1
}

compare_coverage() {
    local base_profdata="$1"
    local comp_profdata="$2"
    local base_label="$3"
    local comp_label="$4"
    local test_bin
    test_bin=$(find target/debug/deps -name "coverage_matrix_tests-*" -not -name "*.d" | head -1)

    echo ""
    echo "=== Coverage Delta: $base_label → $comp_label ==="

    # Export both as JSON for diff
    "$LLVM_COV" export \
        --instr-profile="$base_profdata" \
        --object="$test_bin" \
        --ignore-filename-regex="rustc/|\.cargo/" \
        --format=text 2>/dev/null > "$CACHE_DIR/base_export.json"

    "$LLVM_COV" export \
        --instr-profile="$comp_profdata" \
        --object="$test_bin" \
        --ignore-filename-regex="rustc/|\.cargo/" \
        --format=text 2>/dev/null > "$CACHE_DIR/comp_export.json"

    python3 "$CACHE_DIR/base_export.json" "$CACHE_DIR/comp_export.json" << 'PYEOF'
import json, sys

with open(sys.argv[1]) as f:
    base = json.load(f)
with open(sys.argv[2]) as f:
    comp = json.load(f)

def line_counts(data):
    result = {}
    for f in data['data'][0]['files']:
        fn = f['filename']
        counts = {}
        for seg in f.get('segments', []):
            if len(seg) >= 4 and seg[3]:  # has_count
                line, count = seg[0], seg[2]
                counts[line] = max(counts.get(line, 0), count)
        result[fn] = counts
    return result

base_map = line_counts(base)
comp_map = line_counts(comp)

for fn in sorted(base_map):
    b = base_map[fn]
    c = comp_map.get(fn, {})
    lost = [l for l, cnt in b.items() if cnt > 0 and c.get(l, 0) == 0]
    gained = [l for l, cnt in c.items() if cnt > 0 and b.get(l, 0) == 0]
    if lost or gained:
        short = fn.split('/')[-1]
        print(f"\n  {short}:")
        if lost:
            print(f"    -{len(lost)} lines lost (was covered, now missed)")
        if gained:
            print(f"    +{len(gained)} lines gained (was missed, now covered)")

# Summary
total_lost = 0
total_gained = 0
for fn in base_map:
    b = base_map[fn]
    c = comp_map.get(fn, {})
    total_lost += sum(1 for l, cnt in b.items() if cnt > 0 and c.get(l, 0) == 0)
    total_gained += sum(1 for l, cnt in c.items() if cnt > 0 and b.get(l, 0) == 0)

print(f"\n  TOTAL: -{total_lost} lines lost, +{total_gained} lines gained")
PYEOF
}

# ── Main ────────────────────────────────────────────────────────────────

echo "Coverage comparison tool for pillow-rs-freetype"
echo "Cache dir: $CACHE_DIR"
echo "LLVM tools: $LLVM_BIN"
echo ""

# Verify prerequisites
if ! command -v rustc &>/dev/null; then
    echo "ERROR: rustc not found"
    exit 1
fi
if ! "$LLVM_PROFDATA" --version &>/dev/null; then
    echo "ERROR: llvm-profdata not found. Run: rustup component add llvm-tools-preview"
    exit 1
fi

mkdir -p "$CACHE_DIR"

if [ $# -ge 2 ]; then
    # Compare mode
    compare_coverage "$1" "$2" "baseline" "compare"
elif [ $# -eq 1 ] && [ "$1" = "--5vs29" ]; then
    # Quick 5-vs-29 comparison
    echo "=== 29-font baseline ==="
    filter_fixture "$FULL_FONTS_29" "$CACHE_DIR/fixture_29f.json"
    PROF_29=$(run_coverage "29f" "$CACHE_DIR/fixture_29f.json" "$CACHE_DIR/29f")

    echo ""
    echo "=== 5-font subset ==="
    filter_fixture "$FONTS_5" "$CACHE_DIR/fixture_5f.json"
    PROF_5=$(run_coverage "5f" "$CACHE_DIR/fixture_5f.json" "$CACHE_DIR/5f")

    show_coverage "$PROF_29" "29-font"
    show_coverage "$PROF_5" "5-font"
    compare_coverage "$PROF_29" "$PROF_5" "29-font" "5-font"
elif [ $# -eq 1 ] && [ "$1" = "--5vs8" ]; then
    # 5-vs-8 comparison
    filter_fixture "$FONTS_5" "$CACHE_DIR/fixture_5f.json"
    PROF_5=$(run_coverage "5f" "$CACHE_DIR/fixture_5f.json" "$CACHE_DIR/5f")

    filter_fixture "$FONTS_8" "$CACHE_DIR/fixture_8f.json"
    PROF_8=$(run_coverage "8f" "$CACHE_DIR/fixture_8f.json" "$CACHE_DIR/8f")

    show_coverage "$PROF_5" "5-font"
    show_coverage "$PROF_8" "8-font"
    compare_coverage "$PROF_5" "$PROF_8" "5-font" "8-font"
else
    # Default: run on current fixture as-is
    echo "=== Current fixture coverage ==="
    if [ ! -f "$FIXTURE_ORIG" ]; then
        echo "ERROR: fixture not found: $FIXTURE_ORIG"
        exit 1
    fi
    PROF=$(run_coverage "current" "$FIXTURE_ORIG" "$CACHE_DIR/current")
    show_coverage "$PROF" "Current"
fi
