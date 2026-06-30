# Code Coverage Setup for pillow-rs-freetype Fixture-Based Tests

**Date:** 2026-06-30
**Scope:** Measuring Rust source-level code coverage for tests driven by `coverage_matrix_ft.json` fixtures.

---

## 1. Overview

The `pillow-rs-freetype` test suite uses fixture-driven tests: a single `#[test]` function loops through rows in a JSON matrix, loading fonts and rendering glyphs. Standard Rust test coverage tools treat this as one invocation, so coverage data captures all code paths exercised across the entire fixture.

This document covers:
- Available coverage tools and their setup
- The recommended raw `-C instrument-coverage` approach (most flexible)
- A comparison script for pre/post font-set coverage analysis
- How to interpret coverage reports for fixture-based tests

---

## 2. Prerequisites

### 2.1 Rust Toolchain

Coverage works on **stable** Rust (1.60+). The project uses Rust 1.91.1.

```bash
# Verify Rust version
rustc --version

# Install the LLVM tools component (required for profdata merge and reporting)
rustup component add llvm-tools-preview
```

This installs `llvm-profdata` and `llvm-cov` at:
```
$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | grep host | awk '{print $2}')/bin/
```

### 2.2 System Requirements

No external dependencies needed. The `-C instrument-coverage` flag is built into rustc. The LLVM tools from `llvm-tools-preview` are self-contained.

### 2.3 Available Tools

| Tool | Status | Notes |
|------|--------|-------|
| `-C instrument-coverage` + `llvm-profdata`/`llvm-cov` | **Recommended** | Raw approach, full control, works on stable |
| `cargo-llvm-cov` | Available (`cargo install cargo-llvm-cov`) | Wrapper with nice output; struggles with panicking tests |
| `cargo-tarpaulin` | Not installed | Requires Docker or ptrace; slower |

**Why the raw approach:** `cargo-llvm-cov` aborts when the test binary exits non-zero (our fixture tests panic on SHA mismatches). The raw approach collects coverage regardless of test pass/fail.

---

## 3. Test Structure (How Fixture Tests Work)

The test lives in `pillow-rs-freetype/tests/coverage_matrix_tests.rs`:

```rust
#[test]
fn test_font_coverage_matrix_freetype() {
    run_matrix(BitmapBackend::FreeType, "coverage_matrix_ft.json");
}
```

The `run_matrix()` function:
1. Reads `coverage_matrix_ft.json` from `tests/fixtures/`
2. For each row, loads a font by name (e.g., `"DejaVuSans-Oblique"` → `fonts_autohint/DejaVuSans-Oblique.ttf`)
3. Renders the glyph, computes SHA-256 or bbox, compares with reference
4. Accumulates pass/fail counts, panics at end if any failures

**Key implication for coverage:** This is ONE test invocation. Coverage tools capture all lines hit across all 27,695 rows (29 fonts × 955 rows). To test a font subset, filter the JSON fixture to only include rows for those fonts.

---

## 4. Quick Start: Coverage Comparison Script

The script `scripts/compare_font_coverage.sh` automates the full workflow:

```bash
# Prerequisites
rustup component add llvm-tools-preview

# Compare 5-font minimal set vs 29-font baseline
bash scripts/compare_font_coverage.sh --5vs29

# Compare 5-font vs 8-font sets
bash scripts/compare_font_coverage.sh --5vs8

# Run coverage on current fixture as-is
bash scripts/compare_font_coverage.sh

# Compare two existing profdata files
bash scripts/compare_font_coverage.sh /path/to/baseline.profdata /path/to/compare.profdata
```

### What the script does:

```
1. Filter fixture JSON → keep only specified fonts' rows
2. Swap fixture in place (saves/restores original)
3. cargo test with RUSTFLAGS="-C instrument-coverage"
4. llvm-profdata merge → .profdata
5. llvm-cov report → per-file coverage table
6. llvm-cov export → JSON for programmatic comparison
7. Python diff → lines lost/gained per file
```

### Environment variables:

| Variable | Default | Purpose |
|----------|---------|---------|
| `COVERAGE_CACHE_DIR` | `/tmp/pillow-rs-freetype-coverage` | Where profraw/profdata files are stored |

---

## 5. Manual Workflow (Step by Step)

For manual coverage measurement and comparison:

### 5.1 Build with Coverage Instrumentation

