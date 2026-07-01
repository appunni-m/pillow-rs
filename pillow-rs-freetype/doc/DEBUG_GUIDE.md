# Autohinter Parity — Remaining 36 Failures: Debugging Guide

## Quick Summary
853 → 36 failures (-817, 99.7%). 14 algorithmic fixes applied.
All remaining failures have edge positions matching C byte-for-byte.
Cells (cover, area) match C for traced glyphs. 
Only pixel-level diffs of 1-255 coverage units remain.

## Categorization

### Category A: Composite glyph bbox diffs (5 failures)
**Glyphs:** DejaVuSerif-BoldItalic_20 (U+690, 2088, 8305, 11388), DejaVuSans-ExtraLight_20 (U+8305)
**Symptom:** our bbox differs by 2-4px width AND all pixel values differ
**Root cause:** composite glyph offset handling at 20pt. The `pp1x_fu` shift
in `scaler.rs:148` uses glyf header xMin, which may differ from the decomposed
minimum for composite glyphs.

**Debugging method:**
```bash
# 1. Add eprintln to scaler.rs at line 148:
eprintln!("[PP1X] gi={glyph_index} outline_raw.xmin={} lsb={} pp1x_fu={pp1x_fu}",
    outline_raw.xmin, h_metric.lsb);

# 2. Run for failing composite glyph:
RUST_LOG=off cargo run -p pillow-rs-freetype --example debug_glyph -- \
  pillow-rs-freetype/tests/fixtures/input/fonts_autohint/DejaVuSerif-BoldItalic.ttf 20 2088

# 3. Compare with C:
# C uses xMin from glyf header, same as us. But C's composite decomposition
# may produce different overall xMin. Check C's reload trace:
/tmp/gen_refs_v4 pillow-rs-freetype/tests/fixtures/input/fonts_autohint/DejaVuSerif-BoldItalic.ttf 2088 20 2>/dev/null | head -10
```

### Category B: Edge link mismatches (~5 failures)
**Glyphs:** geok (DejaVuSans-Oblique 10pt+20pt, DejaVuSansCondensed 10pt+20pt, DejaVuSans 20pt)
**Symptom:** Edge positions identical but link/serif assignments differ → Phase 2 anchor computed from wrong stem → edge pos shifts by 2-15 FU
**Root cause:** Standard width for oblique fonts differs → `link_segments_inner` scoring produces different stem pairs → edges get different links → hint_edges Phase 2 produces different positions

**Debugging method:**
```bash
# 1. Add segment dump to C's link_segments (aflatin.c:2029 area):
fprintf(stderr, "[C LINK_IN] dim=%d n=%d major=%d wc=%u\n", dim, axis->num_segments, (int)axis->major_dir, width_count);
{ int __s; for (__s=0; __s<axis->num_segments; __s++)
    fprintf(stderr, "  S%d: pos=%ld dir=%d u=[%ld,%ld] h=%ld delta=%ld\n", __s,
      (long)segments[__s].pos, (int)segments[__s].dir,
      (long)segments[__s].min_coord, (long)segments[__s].max_coord,
      (long)segments[__s].height, (long)segments[__s].delta);
}

# 2. Add matching dump to our link_segments_inner (latin.rs):
eprintln!("[RUST LINK_IN] dim={dim:?} n={n} major={major_dir:?} wc={width_count}");

# 3. Run both and diff the segment positions and scores.
# The first segment where pos/dir/min_coord/max_coord differs is the root cause.

# 4. Build and run:
cd pillow-rs-freetype/freetype/build && cmake --build . -j$(nproc)
cd /home/appunni/work/pil-wasm
gcc -o /tmp/gen_refs_v4 /tmp/gen_refs_v2.c ... (rebuild C binary with traces)
RUST_LOG=off cargo run -p pillow-rs-freetype --example debug_glyph -- ... 2>&1 | grep "LINK_IN"
/tmp/gen_refs_v4 ... 2>&1 | grep "C LINK_IN"

# 5. If segment positions match but scores differ, the max_width (from standard
# char widths) is wrong. Trace standard char width computation for oblique fonts.
```

### Category C: Pixel-only diffs (~26 failures)
**Glyphs:** geor 2, thai 3, medf 2, telu 2, vaii 1, ethi 1, latp ~10, latb ~3, geok 2
**Symptom:** Same bbox, same edge positions, cells match C exactly, pixel diffs of 1-3 units
**Root cause:** `fill_rule` clamping or sweep span writing with `FT_GRAY_SET` vs our `write_span`