```bash
cd <repo_root>

# Single build with coverage enabled (faster for multiple runs)
RUSTFLAGS="-C instrument-coverage" cargo test -p pillow-rs-freetype --no-run
```

### 5.2 Run Tests and Collect Profiles

```bash
# Clean previous runs
rm -f /tmp/coverage/freetype-*.profraw
mkdir -p /tmp/coverage

# Run with profraw output
RUSTFLAGS="-C instrument-coverage" \
LLVM_PROFILE_FILE="/tmp/coverage/freetype-%m-%p.profraw" \
cargo test -p pillow-rs-freetype test_font_coverage_matrix_freetype -- --nocapture
```

The `%m` in `LLVM_PROFILE_FILE` expands to a binary signature (separates profiles from different binaries). The test will panic on failures — **this is expected**. Profile data is still written.

### 5.3 Merge Profiles

```bash
# Find llvm-profdata from the Rust toolchain
LLVM_BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | grep host | awk '{print $2}')/bin"

# Merge sparse (faster, skips duplicate data)
"$LLVM_BIN/llvm-profdata" merge -sparse \
    /tmp/coverage/freetype-*.profraw \
    -o /tmp/coverage/freetype.profdata
```

### 5.4 Generate Coverage Report

```bash
# Find the test binary
TEST_BIN=$(find target/debug/deps -name "coverage_matrix_tests-*" -not -name "*.d" | head -1)

# Per-file line coverage
"$LLVM_BIN/llvm-cov" report \
    --instr-profile=/tmp/coverage/freetype.profdata \
    --object="$TEST_BIN" \
    --ignore-filename-regex="rustc/|\.cargo/" \
    --show-region-summary=false
```

### 5.5 Export for Programmatic Comparison

```bash
# JSON export
"$LLVM_BIN/llvm-cov" export \
    --instr-profile=/tmp/coverage/freetype.profdata \
    --object="$TEST_BIN" \
    --ignore-filename-regex="rustc/|\.cargo/" \
    --format=text > /tmp/coverage_export.json
```

The JSON structure:
```json
{
  "type": "llvm.coverage.json.export",
  "version": "2.0.1",
  "data": [{
    "files": [
      {
        "filename": "/absolute/path/to/file.rs",
        "segments": [[line, col, count, has_count, is_region_entry, is_gap], ...],
        "summary": {
          "lines": {"count": 1525, "covered": 1406, "percent": 92.2},
          "functions": {"count": 33, "covered": 29, "percent": 87.9},
          ...
        }
      }
    ]
  }]
}
```

### 5.6 Comparing Two Coverage Runs

Use the embedded Python script from `compare_font_coverage.sh` or manually:

```python
import json

def line_counts(export_json_path):
    with open(export_json_path) as f:
        data = json.load(f)
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

baseline = line_counts("/tmp/baseline_export.json")
compare  = line_counts("/tmp/compare_export.json")

for fn in sorted(baseline):
    b = baseline[fn]
    c = compare.get(fn, {})
    lost = [l for l, cnt in b.items() if cnt > 0 and c.get(l, 0) == 0]
    if lost:
        print(f"{fn}: {len(lost)} lines lost")
```

---

## 6. Filtering Fonts for Coverage Comparison

To test a subset of fonts, filter `coverage_matrix_ft.json`:

```python
import json

KEEP = {"DejaVuSans-Oblique", "LiberationSans-Regular", ...}

with open("pillow-rs-freetype/tests/fixtures/coverage_matrix_ft.json") as f:
    data = json.load(f)

data["rows"] = [r for r in data["rows"] if r["font"] in KEEP]

with open("coverage_matrix_ft_filtered.json", "w") as f:
    json.dump(data, f)
```

Then replace the fixture temporarily before running coverage, or modify the test to read from a different path.

---

## 7. Baseline Coverage Results (2026-06-30)

### 7.1 Full 29-Font Suite

| File | Lines | Covered | % | Notes |
|------|-------|---------|---|-------|
| autohint/latin.rs | 1525 | 1406 | 92.20% | Autohinter pipeline |
| autohint/loader.rs | 199 | 198 | 99.50% | Outline loading |
| autohint/types.rs | 84 | 63 | 75.00% | Data structures |
| autohint/coverage.rs | 28 | 6 | 21.43% | Not fully instrumented |
| scaler.rs | 165 | 144 | 87.27% | Glyph scaling |
| grays.rs | 586 | 424 | 72.35% | Rasterizer |
| font.rs | 292 | 189 | 64.73% | Font loading |
| tt/glyf.rs | 253 | 208 | 82.21% | Glyph table parser |
| tt/cmap.rs | 192 | 169 | 88.02% | CMAP table |
| fixed.rs | 44 | 26 | 59.09% | Fixed-point math |
| **TOTAL** | **3685** | **3071** | **83.34%** | |