**Debugging method:**
```bash
# 1. Enable cell dumps for a failing glyph:
GRAYS_DUMP_CELLS=1 RUST_LOG=off cargo run -p pillow-rs-freetype --example debug_glyph -- \
  pillow-rs-freetype/tests/fixtures/input/fonts_autohint/FONT.ttf 10 CP_HEX \
  2>&1 | grep "RUST CELLS" -A20 > rust_cells.txt

# 2. C cell dump (need to enable in ftgrays.c):
# Uncomment: if (1) gray_dump_cells(RAS_VAR);
/tmp/gen_refs_v4 FONT.ttf CP_HEX 10 2>/dev/null | grep "C CELLS" -A20 > c_cells.txt

# 3. Diff cells. If no diff → the bug is in sweep's fill_rule or write_span.
diff rust_cells.txt c_cells.txt

# 4. If cells match, feed our cells into C's sweep logic (Python):
python3 -c "
cells = { ... paste from rust_cells.txt ... }
# Run C's exact sweep algorithm on these cells
# Compare resulting pixels with actual C output
"

# 5. For fill_rule investigation:
# C macro: coverage = area >> 9; if (coverage & fill) coverage = ~coverage;
#          if (coverage > 255 && fill & INT_MIN) coverage = 255;
# Our fn:  let mut coverage = area >> 9;
#          if (coverage & fill) != 0 { coverage = !coverage; }
#          if coverage > 255 && (fill & i32::MIN) != 0 { coverage = 255; }
# Verify our !coverage matches C's ~coverage for all 32-bit values
```

## Quick Diagnostic Script
For any single failing glyph, run this to classify the failure:
```bash
#!/bin/bash
FONT="$1" CP="$2" SZ="$3"

# Get our edges
RUST_LOG=autohint::pipeline=trace cargo run -p pillow-rs-freetype --example debug_glyph -- \
  "pillow-rs-freetype/tests/fixtures/input/fonts_autohint/$FONT.ttf" $SZ $CP \
  2>&1 | grep "TR_PHASE4" -A20 | grep "edge\[" > our_edges.txt

# Get C edges
/tmp/gen_refs_v4 "pillow-rs-freetype/tests/fixtures/input/fonts_autohint/$FONT.ttf" $CP $SZ \
  2>&1 | grep "TRACE PHASE4" -A20 | grep "edge\[" > c_edges.txt

# Compare
echo "=== EDGE DIFFS ==="
diff <(sort our_edges.txt) <(sort c_edges.txt)

# Get cells
GRAYS_DUMP_CELLS=1 RUST_LOG=off cargo run -p pillow-rs-freetype --example debug_glyph -- \
  "pillow-rs-freetype/tests/fixtures/input/fonts_autohint/$FONT.ttf" $SZ $CP \
  2>&1 | grep "RUST CELLS" -A20 > our_cells.txt

echo "=== PIXEL MATCH ==="
grep "OUR:\|FT:" from stderr
```

## Top 3 Most Impactful Fixes to Attempt

1. **Fix Category C first** (~26 failures, all pixel-only): Our `fill_rule` 
   produces `!coverage` vs C's `~coverage`. For 32-bit ints these are 
   equivalent (two's complement), but if `coverage` exceeds 11 bits (>>9 
   from a 20-bit area), the extra sign bits affect `~coverage`. Add `coverage &= 0x1FF` 
   before `!coverage` to mask to 9 bits, matching C's implicit truncation.

2. **Fix Category A** (5 failures): Check if C's composite glyph pp1.x uses 
   the decomposed minimum or the header xMin. Add your own computation of 
   pp1x_fu from the actual decomposed point minimum, compare with C's behavior.

3. **Fix Category B** (5 failures): Trace standard char widths for oblique 
   fonts specifically. Oblique fonts skip HORZ hinting (NO_HORIZONTAL), but 
   the standard char width computation happens before the italic check. The 
   standard char's HORZ segments may be processed differently.

## Proven Workflow for Each Fix
1. Add `eprintln!`/`fprintf` at the suspected divergence point
2. Rebuild C binary: `cd pillow-rs-freetype/freetype/build && cmake --build . -j$(nproc) && cd /home/appunni/work/pil-wasm && gcc -o /tmp/gen_refs_v4 /tmp/gen_refs_v2.c ...`
3. Run both and capture to files: `RUST_LOG=off cargo run ... 2>rust_out.txt; /tmp/gen_refs_v4 ... 2>c_out.txt`
4. Diff the raw values
5. Apply fix, verify with `cargo test -p pillow-rs-freetype --test direct_ft_compare`
6. Commit with C file:line reference in commit message