### 7.2 5-Font Minimal Set Coverage Delta

| File | Δ Lines Lost | Cause |
|------|-------------|-------|
| tt/glyf.rs | -73 | Lost font families use composite/compound glyphs |
| autohint/latin.rs | -37 | Segment merging direction mismatches, directionless catch, round-vs-flat blue zones |
| grays.rs | -8 | Conic curve start handling variations |
| tt/loca.rs | -4 | Short vs long index-to-location format |
| autohint/loader.rs | -2 | Minor edge case |
| font.rs | -2 | Minor edge case |
| **TOTAL** | **-126** | |

### 7.3 Key Finding

The 5-font set loses 126 lines of coverage across 6 files. The largest loss is in `glyf.rs` (composite glyph parsing) because the 5 selected fonts happen to use only simple glyphs for ASCII codepoints. The `latin.rs` loss (37 lines) is from geometric edge cases that specific fonts' outline shapes trigger.

This demonstrates that **font diversity matters for coverage**, disproving the initial hypothesis that the autohinter is entirely font-agnostic. While the autohinter's high-level structure is font-agnostic, the actual code paths taken depend on the specific glyph outline geometries, which vary across fonts.

---

## 8. Using `cargo-llvm-cov` (Alternative)

For tests that don't panic, `cargo-llvm-cov` provides cleaner output:

```bash
# Install
cargo install cargo-llvm-cov

# Run coverage (fails if tests panic — use for non-fixture tests)
cargo llvm-cov --package pillow-rs-freetype --lib -- --nocapture

# HTML report
cargo llvm-cov --package pillow-rs-freetype --open
```

**Limitation for fixture tests:** Our fixture tests intentionally panic on SHA mismatches. `cargo-llvm-cov` interprets the non-zero exit as a coverage collection failure and doesn't produce a report. Use the raw approach instead.

---

## 9. CI Integration

To add coverage tracking to CI:

```yaml
# .github/workflows/coverage.yml (example)
- name: Setup coverage tools
  run: |
    rustup component add llvm-tools-preview

- name: Build with coverage
  run: RUSTFLAGS="-C instrument-coverage" cargo test -p pillow-rs-freetype --no-run

- name: Run coverage
  run: |
    LLVM_PROFILE_FILE="coverage-%m-%p.profraw" \
    cargo test -p pillow-rs-freetype test_font_coverage_matrix_freetype -- --nocapture || true

- name: Generate report
  run: |
    LLVM_BIN="$(rustc --print sysroot)/lib/rustlib/$(rustc -vV | grep host | awk '{print $2}')/bin"
    "$LLVM_BIN/llvm-profdata" merge -sparse coverage-*.profraw -o coverage.profdata
    "$LLVM_BIN/llvm-cov" report --instr-profile=coverage.profdata \
      --object=$(find target/debug/deps -name "coverage_matrix_tests-*" -not -name "*.d" | head -1) \
      --ignore-filename-regex="rustc/|\.cargo/"
```

---

## 10. Troubleshooting

### profraw version mismatch

```
error: raw profile version mismatch: Profile uses raw profile format version = 10;
expected version = 9
```

**Fix:** Use the `llvm-profdata` from the Rust toolchain, not the system one.
```bash
# Correct:
$(rustc --print sysroot)/lib/rustlib/.../bin/llvm-profdata

# Wrong:
/usr/bin/llvm-profdata  # system version may not match rustc
```

### No profraw files after test run

Check that `LLVM_PROFILE_FILE` is set and the directory exists:
```bash
mkdir -p /tmp/coverage
export LLVM_PROFILE_FILE="/tmp/coverage/test-%m.profraw"
```

### Test binary not found

`cargo test --no-run` compiles to `target/debug/deps/`. Use:
```bash
find target/debug/deps -name "coverage_matrix_tests-*" -not -name "*.d" | head -1
```

### llvm-cov report shows 0% for everything

Ensure you're passing `--object` pointing to the CORRECT test binary (the one that was compiled with `-C instrument-coverage`). A rebuild will invalidate old profdata.
